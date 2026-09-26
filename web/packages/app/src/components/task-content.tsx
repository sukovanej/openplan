import { useQuery, useQueryClient } from "@tanstack/react-query"
import {
  type KeyboardEvent,
  lazy,
  type ReactNode,
  Suspense,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react"
import { useNavigate } from "react-router-dom"

import type { TaskDetail, TaskListItem } from "@openplan/api-client"
import type { BodyEditorHandle, TaskOption } from "@openplan/editor"
import { statusField } from "@openplan/task-ui"
import { Button } from "@openplan/ui"

import { listTasks, writeBody } from "../lib/api"
import { BodyDraft, localDraftStore, type SaveState } from "../lib/body-draft"
import { useDetailAction } from "../lib/detail-actions"
import { taskKey, tasksKey, useProjectMutation } from "../lib/query-client"
import { abortable } from "../lib/runtime"
import { joinBody, splitBody } from "../lib/task-content"
import { taskMatches } from "../lib/task-search"
import { BodySkeleton } from "./states"

// CodeMirror and its markdown grammars are most of the editor's weight, and only a task page needs them.
const BodyEditor = lazy(() => import("@openplan/editor").then((module) => ({ default: module.BodyEditor })))

const NO_TASKS: ReadonlyArray<TaskListItem> = []

// Each save is a commit, so the text goes out when the reader leaves it, not on each key.
function useBodyDraft(project: string, id: string, body: string) {
  const client = useQueryClient()
  const { mutateAsync } = useProjectMutation(project)
  const [draft] = useState(
    () =>
      new BodyDraft(
        body,
        async (base, text) => {
          const detail = (await mutateAsync(writeBody(project, id, base, text))) as TaskDetail
          client.setQueryData(taskKey(project, id), detail)
          return detail.body
        },
        localDraftStore(`openplan:draft:${project}:${id}`),
        (text) => (splitBody(text).title?.trim() === "" ? "A task needs a title." : undefined),
      ),
  )
  const view = useSyncExternalStore(draft.subscribe, draft.getSnapshot)

  useEffect(() => draft.received(body), [draft, body])

  useEffect(() => {
    if (draft.restored) void draft.save()
    const flush = () => void draft.save()
    const onHidden = () => {
      if (document.visibilityState === "hidden") flush()
    }
    window.addEventListener("blur", flush)
    window.addEventListener("pagehide", flush)
    document.addEventListener("visibilitychange", onHidden)
    return () => {
      window.removeEventListener("blur", flush)
      window.removeEventListener("pagehide", flush)
      document.removeEventListener("visibilitychange", onHidden)
      flush()
    }
  }, [draft])

  return { draft, ...view }
}

function useTaskSearch(project: string, id: string, wanted: boolean) {
  const tasks = useQuery({
    queryKey: tasksKey(project),
    queryFn: abortable(listTasks(project)),
    enabled: wanted,
  })
  const all = tasks.data
  return useCallback(
    (query: string): ReadonlyArray<TaskOption> =>
      taskMatches(all ?? NO_TASKS, query, new Set([id])).flatMap(({ task, indices }) => {
        const status = statusField(task.metadata)
        return status === undefined ? [] : [{ task: { id: task.id, title: task.title, status }, indices }]
      }),
    [all, id],
  )
}

function TitleField({
  value,
  onChange,
  onEnter,
  onSave,
}: {
  value: string
  onChange: (title: string) => void
  onEnter: () => void
  onSave: () => void
}) {
  const field = useRef<HTMLTextAreaElement>(null)
  // The field is as tall as its text, which wraps at whatever width the panel gives it.
  useLayoutEffect(() => {
    const element = field.current
    if (element === null) return
    const fit = () => {
      element.style.height = "0px"
      element.style.height = `${element.scrollHeight}px`
    }
    fit()
    const observer = new ResizeObserver(fit)
    observer.observe(element.parentElement ?? element)
    return () => observer.disconnect()
  }, [value])
  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    const element = event.currentTarget
    const atEnd = element.selectionStart === element.value.length && element.selectionEnd === element.value.length
    if (event.key === "Enter" || (event.key === "ArrowDown" && atEnd)) {
      event.preventDefault()
      onEnter()
    } else if (event.key === "Escape") {
      event.preventDefault()
      element.blur()
    } else if ((event.metaKey || event.ctrlKey) && event.key === "s") {
      event.preventDefault()
      onSave()
    }
  }
  return (
    <textarea
      ref={field}
      aria-label="Title"
      rows={1}
      value={value}
      placeholder="Task title"
      spellCheck
      onChange={(event) => onChange(event.target.value.replace(/\n/g, " "))}
      onKeyDown={onKeyDown}
      className="placeholder:text-muted-foreground/60 mb-1.5 block w-full resize-none overflow-hidden bg-transparent text-2xl font-semibold tracking-tight outline-none"
    />
  )
}

function SaveNote({ state, onRetry }: { state: SaveState; onRetry: () => void }) {
  if (state.kind === "saved") return null
  if (state.kind === "failed") {
    return (
      <span role="alert" className="text-danger flex min-w-0 items-center gap-1.5">
        <span className="truncate">Not saved: {state.message}</span>
        <Button onClick={onRetry} className="h-5 px-1.5 text-xs">
          Try again
        </Button>
      </span>
    )
  }
  return (
    <span aria-live="polite" title="The text saves when you leave it. ⌘S saves it now.">
      {state.kind === "saving" ? "Saving…" : "Unsaved"}
    </span>
  )
}

export function TaskContent({
  project,
  id,
  body,
  refs,
  abbreviation,
  fallbackTitle,
  meta,
}: {
  project: string
  id: string
  body: string
  refs: TaskDetail["refs"]
  abbreviation: string
  fallbackTitle: string
  meta: (saveNote: ReactNode) => ReactNode
}) {
  const navigate = useNavigate()
  const { draft, shown, state } = useBodyDraft(project, id, body)
  const save = () => void draft.save()
  const content = useMemo(() => splitBody(shown), [shown])
  // What the reader types is kept against the body it was typed over; a body taken from elsewhere (the
  // daemon, or a merge) replaces it.
  const [typed, setTyped] = useState({ over: content, title: content.title, description: content.description })
  const { title, description } = typed.over === content ? typed : content
  const [searching, setSearching] = useState(false)
  const searchTasks = useTaskSearch(project, id, searching)
  const box = useRef<HTMLDivElement>(null)
  const editor = useRef<BodyEditorHandle>(null)

  useDetailAction("edit-description", () => editor.current?.focus("end"))

  const edit = (nextTitle: string | undefined, nextDescription: string) => {
    setTyped({ over: content, title: nextTitle, description: nextDescription })
    draft.change(joinBody(content, nextTitle, nextDescription))
    // A click on a checkbox changes the text without moving the focus into it, and no leave follows.
    if (!box.current?.contains(document.activeElement)) save()
  }

  const leave = () => {
    setTimeout(() => {
      if (!box.current?.contains(document.activeElement)) save()
    })
  }

  return (
    <div ref={box} onFocus={() => setSearching(true)} onBlur={leave}>
      <TitleField
        value={title ?? fallbackTitle}
        onChange={(next) => edit(next, description)}
        onEnter={() => editor.current?.focus("start")}
        onSave={save}
      />
      {meta(<SaveNote state={state} onRetry={save} />)}
      <Suspense fallback={<BodySkeleton />}>
        <BodyEditor
          ref={editor}
          project={project}
          abbreviation={abbreviation}
          refs={refs}
          markdown={content.description}
          onChange={(next) => edit(title, next)}
          onSave={save}
          searchTasks={searchTasks}
          navigate={navigate}
        />
      </Suspense>
    </div>
  )
}

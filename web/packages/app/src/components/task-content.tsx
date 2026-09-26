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

import type {
  DocDetail,
  DocListItem,
  DocRef,
  Problem,
  TaskDetail,
  TaskListItem,
  TaskRef,
  TaskText,
} from "@openplan/api-client"
import type { BodyEditorHandle, RefOption } from "@openplan/editor"
import { docPath, ProblemBanner, statusField, withoutTextProblems } from "@openplan/task-ui"
import { Button } from "@openplan/ui"

import { listAllDocs, listTasks, writeDocText, writeTaskText } from "../lib/api"
import { useDetailAction } from "../lib/detail-actions"
import { docMatches } from "../lib/doc-search"
import { allDocsKey, docKey, taskKey, tasksKey, useProjectMutation } from "../lib/query-client"
import { abortable } from "../lib/runtime"
import { taskMatches } from "../lib/task-search"
import { localDraftStore, type SaveState, TextDraft, type TextKind, type WriteText } from "../lib/text-draft"
import { BodySkeleton } from "./states"

// CodeMirror and its markdown grammars are most of the editor's weight, and only a task or a doc
// page needs them.
const BodyEditor = lazy(() => import("@openplan/editor").then((module) => ({ default: module.BodyEditor })))

const NO_TASKS: ReadonlyArray<TaskListItem> = []
const NO_DOCS: ReadonlyArray<DocListItem> = []
const MAX_DOC_OPTIONS = 4

// A task and a doc both edit as a title and the markdown under it. A doc's markdown travels as
// `body`, and the editor holds it as `description` too.
const textKind = (refused: string): TextKind<TaskText> => ({
  same: (a, b) => a.title === b.title && a.description === b.description,
  is: (value): value is TaskText =>
    typeof value === "object" &&
    value !== null &&
    typeof (value as TaskText).title === "string" &&
    typeof (value as TaskText).description === "string",
  refuse: (text) => (text.title.trim() === "" ? refused : undefined),
})

const taskText = textKind("A task needs a title.")
const docText = textKind("A doc needs a title.")

// Each save is a commit, so the text goes out when the reader leaves it, not on each key.
function useText(draftKey: string, stored: TaskText, write: WriteText<TaskText>, kind: TextKind<TaskText>) {
  const [draft] = useState(() => new TextDraft<TaskText>(stored, write, localDraftStore(draftKey, kind), kind))
  const view = useSyncExternalStore(draft.subscribe, draft.getSnapshot)

  useEffect(() => draft.received(stored), [draft, stored])

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

// The tasks and the docs of the project, as `[[` offers them. `self` is the page's own task or doc,
// which a body does not reference.
function useRefSearch(project: string, self: { task?: string; doc?: string }, wanted: boolean) {
  const tasks = useQuery({ queryKey: tasksKey(project), queryFn: abortable(listTasks(project)), enabled: wanted })
  const docs = useQuery({
    queryKey: allDocsKey(project),
    queryFn: abortable(listAllDocs([project])),
    enabled: wanted,
  })
  const allTasks = tasks.data ?? NO_TASKS
  const allDocs = docs.data ?? NO_DOCS
  const { task, doc } = self
  return useCallback(
    (query: string): ReadonlyArray<RefOption> => {
      const taskOptions = taskMatches(allTasks, query, new Set(task === undefined ? [] : [task])).flatMap(
        ({ task, indices }): RefOption[] => {
          const status = statusField(task.metadata)
          return status === undefined
            ? []
            : [{ kind: "task", task: { id: task.id, title: task.title, status }, indices }]
        },
      )
      const docOptions = docMatches(allDocs, query, new Set(doc === undefined ? [] : [doc]))
        .slice(0, MAX_DOC_OPTIONS)
        .map(({ doc, indices }): RefOption => ({
          kind: "doc",
          doc: { name: doc.name, title: doc.title || doc.name },
          indices,
        }))
      return [...taskOptions, ...docOptions]
    },
    [allTasks, allDocs, task, doc],
  )
}

function TitleField({
  value,
  placeholder,
  onChange,
  onEnter,
  onSave,
}: {
  value: string
  placeholder: string
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
      placeholder={placeholder}
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

interface TextContentProps {
  project: string
  stored: TaskText
  draftKey: string
  kind: TextKind<TaskText>
  write: WriteText<TaskText>
  self: { task?: string; doc?: string }
  noun: { title: string; body: string; empty: string }
  refs: ReadonlyArray<TaskRef> | undefined
  docRefs: ReadonlyArray<DocRef> | undefined
  problems: ReadonlyArray<Problem>
  abbreviation: string
  meta: (saveNote: ReactNode) => ReactNode
}

function TextContent({
  project,
  stored,
  draftKey,
  kind,
  write,
  self,
  noun,
  refs,
  docRefs,
  problems,
  abbreviation,
  meta,
}: TextContentProps) {
  const navigate = useNavigate()
  const { draft, shown, state } = useText(draftKey, stored, write, kind)
  const save = () => void draft.save()
  // What the reader types is kept against the text it was typed over; a text taken from elsewhere
  // (the daemon, or a merge) replaces it.
  const [typed, setTyped] = useState({ over: shown, text: shown })
  const text = typed.over === shown ? typed.text : shown
  const [searching, setSearching] = useState(false)
  const searchRefs = useRefSearch(project, self, searching)
  const box = useRef<HTMLDivElement>(null)
  const editor = useRef<BodyEditorHandle>(null)

  useDetailAction("edit-description", () => editor.current?.focus("end"))

  const edit = (next: TaskText) => {
    setTyped({ over: shown, text: next })
    draft.change(next)
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
      <ProblemBanner problems={state.kind === "saved" ? problems : withoutTextProblems(problems)} />
      <TitleField
        value={text.title}
        placeholder={noun.title}
        onChange={(next) => edit({ ...text, title: next })}
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
          docRefs={docRefs}
          markdown={shown.description}
          onChange={(next) => edit({ ...text, description: next })}
          onSave={save}
          searchRefs={searchRefs}
          label={noun.body}
          placeholder={noun.empty}
          navigate={navigate}
        />
      </Suspense>
    </div>
  )
}

const TASK_NOUN = { title: "Task title", body: "Description", empty: "Add description…" }
const DOC_NOUN = { title: "Doc title", body: "Body", empty: "Write the doc…" }

export function TaskContent({
  project,
  id,
  title,
  description,
  refs,
  docRefs,
  problems,
  abbreviation,
  meta,
}: {
  project: string
  id: string
  title: string
  description: string
  refs: TaskDetail["refs"]
  docRefs: TaskDetail["doc_refs"]
  problems: ReadonlyArray<Problem>
  abbreviation: string
  meta: (saveNote: ReactNode) => ReactNode
}) {
  const client = useQueryClient()
  const { mutateAsync } = useProjectMutation(project)
  const stored = useMemo(() => ({ title, description }), [title, description])
  const write: WriteText<TaskText> = async (base, text) => {
    const detail = (await mutateAsync(writeTaskText(project, id, base, text))) as TaskDetail
    client.setQueryData(taskKey(project, id), detail)
    return { title: detail.title, description: detail.description }
  }
  return (
    <TextContent
      project={project}
      stored={stored}
      draftKey={`openplan:draft:${project}:${id}`}
      kind={taskText}
      write={write}
      self={{ task: id }}
      noun={TASK_NOUN}
      refs={refs}
      docRefs={docRefs}
      problems={problems}
      abbreviation={abbreviation}
      meta={meta}
    />
  )
}

// A new title renames the doc, and its page moves to the new name with it.
export function DocContent({
  project,
  doc,
  abbreviation,
  meta,
}: {
  project: string
  doc: DocDetail
  abbreviation: string
  meta: (saveNote: ReactNode) => ReactNode
}) {
  const client = useQueryClient()
  const navigate = useNavigate()
  const { mutateAsync } = useProjectMutation(project)
  const stored = useMemo(() => ({ title: doc.title, description: doc.body }), [doc.title, doc.body])
  const write: WriteText<TaskText> = async (base, text) => {
    const detail = (await mutateAsync(
      writeDocText(
        project,
        doc.name,
        { title: base.title, body: base.description },
        { title: text.title, body: text.description },
      ),
    )) as DocDetail
    client.setQueryData(docKey(project, detail.name), detail)
    if (detail.name !== doc.name) navigate(docPath(project, detail.name), { replace: true })
    return { title: detail.title, description: detail.body }
  }
  return (
    <TextContent
      project={project}
      stored={stored}
      draftKey={`openplan:draft:${project}:doc:${doc.name}`}
      kind={docText}
      write={write}
      self={{ doc: doc.name }}
      noun={DOC_NOUN}
      refs={doc.refs}
      docRefs={doc.doc_refs}
      problems={doc.problems}
      abbreviation={abbreviation}
      meta={meta}
    />
  )
}

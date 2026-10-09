import { useQueryClient } from "@tanstack/react-query"
import { Effect } from "effect"
import { Check, Plus, X } from "lucide-react"
import {
  type Dispatch,
  type ReactNode,
  type SetStateAction,
  Suspense,
  useCallback,
  useEffect,
  useEffectEvent,
  useMemo,
  useRef,
  useState,
} from "react"
import { useLocation, useNavigate } from "react-router-dom"

import type { ProjectView, TagView } from "@openplan/api-client"
import type { BodyEditorHandle } from "@openplan/editor"
import { TagList, taskPath } from "@openplan/task-ui"
import {
  Button,
  Kbd,
  Menu,
  type MenuItem,
  MenuTrigger,
  Modal,
  Panel,
  PanelBody,
  PanelHeader,
  PanelTitle,
  useDismissOnOutsideClick,
} from "@openplan/ui"

import { createTag, createTask } from "../lib/api"
import { flash } from "../lib/flash"
import { errorText } from "../lib/format"
import { defaultProject } from "../lib/new-task"
import { selectedProject } from "../lib/project-scope"
import { useWritableProjects } from "../lib/projects"
import { tagsKey, useProjectMutation } from "../lib/query-client"
import { type TagsByName, tagsWith, tagsWithout, useTags } from "../lib/tags"
import { BodySkeleton } from "./states"
import { AddTagButton, TagPicker } from "./tags-field"
import { BodyEditor, TitleField, useRefSearch } from "./task-content"

// `project` is unset while the draft is blank. Once the reader starts it, the draft keeps its
// project, because its tags and its `[[KEY]]` references belong to that project.
interface Draft {
  readonly project?: string
  readonly title: string
  readonly body: string
  readonly tags: ReadonlyArray<string>
}

const EMPTY: Draft = { title: "", body: "", tags: [] }
const NO_TAGS: ReadonlyArray<string> = []

const isBlank = (draft: Draft) => draft.title === "" && draft.body === "" && draft.tags.length === 0

interface Drafting {
  readonly draft: Draft
  readonly setDraft: Dispatch<SetStateAction<Draft>>
  readonly lastProject: string | undefined
  readonly setLastProject: (project: string) => void
}

// The draft outlives the dialog, so Esc or a click outside does not lose what the reader typed.
export function NewTaskDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [draft, setDraft] = useState(EMPTY)
  const [lastProject, setLastProject] = useState<string>()
  const drafting = useMemo(() => ({ draft, setDraft, lastProject, setLastProject }), [draft, lastProject])
  return (
    <Modal open={open} onClose={onClose} label="New task" className="w-full max-w-[59rem]">
      <NewTaskForm drafting={drafting} onClose={onClose} />
    </Modal>
  )
}

function NewTaskForm({ drafting, onClose }: { drafting: Drafting; onClose: () => void }) {
  const writable = useWritableProjects()
  const { pathname, search } = useLocation()

  if (writable === undefined) return null
  const { draft, lastProject } = drafting
  const projectNames = writable.map((project) => project.name)
  const chosen =
    draft.project !== undefined && projectNames.includes(draft.project)
      ? draft.project
      : defaultProject(projectNames, selectedProject(pathname, search), lastProject)
  const project = writable.find((candidate) => candidate.name === chosen)
  if (project === undefined) {
    return (
      <Shell onClose={onClose}>
        <p className="text-muted-foreground text-sm">
          No project can take a task. Register a repository with <code>openplan project add</code>.
        </p>
      </Shell>
    )
  }
  // The body editor reads its project once, so another project needs another editor.
  return <DraftForm key={project.name} project={project} drafting={drafting} onClose={onClose} />
}

function DraftForm({
  project,
  drafting: { draft, setDraft, setLastProject },
  onClose,
}: {
  project: ProjectView
  drafting: Drafting
  onClose: () => void
}) {
  const navigate = useNavigate()
  const client = useQueryClient()
  const { mutate, reset, isPending, isError, error } = useProjectMutation(project.name, "inline")
  const { byName: registry } = useTags(project.name)
  const [searching, setSearching] = useState(false)
  const searchRefs = useRefSearch(project.name, {}, searching)
  const titleField = useRef<HTMLTextAreaElement>(null)
  const editor = useRef<BodyEditorHandle>(null)
  const title = draft.title.trim()
  // The draft keeps the names of another project when the reader switches, so switching back finds
  // them again. Only the names this project's registry holds are shown and sent.
  const tags = useMemo(
    () => (registry === undefined ? NO_TAGS : draft.tags.filter((name) => registry.has(name))),
    [registry, draft.tags],
  )

  const change = useCallback(
    (patch: Partial<Draft>) => {
      setDraft((held) => {
        const next = { ...held, ...patch }
        return { ...next, project: isBlank(next) ? undefined : project.name }
      })
      reset()
    },
    [setDraft, project.name, reset],
  )
  const changeTags = useCallback((next: ReadonlyArray<string>) => change({ tags: next }), [change])

  // The registry the picker read does not hold the new tag yet, and a chip for a name the registry
  // does not hold reads as an unknown tag, so the new tag goes into the cached registry at once.
  const register = useCallback(
    (name: string) =>
      mutate(
        Effect.tap(createTag(project.name, { name }), (tag) =>
          Effect.sync(() => {
            client.setQueryData<ReadonlyArray<TagView>>(tagsKey(project.name), (held) =>
              held === undefined || held.some((each) => each.name === tag.name) ? held : [...held, tag],
            )
            setDraft((held) => ({
              ...held,
              project: project.name,
              tags: held.tags.includes(tag.name) ? held.tags : [...held.tags, tag.name],
            }))
          }),
        ),
      ),
    [mutate, project.name, setDraft, client],
  )

  // The dialog can close while the write is on its way, so what follows the write runs with the
  // write and not in a callback of this form.
  const create = (another: boolean) => {
    if (title === "" || isPending) return
    const body = draft.body.trim()
    const input = { title, body: body === "" ? undefined : body, tags: tags.length === 0 ? undefined : tags }
    mutate(
      Effect.tap(createTask(project.name, input), (id) =>
        Effect.sync(() => {
          setLastProject(project.name)
          if (another) {
            flash.show(`Created ${id}`, "ok")
            setDraft((held) => ({ ...held, title: "", body: "" }))
            titleField.current?.focus()
          } else {
            setDraft(EMPTY)
            onClose()
            navigate(taskPath(project.name, id))
          }
        }),
      ),
    )
  }

  // The focus leaves the form when a tag picker or the project menu closes, or after Esc, so the
  // document catches the chord. Its capture phase also comes before the body editor, which takes ⌘⏎
  // for a new line.
  const submit = useEffectEvent(create)
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Enter" || !(event.metaKey || event.ctrlKey)) return
      event.preventDefault()
      event.stopPropagation()
      submit(event.shiftKey)
    }
    document.addEventListener("keydown", onKeyDown, true)
    return () => document.removeEventListener("keydown", onKeyDown, true)
  }, [])

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        create(false)
      }}
      className="contents"
    >
      <Shell
        onClose={onClose}
        footer={
          <>
            <ProjectPicker project={project.name} onPick={(name) => setDraft((held) => ({ ...held, project: name }))} />
            {isError && (
              <p role="alert" className="text-danger min-w-0 truncate text-sm">
                {errorText(error)}
              </p>
            )}
            <span className="text-muted-foreground ml-auto flex shrink-0 items-center gap-1.5 text-xs max-md:hidden">
              <Kbd token="mod+shift+Enter" /> creates another
            </span>
            <Button
              type="submit"
              variant="accent"
              size="md"
              disabled={title === "" || isPending}
              className="shrink-0 disabled:opacity-40 max-md:ml-auto"
            >
              <Plus className="size-3.5" aria-hidden />
              Create
              <Kbd token="mod+Enter" className="max-md:hidden" />
            </Button>
          </>
        }
      >
        <TitleField
          field={titleField}
          autoFocus
          value={draft.title}
          placeholder="Task title"
          onChange={(next) => change({ title: next })}
          onEnter={() => editor.current?.focus("start")}
          onSave={() => create(false)}
        />
        {registry !== undefined && (
          <DraftTags
            names={tags}
            registry={registry}
            onChange={changeTags}
            onRegister={register}
            className="mb-4 min-h-8"
          />
        )}
        <div onFocus={() => setSearching(true)}>
          <Suspense fallback={<BodySkeleton />}>
            <BodyEditor
              ref={editor}
              project={project.name}
              abbreviation={project.abbreviation}
              markdown={draft.body}
              onChange={(body) => change({ body })}
              onSave={() => create(false)}
              searchRefs={searchRefs}
              label="Description"
              placeholder="Add description…"
              navigate={(path) => {
                onClose()
                navigate(path)
              }}
            />
          </Suspense>
        </div>
      </Shell>
    </form>
  )
}

// As wide as the task column of the task page and nearly as tall as the window, so the draft reads
// as the task it becomes.
function Shell({ footer, onClose, children }: { footer?: ReactNode; onClose: () => void; children: ReactNode }) {
  return (
    <Panel className="bg-background h-[85dvh] rounded-xl shadow-lg">
      <PanelHeader className="gap-2">
        <PanelTitle>New task</PanelTitle>
        <Button size="icon" aria-label="Close" onClick={onClose} className="ml-auto">
          <X className="size-4" aria-hidden />
        </Button>
      </PanelHeader>
      <PanelBody className="p-6 max-md:p-4">{children}</PanelBody>
      {footer !== undefined && <div className="flex shrink-0 items-center gap-3 border-t px-4 py-2.5">{footer}</div>}
    </Panel>
  )
}

// The picker sits in the footer at the bottom of the panel, so its menu opens upward over the body.
function ProjectPicker({ project, onPick }: { project: string; onPick: (name: string) => void }) {
  const writable = useWritableProjects()
  const [open, setOpen] = useState(false)
  const root = useRef<HTMLDivElement>(null)
  useDismissOnOutsideClick(root, open ? () => setOpen(false) : undefined)

  if (writable === undefined || writable.length < 2) return null
  const items: ReadonlyArray<MenuItem> = writable.map((candidate) => ({
    key: candidate.name,
    content: (
      <>
        <span className="min-w-0 grow truncate">{candidate.name}</span>
        {candidate.name === project && (
          <Check className="text-muted-foreground size-3.5 shrink-0" aria-label="Current" />
        )}
      </>
    ),
  }))

  return (
    <div ref={root} className="relative">
      <MenuTrigger open={open} aria-label="Project" onClick={() => setOpen(!open)} className="max-w-56">
        <span className="truncate">{project}</span>
      </MenuTrigger>
      {open && (
        <Menu
          label="Projects"
          items={items}
          initial={writable.findIndex((candidate) => candidate.name === project)}
          onPick={(index) => {
            setOpen(false)
            onPick(writable[index].name)
          }}
          onClose={() => setOpen(false)}
          className="absolute bottom-full left-0 z-30 mb-1 max-h-56 w-56 overflow-y-auto"
        />
      )}
    </div>
  )
}

function DraftTags({
  names,
  registry,
  onChange,
  onRegister,
  className,
}: {
  names: ReadonlyArray<string>
  registry: TagsByName
  onChange: (tags: ReadonlyArray<string>) => void
  onRegister: (name: string) => void
  className?: string
}) {
  const [adding, setAdding] = useState(false)
  const close = useCallback(() => setAdding(false), [])
  const pick = useCallback((name: string) => onChange(tagsWith(names, registry, name)), [names, registry, onChange])
  return (
    <TagList
      names={names}
      tags={registry}
      onRemove={(name) => onChange(tagsWithout(names, registry, name))}
      trailing={
        adding ? (
          <TagPicker names={names} tags={registry} onPick={pick} onRegister={onRegister} onClose={close} />
        ) : (
          <AddTagButton carried={names.length} onClick={() => setAdding(true)} />
        )
      }
      className={className}
    />
  )
}

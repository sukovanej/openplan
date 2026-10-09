import { useQueryClient } from "@tanstack/react-query"
import { Check, ChevronsUpDown, Plus, X } from "lucide-react"
import {
  type Dispatch,
  type KeyboardEvent,
  type ReactNode,
  type SetStateAction,
  Suspense,
  useCallback,
  useMemo,
  useRef,
  useState,
} from "react"
import { useLocation, useNavigate } from "react-router-dom"

import type { ProjectView, TagView } from "@openplan/api-client"
import type { BodyEditorHandle } from "@openplan/editor"
import { TagChip } from "@openplan/task-ui"
import {
  Button,
  cn,
  CONTROL_HEIGHT,
  Kbd,
  Menu,
  type MenuItem,
  Modal,
  Panel,
  PanelBody,
  PanelHeader,
  PanelTitle,
  useDismissOnOutsideClick,
} from "@openplan/ui"

import { createTag, createTask } from "../lib/api"
import { demotedReason, useFaults } from "../lib/faults"
import { flash } from "../lib/flash"
import { errorText } from "../lib/format"
import { defaultProject } from "../lib/new-task"
import { selectedProject } from "../lib/project-scope"
import { useProjects } from "../lib/projects"
import { tagsKey, useProjectMutation } from "../lib/query-client"
import { tagsWith, tagsWithout, useTags } from "../lib/tags"
import { BodySkeleton } from "./states"
import { TagPicker } from "./tags-field"
import { BodyEditor, TitleField, useRefSearch } from "./task-content"

// `project` is unset until the reader starts the draft. From then on the draft keeps its project,
// because its tags and its `[[KEY]]` references belong to that project.
interface Draft {
  readonly project?: string
  readonly title: string
  readonly body: string
  readonly tags: ReadonlyArray<string>
}

const EMPTY: Draft = { title: "", body: "", tags: [] }

// The draft outlives the dialog, so Esc or a click outside does not lose what the reader typed.
export function NewTaskDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [draft, setDraft] = useState(EMPTY)
  const [last, setLast] = useState<string>()
  return (
    <Modal open={open} onClose={onClose} label="New task" className="w-full max-w-3xl">
      <NewTaskForm draft={draft} setDraft={setDraft} last={last} onCreated={setLast} onClose={onClose} />
    </Modal>
  )
}

// A demoted project cannot take a write, so it is no choice here.
function NewTaskForm({
  draft,
  setDraft,
  last,
  onCreated,
  onClose,
}: {
  draft: Draft
  setDraft: Dispatch<SetStateAction<Draft>>
  last: string | undefined
  onCreated: (project: string) => void
  onClose: () => void
}) {
  const projects = useProjects()
  const faults = useFaults()
  const { pathname, search } = useLocation()
  const writable = useMemo(
    () => (projects ?? []).filter((project) => demotedReason(faults, project.name) === undefined),
    [projects, faults],
  )

  if (projects === undefined) return null
  const names = writable.map((project) => project.name)
  const name =
    draft.project !== undefined && names.includes(draft.project)
      ? draft.project
      : defaultProject(names, selectedProject(pathname, search), last)
  const project = writable.find((each) => each.name === name)
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
  return (
    <DraftForm
      key={project.name}
      project={project}
      writable={writable}
      draft={draft}
      setDraft={setDraft}
      onCreated={onCreated}
      onClose={onClose}
    />
  )
}

function DraftForm({
  project,
  writable,
  draft,
  setDraft,
  onCreated,
  onClose,
}: {
  project: ProjectView
  writable: ReadonlyArray<ProjectView>
  draft: Draft
  setDraft: Dispatch<SetStateAction<Draft>>
  onCreated: (project: string) => void
  onClose: () => void
}) {
  const navigate = useNavigate()
  const client = useQueryClient()
  const mutation = useProjectMutation(project.name, "inline")
  const { reset, mutate } = mutation
  const { byName: registry } = useTags(project.name)
  const [searching, setSearching] = useState(false)
  const searchRefs = useRefSearch(project.name, {}, searching)
  const title = draft.title.trim()
  const titleField = useRef<HTMLTextAreaElement>(null)
  const editor = useRef<BodyEditorHandle>(null)

  const change = useCallback(
    (patch: Partial<Draft>) => {
      setDraft((held) => ({ ...held, project: project.name, ...patch }))
      reset()
    },
    [setDraft, project.name, reset],
  )

  const changeTags = useCallback((tags: ReadonlyArray<string>) => change({ tags }), [change])

  const register = useCallback(
    (name: string) =>
      mutate(createTag(project.name, { name }), {
        // Until the registry holds the new tag, its chip reads as a name the registry does not know.
        onSuccess: (created) => {
          const tag = created as TagView
          client.setQueryData<ReadonlyArray<TagView>>(tagsKey(project.name), (held) =>
            held === undefined || held.some((each) => each.name === tag.name) ? held : [...held, tag],
          )
          setDraft((held) => ({ ...held, project: project.name, tags: [...held.tags, tag.name] }))
        },
      }),
    [mutate, project.name, setDraft, client],
  )

  const create = (another: boolean) => {
    if (title === "" || mutation.isPending) return
    const body = draft.body.trim()
    mutate(
      createTask(project.name, {
        title,
        body: body === "" ? undefined : body,
        tags: draft.tags.length === 0 ? undefined : draft.tags,
      }),
      {
        onSuccess: (id) => {
          flash.show(`Created ${String(id)}`, "ok")
          onCreated(project.name)
          if (another) {
            setDraft((held) => ({ ...held, title: "", body: "" }))
            titleField.current?.focus()
          } else {
            setDraft(EMPTY)
            onClose()
          }
        },
      },
    )
  }

  // The capture phase reaches the form before the body editor, which would take ⌘↵ for a new line.
  const onKeyDownCapture = (event: KeyboardEvent) => {
    if (event.key !== "Enter" || !(event.metaKey || event.ctrlKey)) return
    event.preventDefault()
    event.stopPropagation()
    create(event.shiftKey)
  }

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        create(false)
      }}
      onKeyDownCapture={onKeyDownCapture}
      className="contents"
    >
      <Shell
        onClose={onClose}
        footer={
          <>
            {writable.length > 1 && (
              <ProjectPicker
                project={project.name}
                writable={writable}
                onPick={(name) => {
                  if (name !== project.name) setDraft((held) => ({ ...held, project: name, tags: [] }))
                }}
              />
            )}
            {mutation.isError && (
              <p role="alert" className="text-danger min-w-0 truncate text-sm">
                {errorText(mutation.error)}
              </p>
            )}
            <span className="text-muted-foreground ml-auto flex shrink-0 items-center gap-1.5 text-xs max-md:hidden">
              <Kbd token="mod+shift+Enter" /> creates another
            </span>
            <Button
              type="submit"
              variant="accent"
              size="md"
              disabled={title === "" || mutation.isPending}
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
        <div className="mb-4 flex min-h-8 items-center gap-4">
          {registry !== undefined && (
            <DraftTags names={draft.tags} registry={registry} onChange={changeTags} onRegister={register} />
          )}
        </div>
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

// The task page's panel, with a footer that says where the task goes and makes it.
function Shell({ footer, onClose, children }: { footer?: ReactNode; onClose: () => void; children: ReactNode }) {
  return (
    <Panel className="bg-background h-auto max-h-[85dvh] rounded-xl shadow-lg">
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
function ProjectPicker({
  project,
  writable,
  onPick,
}: {
  project: string
  writable: ReadonlyArray<ProjectView>
  onPick: (name: string) => void
}) {
  const [open, setOpen] = useState(false)
  const root = useRef<HTMLDivElement>(null)
  useDismissOnOutsideClick(root, open ? () => setOpen(false) : undefined)

  const items: ReadonlyArray<MenuItem> = writable.map((each) => ({
    key: each.name,
    content: (
      <>
        <span className="min-w-0 grow truncate">{each.name}</span>
        {each.name === project && <Check className="text-muted-foreground size-3.5 shrink-0" aria-label="Current" />}
      </>
    ),
  }))

  return (
    <div ref={root} className="relative">
      <button
        type="button"
        aria-label="Project"
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
        className={cn(
          "hover:bg-muted focus-visible:ring-ring inline-flex max-w-56 items-center gap-1.5 rounded-md border px-2 text-sm transition-colors focus-visible:ring-2 focus-visible:outline-none",
          CONTROL_HEIGHT,
        )}
      >
        <span className="truncate">{project}</span>
        <ChevronsUpDown className="text-muted-foreground size-3.5 shrink-0" aria-hidden />
      </button>
      {open && (
        <Menu
          label="Projects"
          items={items}
          initial={writable.findIndex((each) => each.name === project)}
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
}: {
  names: ReadonlyArray<string>
  registry: ReadonlyMap<string, TagView>
  onChange: (tags: ReadonlyArray<string>) => void
  onRegister: (name: string) => void
}) {
  const [adding, setAdding] = useState(false)
  const close = useCallback(() => setAdding(false), [])
  const pick = useCallback((name: string) => onChange(tagsWith(names, registry, name)), [names, registry, onChange])
  return (
    <div className="flex flex-wrap items-center gap-1">
      {names.map((name) => (
        <TagChip
          key={name}
          name={name}
          tag={registry.get(name)}
          onRemove={() => onChange(tagsWithout(names, registry, name))}
        />
      ))}
      {adding ? (
        <TagPicker names={names} tags={registry} onPick={pick} onRegister={onRegister} onClose={close} />
      ) : (
        <Button
          variant="accent"
          onClick={() => setAdding(true)}
          aria-label="Add tag"
          className={names.length > 0 ? "px-1.5" : undefined}
        >
          <Plus className="size-3.5" />
          {names.length === 0 && "Add tag"}
        </Button>
      )}
    </div>
  )
}

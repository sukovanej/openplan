import { useQuery } from "@tanstack/react-query"
import { Check, Plus, Trash2, X } from "lucide-react"
import { type ReactNode, useRef, useState } from "react"
import { useParams } from "react-router-dom"

import type { Color, TagView } from "@openplan/api-client"
import { ColorDot, ColorPicker, TagChip } from "@openplan/task-ui"
import {
  Button,
  cn,
  EmptyState,
  Panel,
  PanelBody,
  Row,
  Section,
  SkeletonList,
  TextInput,
  Tooltip,
  useDismissOnOutsideClick,
} from "@openplan/ui"

import { createTag, deleteTag, listTags, patchTag, TaskRejected } from "../lib/api"
import { demotedReason, useDemotedReason, useFaults } from "../lib/faults"
import { errorText } from "../lib/format"
import { useProject, useProjects } from "../lib/projects"
import { tagsKey, useProjectMutation } from "../lib/query-client"
import { useRowCursor } from "../lib/row-cursor"
import { abortable } from "../lib/runtime"
import { described, renamed } from "../lib/tags"

// This page holds no task rows, and the cursor is the board's — left as it was, `j` then Enter here
// would open a task the reader can no longer see.
const NO_ROWS: ReadonlyArray<string> = []
const focusOnMount = (element: HTMLDivElement | null) => element?.focus()
type ProjectMutation = ReturnType<typeof useProjectMutation>

export function TagsRoute() {
  const { project } = useParams()
  useRowCursor(NO_ROWS)
  return project === undefined ? <EveryProjectTags /> : <OneProjectTags project={project} />
}

function OneProjectTags({ project }: { project: string }) {
  const projects = useProjects()
  const known = useProject(project)
  const reason = useDemotedReason(project)
  // Until the list arrives every name is equally plausible, so an unknown one is only unknown once
  // the daemon has answered.
  if (projects !== undefined && known === undefined) {
    return <EmptyState title="No such project" detail={project} />
  }
  if (reason !== undefined) {
    return <EmptyState title={`${project} is not being served`} detail={reason} />
  }
  return (
    <TagsPanel>
      <ProjectTags project={project} />
    </TagsPanel>
  )
}

// Each project keeps a registry of its own, so each has its own section and its own form.
function EveryProjectTags() {
  const projects = useProjects()
  const faults = useFaults()
  return (
    <TagsPanel>
      {projects === undefined ? (
        <SkeletonList count={3} className="h-10 w-full" />
      ) : projects.length === 0 ? (
        <p className="text-muted-foreground text-sm">No projects yet.</p>
      ) : (
        projects.map((entry, index) => {
          const reason = demotedReason(faults, entry.name)
          return (
            <Section key={entry.name} title={entry.name} className={cn(index === 0 && "mt-0 border-t-0 pt-0")}>
              {reason === undefined ? (
                <ProjectTags project={entry.name} />
              ) : (
                <p className="text-muted-foreground text-sm">Not being served: {reason}</p>
              )}
            </Section>
          )
        })
      )}
    </TagsPanel>
  )
}

function TagsPanel({ children }: { children: ReactNode }) {
  return (
    <Panel>
      <PanelBody className="p-6 max-md:p-4">{children}</PanelBody>
    </Panel>
  )
}

function ProjectTags({ project }: { project: string }) {
  const tags = useQuery({
    queryKey: tagsKey(project),
    queryFn: abortable(listTags(project)),
  })
  const registration = useProjectMutation(project)
  // The read and the writes go to the same store, so a registry that cannot be read is a registry
  // that cannot be written either — offering the form would only produce a toast.
  if (tags.isError) return <EmptyState title="Could not load tags" detail={errorText(tags.error)} />
  if (tags.isPending) return <SkeletonList count={3} className="h-10 w-full" />
  return (
    <>
      <TagForm project={project} mutation={registration} className="mb-6" />
      {tags.data.length === 0 ? (
        <p className="text-muted-foreground text-sm">No tags yet. Register one above.</p>
      ) : (
        <ul>
          {tags.data.map((tag, index) => (
            <li key={tag.name}>
              <TagRow project={project} tag={tag} last={index === tags.data.length - 1} />
            </li>
          ))}
        </ul>
      )}
    </>
  )
}

type Editing = "no" | "deleting" | "forcing" | "recolouring"

function TagRow({ project, tag, last }: { project: string; tag: TagView; last: boolean }) {
  const [editing, setEditing] = useState<Editing>("no")
  const mutation = useProjectMutation(project)
  return (
    <Row variant="divided" last={last} className="flex flex-wrap items-center gap-3 px-2 py-2">
      {/* The dismiss-on-outside-click watches this wrapper, not the popover: with the button outside
          it, pressing the button counted as an outside click, and the click that followed reopened
          what the mousedown had just closed. */}
      <Palette
        open={editing === "recolouring"}
        value={tag.color}
        label={tag.display}
        disabled={editing !== "no" && editing !== "recolouring"}
        onToggle={() => setEditing((open) => (open === "recolouring" ? "no" : "recolouring"))}
        onClose={() => setEditing("no")}
        onPick={(color) => {
          setEditing("no")
          mutation.mutate(patchTag(project, tag.name, { color }))
        }}
      />
      <InPlace
        project={project}
        tag={tag}
        value={tag.display}
        patch={renamed}
        label={`Rename ${tag.display}`}
        className="flex items-center gap-3"
        inputClassName="w-48 max-sm:flex-1"
      >
        <TagChip name={tag.name} tag={tag} />
        <span className="text-muted-foreground/70 font-mono text-xs">{tag.name}</span>
      </InPlace>
      <InPlace
        project={project}
        tag={tag}
        value={tag.description ?? ""}
        patch={described}
        label={`Describe ${tag.display}`}
        className="text-muted-foreground min-w-0 truncate text-sm max-sm:order-last max-sm:basis-full max-sm:whitespace-normal"
        inputClassName="flex-1 max-sm:order-last max-sm:basis-full"
      >
        {tag.description ?? <span className="text-muted-foreground/50">Add description</span>}
      </InPlace>
      <div className="ml-auto flex shrink-0 items-center gap-1">
        {editing === "deleting" || editing === "forcing" ? (
          <DeleteConfirm
            project={project}
            tag={tag}
            mutation={mutation}
            forcing={editing === "forcing"}
            onRefused={() => setEditing("forcing")}
            onCancel={() => setEditing("no")}
          />
        ) : (
          <Button
            variant="danger"
            aria-label={`Delete ${tag.display}`}
            onClick={() => setEditing("deleting")}
            className="text-danger/70 hover:text-danger"
          >
            <Trash2 className="size-3.5" />
          </Button>
        )}
      </div>
    </Row>
  )
}

function InPlace({
  project,
  tag,
  value,
  patch,
  label,
  className,
  inputClassName,
  children,
}: {
  project: string
  tag: TagView
  value: string
  patch: typeof renamed
  label: string
  className: string
  inputClassName: string
  children: ReactNode
}) {
  const [open, setOpen] = useState(false)
  if (open) {
    return (
      <InPlaceInput
        project={project}
        tag={tag}
        value={value}
        patch={patch}
        label={label}
        className={inputClassName}
        onClose={() => setOpen(false)}
      />
    )
  }
  return (
    <button
      type="button"
      aria-label={label}
      onClick={() => setOpen(true)}
      className={cn(
        "hover:bg-muted focus-visible:ring-ring -mx-1 cursor-text rounded-md px-1 text-left transition-colors focus-visible:ring-2 focus-visible:outline-none",
        className,
      )}
    >
      {children}
    </button>
  )
}

function InPlaceInput({
  project,
  tag,
  value,
  patch,
  label,
  className,
  onClose,
}: {
  project: string
  tag: TagView
  value: string
  patch: typeof renamed
  label: string
  className: string
  onClose: () => void
}) {
  const [typed, setTyped] = useState(value)
  const mutation = useProjectMutation(project, "inline")
  const save = () => {
    if (mutation.isPending) return
    const change = patch(tag, typed)
    if (change === undefined) onClose()
    else mutation.mutate(patchTag(project, tag.name, change), { onSuccess: onClose })
  }
  return (
    <>
      <TextInput
        autoFocus
        value={typed}
        aria-label={label}
        aria-invalid={mutation.isError}
        onChange={(event) => {
          setTyped(event.target.value)
          mutation.reset()
        }}
        onBlur={save}
        onKeyDown={(event) => {
          if (event.key === "Enter") save()
          if (event.key === "Escape") onClose()
        }}
        className={cn("h-6.5", className)}
      />
      {mutation.isError && (
        <p role="alert" className="text-danger order-last basis-full text-xs">
          {errorText(mutation.error)}
        </p>
      )}
    </>
  )
}

function Palette({
  open,
  value,
  label,
  disabled,
  onToggle,
  onPick,
  onClose,
}: {
  open: boolean
  value: Color
  label: string
  disabled: boolean
  onToggle: () => void
  onPick: (color: Color) => void
  onClose: () => void
}) {
  const root = useRef<HTMLDivElement>(null)
  useDismissOnOutsideClick(root, open ? onClose : undefined)

  return (
    <div
      ref={root}
      className="relative"
      onKeyDown={(event) => {
        if (event.key === "Escape") onClose()
      }}
    >
      <Button
        aria-label={`Recolour ${label}`}
        aria-expanded={open}
        disabled={disabled}
        onClick={onToggle}
        className="px-1.5 py-1.5 disabled:opacity-40"
      >
        <ColorDot color={value} className="size-3.5" />
      </Button>
      {open && (
        <div
          ref={focusOnMount}
          tabIndex={-1}
          className="bg-popover absolute top-full left-0 z-30 mt-1.5 w-max rounded-md border p-2 shadow-md focus:outline-none"
        >
          <ColorPicker value={value} onPick={onPick} />
        </div>
      )}
    </div>
  )
}

const FORCE_COST = "The tag goes, and every task that still names it is left holding a dangling tag."

// The first attempt never carries `force`: the daemon is the only thing that knows how many tasks
// name the tag. A conflict alone does not say that force would change it — a delete another writer
// keeps moving under is refused again just the same — so the offer waits for the reason that names
// one.
function DeleteConfirm({
  project,
  tag,
  mutation,
  forcing,
  onRefused,
  onCancel,
}: {
  project: string
  tag: TagView
  mutation: ProjectMutation
  forcing: boolean
  onRefused: () => void
  onCancel: () => void
}) {
  const remove = () => {
    if (mutation.isPending) return
    mutation.mutate(deleteTag(project, tag.name, forcing), {
      onError: (error) => {
        if (!forcing && error instanceof TaskRejected && error.reason === "tag_referenced") onRefused()
      },
    })
  }
  const confirm = (
    <Button variant="danger" onClick={remove} disabled={mutation.isPending} className="text-danger disabled:opacity-40">
      <Check className="size-3.5" />
      {forcing ? "Delete anyway" : "Delete"}
    </Button>
  )
  return (
    <>
      <span className="text-muted-foreground text-xs">Delete {tag.display}?</span>
      {forcing ? <Tooltip content={FORCE_COST}>{confirm}</Tooltip> : confirm}
      <Button onClick={onCancel}>
        <X className="size-3.5" />
        Cancel
      </Button>
    </>
  )
}

// Registration leaves the colour out: the registry derives one from the name, and the row recolours
// in a click.
function TagForm({ project, mutation, className }: { project: string; mutation: ProjectMutation; className?: string }) {
  const [name, setName] = useState("")
  const [description, setDescription] = useState("")
  const named = name.trim()

  const submit = () => {
    if (named === "" || mutation.isPending) return
    const described = description.trim()
    mutation.mutate(createTag(project, { name: named, description: described === "" ? undefined : described }), {
      onSuccess: () => {
        setName("")
        setDescription("")
      },
    })
  }

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        submit()
      }}
      className={cn("flex flex-wrap items-center gap-2", className)}
    >
      <TextInput
        value={name}
        onChange={(event) => setName(event.target.value)}
        placeholder="Tag name"
        className="w-48 max-sm:w-full"
      />
      <TextInput
        value={description}
        onChange={(event) => setDescription(event.target.value)}
        placeholder="What it marks (optional)"
        className="min-w-0 flex-1 max-sm:basis-full"
      />
      <Button
        type="submit"
        variant="accent"
        size="md"
        disabled={named === "" || mutation.isPending}
        className="disabled:opacity-40"
      >
        <Plus className="size-3.5" />
        Register tag
      </Button>
    </form>
  )
}

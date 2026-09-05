import { useQuery } from "@tanstack/react-query"
import { Check, Pencil, Plus, Trash2, X } from "lucide-react"
import { useRef, useState } from "react"
import { Link, useParams } from "react-router-dom"

import type { Color, TagView } from "@openplan/api-client"
import { boardPath, ColorDot, ColorPicker, TagChip } from "@openplan/task-ui"
import {
  Button,
  cn,
  EmptyState,
  Panel,
  PanelBody,
  PanelHeader,
  PanelTitle,
  Row,
  SkeletonList,
  TextInput,
  Tooltip,
  useDismissOnOutsideClick,
} from "@openplan/ui"

import { createTag, deleteTag, listTags, patchTag, TaskRejected } from "../lib/api"
import { errorText } from "../lib/format"
import { demotedReason, useProject, useProjects } from "../lib/projects"
import { tagsKey, useProjectMutation } from "../lib/query-client"
import { useRowCursor } from "../lib/row-cursor"
import { runtime } from "../lib/runtime"

// This page holds no task rows, and the cursor is the board's — left as it was, `j` then Enter here
// would open a task the reader can no longer see.
const NO_ROWS: ReadonlyArray<string> = []
const focusOnMount = (element: HTMLDivElement | null) => element?.focus()
type ProjectMutation = ReturnType<typeof useProjectMutation>

export function TagsRoute() {
  const { project = "" } = useParams()
  const projects = useProjects()
  const known = useProject(project)
  const tags = useQuery({
    queryKey: tagsKey(project),
    queryFn: () => runtime.runPromise(listTags(project)),
  })
  const registration = useProjectMutation(project)
  useRowCursor(NO_ROWS)

  // Until the list arrives every name is equally plausible, so an unknown one is only unknown once
  // the daemon has answered.
  if (projects !== undefined && known === undefined) {
    return <EmptyState title="No such project" detail={project} />
  }
  const reason = demotedReason(known)
  if (reason !== undefined) {
    return <EmptyState title={`${project} is not being served`} detail={reason} />
  }
  return (
    <Panel>
      <PanelHeader className="gap-3">
        <PanelTitle>Tags</PanelTitle>
        <Link to={boardPath(project)} className="text-muted-foreground hover:text-foreground ml-auto text-xs">
          ← {project}
        </Link>
      </PanelHeader>
      <PanelBody className="p-6">
        {/* The read and the writes resolve the same worktree, so a registry that cannot be read is a
            registry that cannot be written either — offering the form would only produce a toast. */}
        {tags.isError ? (
          <EmptyState title="Could not load tags" detail={errorText(tags.error)} />
        ) : tags.isPending ? (
          <SkeletonList count={3} className="h-10 w-full" />
        ) : (
          <>
            <TagForm project={project} mutation={registration} className="mb-6" />
            {tags.data.length === 0 ? (
              <p className="text-muted-foreground text-sm">No tags yet. Register one above.</p>
            ) : (
              <ul>
                {tags.data.map((tag, index) => (
                  <li key={`${tag.name}:${tag.display}:${tag.description ?? ""}`}>
                    <TagRow project={project} tag={tag} last={index === tags.data.length - 1} />
                  </li>
                ))}
              </ul>
            )}
          </>
        )}
      </PanelBody>
    </Panel>
  )
}

type Editing = "no" | "naming" | "deleting" | "forcing" | "recolouring"

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
      {editing === "naming" ? (
        <TagForm
          project={project}
          tag={tag}
          mutation={mutation}
          onClose={() => setEditing("no")}
          className="min-w-0 flex-1"
        />
      ) : (
        <>
          <TagChip name={tag.name} tag={tag} />
          <span className="text-muted-foreground/70 font-mono text-xs">{tag.name}</span>
          {tag.description !== undefined && (
            <span className="text-muted-foreground min-w-0 truncate text-sm">{tag.description}</span>
          )}
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
              <>
                <Button aria-label={`Edit ${tag.display}`} onClick={() => setEditing("naming")}>
                  <Pencil className="size-3.5" />
                </Button>
                <Button
                  variant="danger"
                  aria-label={`Delete ${tag.display}`}
                  onClick={() => setEditing("deleting")}
                  className="text-danger/70 hover:text-danger"
                >
                  <Trash2 className="size-3.5" />
                </Button>
              </>
            )}
          </div>
        </>
      )}
    </Row>
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
// name the tag. A conflict alone does not say that force would change it — a delete the branch
// cannot take is refused again just the same — so the offer waits for the reason that names one.
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

// A rename rewrites the `tags:` of every task on this branch that names the tag, so it is sent only
// when the name really changed. Registration leaves the colour out: the registry derives one from
// the name, and the row recolours in a click.
function TagForm({
  project,
  tag,
  mutation,
  onClose,
  className,
}: {
  project: string
  tag?: TagView
  mutation: ProjectMutation
  onClose?: () => void
  className?: string
}) {
  const editing = tag !== undefined
  const [name, setName] = useState(tag?.display ?? "")
  const [description, setDescription] = useState(tag?.description ?? "")
  const named = name.trim()

  const submit = () => {
    if (named === "" || mutation.isPending) return
    const described = description.trim()
    if (tag === undefined) {
      mutation.mutate(createTag(project, { name: named, description: described === "" ? undefined : described }), {
        onSuccess: () => {
          setName("")
          setDescription("")
        },
      })
      return
    }
    mutation.mutate(
      patchTag(project, tag.name, {
        name: named === tag.display ? undefined : named,
        description: described === (tag.description ?? "") ? undefined : described === "" ? null : described,
      }),
      { onSuccess: onClose },
    )
  }

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        submit()
      }}
      onKeyDown={(event) => {
        if (event.key === "Escape") onClose?.()
      }}
      className={cn("flex flex-wrap items-center gap-2", className)}
    >
      <TextInput
        autoFocus={editing}
        value={name}
        onChange={(event) => setName(event.target.value)}
        placeholder="Tag name"
        className="w-48"
      />
      <TextInput
        value={description}
        onChange={(event) => setDescription(event.target.value)}
        placeholder="What it marks (optional)"
        className="min-w-0 flex-1"
      />
      <Button
        type="submit"
        variant="accent"
        disabled={named === "" || mutation.isPending}
        className="disabled:opacity-40"
      >
        {editing ? <Check className="size-3.5" /> : <Plus className="size-3.5" />}
        {editing ? "Save" : "Register tag"}
      </Button>
      {onClose !== undefined && (
        <Button onClick={onClose}>
          <X className="size-3.5" />
          Cancel
        </Button>
      )}
    </form>
  )
}

import { Check, Pencil, Plus, Trash2, X } from "lucide-react"
import { useEffect, useRef, useState } from "react"
import { Link, useParams } from "react-router-dom"

import type { Color, TagView } from "@open-planner/api-client"
import { boardPath, ColorDot, ColorPicker, TagChip } from "@open-planner/task-ui"
import {
  Button,
  EmptyState,
  Panel,
  PanelBody,
  PanelHeader,
  PanelTitle,
  Row,
  SkeletonList,
  TextInput,
  Tooltip,
} from "@open-planner/ui"

import { createTag, deleteTag, patchTag } from "../lib/api"
import { errorText } from "../lib/format"
import { demotedReason, useProject, useProjects } from "../lib/projects"
import { useRowCursor } from "../lib/row-cursor"
import { runMutation, tagsQuery, useQuery } from "../lib/store"

// This page holds no task rows, and the cursor is the board's — left as it was, `j` then Enter here
// would open a task the reader can no longer see.
const NO_ROWS: ReadonlyArray<string> = []

export function TagsRoute() {
  const { project = "" } = useParams()
  const projects = useProjects()
  const known = useProject(project)
  const tags = useQuery(tagsQuery(project))
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
        <NewTag project={project} />
        {tags._tag === "failure" ? (
          <EmptyState title="Could not load tags" detail={errorText(tags.error)} />
        ) : tags._tag === "loading" ? (
          <SkeletonList count={3} className="h-10 w-full" />
        ) : tags.value.length === 0 ? (
          <p className="text-muted-foreground text-sm">No tags yet. Register one above.</p>
        ) : (
          <ul>
            {tags.value.map((tag, index) => (
              <li key={tag.name}>
                <TagRow project={project} tag={tag} last={index === tags.value.length - 1} />
              </li>
            ))}
          </ul>
        )}
      </PanelBody>
    </Panel>
  )
}

// The colour is left out: the registry derives one from the name, and the row recolours in a click.
function NewTag({ project }: { project: string }) {
  const [name, setName] = useState("")
  const [description, setDescription] = useState("")
  const trimmed = name.trim()

  const submit = () => {
    if (trimmed === "") return
    const described = description.trim()
    void runMutation(
      project,
      createTag(project, { name: trimmed, description: described === "" ? undefined : described }),
    ).then((landed) => {
      if (!landed) return
      setName("")
      setDescription("")
    })
  }

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        submit()
      }}
      className="mb-6 flex flex-wrap items-center gap-2"
    >
      <TextInput
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
      <Button type="submit" variant="accent" disabled={trimmed === ""} className="disabled:opacity-40">
        <Plus className="size-3.5" />
        Register tag
      </Button>
    </form>
  )
}

type Editing = "no" | "naming" | "deleting" | "forcing"

function TagRow({ project, tag, last }: { project: string; tag: TagView; last: boolean }) {
  const [editing, setEditing] = useState<Editing>("no")
  const [recolouring, setRecolouring] = useState(false)
  // A refetch replaces the row's tag while a form stands open over the old one; close it rather than
  // let a save write yesterday's name back.
  useEffect(() => {
    setEditing("no")
    setRecolouring(false)
  }, [tag.display, tag.description])

  return (
    <Row variant="divided" last={last} className="flex flex-wrap items-center gap-3 px-2 py-2">
      <div className="relative">
        <Button
          aria-label={`Recolour ${tag.display}`}
          onClick={() => setRecolouring((open) => !open)}
          className="px-1.5 py-1.5"
        >
          <ColorDot color={tag.color} className="size-3.5" />
        </Button>
        {recolouring && (
          <div className="bg-popover absolute top-full left-0 z-30 mt-1.5 w-max rounded-md border p-2 shadow-md">
            <ColorPicker
              value={tag.color}
              onPick={(color: Color) => {
                setRecolouring(false)
                void runMutation(project, patchTag(project, tag.name, { color }))
              }}
            />
          </div>
        )}
      </div>
      {editing === "naming" ? (
        <TagForm project={project} tag={tag} onClose={() => setEditing("no")} />
      ) : (
        <>
          <TagChip name={tag.name} tag={tag} />
          <span className="text-muted-foreground/70 font-mono text-xs">{tag.name}</span>
          {tag.description !== undefined && (
            <span className="text-muted-foreground min-w-0 truncate text-sm">{tag.description}</span>
          )}
          <div className="ml-auto flex shrink-0 items-center gap-1">
            {editing === "no" ? (
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
            ) : (
              <DeleteConfirm
                project={project}
                tag={tag}
                forcing={editing === "forcing"}
                onRefused={() => setEditing("forcing")}
                onCancel={() => setEditing("no")}
              />
            )}
          </div>
        </>
      )}
    </Row>
  )
}

const FORCE_COST = "The tag goes, and every task that still names it is left holding a dangling tag."

// The first attempt never carries `force`: the daemon is the only thing that knows how many tasks
// name the tag, and its refusal — in the toast — is what earns the second, explicit button.
function DeleteConfirm({
  project,
  tag,
  forcing,
  onRefused,
  onCancel,
}: {
  project: string
  tag: TagView
  forcing: boolean
  onRefused: () => void
  onCancel: () => void
}) {
  const remove = (force: boolean) => {
    void runMutation(project, deleteTag(project, tag.name, force)).then((landed) => {
      if (!landed && !force) onRefused()
    })
  }
  const confirm = (
    <Button variant="danger" onClick={() => remove(forcing)} className="text-danger">
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
// when the name really changed.
function TagForm({ project, tag, onClose }: { project: string; tag: TagView; onClose: () => void }) {
  const [display, setDisplay] = useState(tag.display)
  const [description, setDescription] = useState(tag.description ?? "")
  const first = useRef<HTMLInputElement>(null)
  useEffect(() => first.current?.focus(), [])

  const save = () => {
    const named = display.trim()
    if (named === "") return
    const described = description.trim()
    void runMutation(
      project,
      patchTag(project, tag.name, {
        name: named === tag.display ? undefined : named,
        description: described === (tag.description ?? "") ? undefined : described === "" ? null : described,
      }),
    ).then((landed) => {
      if (landed) onClose()
    })
  }

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        save()
      }}
      onKeyDown={(event) => {
        if (event.key === "Escape") onClose()
      }}
      className="flex min-w-0 flex-1 flex-wrap items-center gap-2"
    >
      <TextInput
        ref={first}
        value={display}
        onChange={(event) => setDisplay(event.target.value)}
        placeholder="Tag name"
        className="w-48"
      />
      <TextInput
        value={description}
        onChange={(event) => setDescription(event.target.value)}
        placeholder="What it marks (optional)"
        className="min-w-0 flex-1"
      />
      <Button type="submit" variant="accent">
        <Check className="size-3.5" />
        Save
      </Button>
      <Button onClick={onClose}>
        <X className="size-3.5" />
        Cancel
      </Button>
    </form>
  )
}

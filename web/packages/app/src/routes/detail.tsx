import { Pencil, Plus, X } from "lucide-react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { Link, useNavigate, useParams, useSearchParams } from "react-router-dom"

import type { Comment, TaskChild, TaskDetail, TaskListItem } from "@open-planner/api-client"
import {
  boardPath,
  BranchSwitcher,
  CommentThread,
  createdOf,
  parentOf,
  ParentLink,
  problems,
  statusField,
  TaskBody,
  TaskIdentity,
  taskPath,
  TaskTimes,
} from "@open-planner/task-ui"
import {
  Button,
  type ComboOption,
  Combobox,
  EmptyState,
  MetaLine,
  Panel,
  PanelBody,
  PanelHeader,
  PanelTitle,
  Row,
  Section,
} from "@open-planner/ui"

import { Blocked } from "../components/blocked"
import { BodySkeleton, DetailSkeleton } from "../components/states"
import { TagsField } from "../components/tags-field"
import { createTask, patchTask, TaskNotFound } from "../lib/api"
import { hoveredRow } from "../lib/copy-target"
import { useDetailAction } from "../lib/detail-actions"
import { errorText } from "../lib/format"
import { useAbbreviation } from "../lib/projects"
import { subtaskCursor, useSubtaskCursor } from "../lib/row-cursor"
import { listItem, runMutation, taskQuery, tasksQuery, useQuery } from "../lib/store"
import { taskMatches } from "../lib/task-search"

const NO_TASKS: ReadonlyArray<TaskListItem> = []
const NO_CHILDREN: ReadonlyArray<TaskChild> = []
const NO_COMMENTS: ReadonlyArray<Comment> = []

// Where an edit of the shown version lands, and what to say when it can land nowhere. The daemon
// resolves the branch and reports whether a live worktree can take the write, so the page names that
// branch rather than guessing at one, and offers only the actions that can succeed.
interface WriteHere {
  readonly branch: string | undefined
  readonly blocked: string | undefined
}

function writeHere(task: TaskDetail | TaskListItem): WriteHere {
  const target = task.write_target
  if (target === undefined) {
    return { branch: undefined, blocked: "This repository has no branch to write to." }
  }
  return {
    branch: target.branch,
    blocked: target.writable ? undefined : `No writable worktree holds ${target.branch}, so this task cannot change.`,
  }
}

export function DetailRoute() {
  const { project = "", id = "" } = useParams()
  // The selected branch lives in the URL (`?branch=`), so it is shareable and resets to the
  // headline on navigation without a render lag; absent means the headline (current-worktree)
  // version.
  const [params, setParams] = useSearchParams()
  const branch = params.get("branch") ?? undefined
  const onSelect = (next: string | undefined) =>
    setParams(next === undefined ? {} : { branch: next }, { replace: true })

  const task = useQuery(useMemo(() => taskQuery(project, id, branch), [project, id, branch]))

  // Switching branch mints a fresh query that starts in `loading`. Keep the last loaded version on
  // screen while it resolves — a branch click updates the card in place instead of flashing the
  // skeleton. Only a first load, or a different task, falls back to the skeleton. The project is
  // half of what names a task, and this route does not remount when only the project changes, so
  // holding the id alone would show one project's task under another project's URL.
  const lastShown = useRef<{ project: string; id: string; value: TaskDetail } | null>(null)
  if (task._tag === "success") lastShown.current = { project, id, value: task.value }

  if (task._tag === "failure") {
    return task.error instanceof TaskNotFound ? (
      <NotFound project={project} id={task.error.id} />
    ) : (
      <EmptyState title="Could not load task" detail={errorText(task.error)} />
    )
  }
  const held = lastShown.current
  const shown = task._tag === "success" ? task.value : held?.project === project && held.id === id ? held.value : null
  // The list cache already holds the header fields (title, status, branches); seed from it so the
  // header renders instantly and only the body and hierarchy stream in.
  const seed = shown ?? listItem(project, id)
  if (seed === undefined) return <DetailSkeleton />
  return (
    <TaskDetailView
      project={project}
      task={seed}
      detail={shown}
      body={shown?.body}
      selected={branch}
      onSelect={onSelect}
    />
  )
}

function TaskDetailView({
  project,
  task,
  detail,
  body,
  selected,
  onSelect,
}: {
  project: string
  task: TaskDetail | TaskListItem
  detail: TaskDetail | null
  body: string | undefined
  selected: string | undefined
  onSelect: (branch: string | undefined) => void
}) {
  const abbreviation = useAbbreviation(project)
  const write = writeHere(detail ?? task)
  return (
    <Panel>
      <PanelHeader className="gap-2">
        <PanelTitle>
          <TaskIdentity variant="header" status={statusField(task.metadata)} id={task.id} title={task.title} />
        </PanelTitle>
        <div className="ml-auto min-w-0">
          <HeaderParent
            project={project}
            id={task.id}
            parent={detail === null ? undefined : parentOf(detail.metadata)}
            parentTitle={detail?.parent_title}
            ready={detail !== null}
            write={write}
          />
        </div>
      </PanelHeader>
      <PanelBody className="p-6">
        <h1 className="mb-1.5 text-2xl font-semibold tracking-tight">{task.title}</h1>
        {/* `created` arrives with the full detail while `updated` is already on the seeded list item,
            so the line renders as soon as the header does and fills in rather than shifting the body
            twice. */}
        <MetaLine className="mb-4 h-4">
          <TaskTimes
            created={detail === null ? undefined : createdOf(detail.metadata)}
            updated={task.updated}
            problems={detail === null ? [] : problems(detail.metadata)}
          />
        </MetaLine>
        <TagsField
          project={project}
          id={task.id}
          metadata={task.metadata}
          branch={write.branch}
          blocked={write.blocked}
          className="mb-4"
        />
        <BranchSwitcher branches={task.branches} selected={selected} headline={task.headline} onSelect={onSelect} />
        {body === undefined ? (
          <BodySkeleton />
        ) : (
          <TaskBody
            project={project}
            markdown={stripTitle(body)}
            refs={detail?.refs}
            abbreviation={abbreviation}
            data-keys-ignore
          />
        )}
        <SubtasksSection
          project={project}
          id={task.id}
          items={detail?.children ?? NO_CHILDREN}
          ready={detail !== null}
          write={write}
        />
        {detail !== null && (
          <CommentThread
            project={project}
            comments={detail.comments ?? NO_COMMENTS}
            refs={detail.refs}
            abbreviation={abbreviation}
          />
        )}
      </PanelBody>
    </Panel>
  )
}

// The ids of `start`'s ancestors (parent, grandparent, …), cycle-safe. Making any of them a child of
// `start` would close a loop, so a subtask picker excludes them.
function ancestorIds(tasks: ReadonlyArray<TaskListItem>, start: string): Set<string> {
  const byId = new Map(tasks.map((task) => [task.id, task]))
  const out = new Set<string>()
  const parentOfId = (id: string) => {
    const found = byId.get(id)
    return found === undefined ? undefined : parentOf(found.metadata)
  }
  let cursor = parentOfId(start)
  while (cursor !== undefined && byId.has(cursor) && !out.has(cursor)) {
    out.add(cursor)
    cursor = parentOfId(cursor)
  }
  return out
}

// The ids of `root`'s whole subtree, cycle-safe. Reparenting `root` under any of them would close a
// loop, so the parent picker excludes them.
function descendantIds(tasks: ReadonlyArray<TaskListItem>, root: string): Set<string> {
  const children = new Map<string, string[]>()
  for (const task of tasks) {
    const parent = parentOf(task.metadata)
    if (parent !== undefined) {
      const bucket = children.get(parent)
      if (bucket === undefined) children.set(parent, [task.id])
      else bucket.push(task.id)
    }
  }
  const out = new Set<string>()
  const stack = [root]
  while (stack.length > 0) {
    const current = stack.pop()!
    for (const child of children.get(current) ?? []) {
      if (!out.has(child)) {
        out.add(child)
        stack.push(child)
      }
    }
  }
  return out
}

function ComboTaskRow({ task, indices }: { task: TaskListItem; indices: ReadonlyArray<number> }) {
  return <TaskIdentity status={statusField(task.metadata)} id={task.id} title={task.title} indices={indices} />
}

// The parent as a header-right "Subtask of <link>", retargetable in place. Clicking the pencil (or
// pressing `p`) swaps the link for the shared search field; `g p` jumps to the parent.
function HeaderParent({
  project,
  id,
  parent,
  parentTitle,
  ready,
  write,
}: {
  project: string
  id: string
  parent: string | undefined
  parentTitle: string | undefined
  ready: boolean
  write: WriteHere
}) {
  const navigate = useNavigate()
  const [editing, setEditing] = useState(false)
  useDetailAction("edit-parent", () => {
    if (write.blocked === undefined) setEditing(true)
  })
  // A refresh can take the write target away while the picker stands open — another worktree took
  // the branch, or a merge started there. Close it rather than leave a control that cannot land.
  useEffect(() => {
    if (write.blocked !== undefined) setEditing(false)
  }, [write.blocked])
  useDetailAction("go-parent", () => {
    if (parent !== undefined && parentTitle !== undefined) navigate(taskPath(project, parent))
  })

  if (editing) {
    return <ParentPicker project={project} id={id} branch={write.branch} onClose={() => setEditing(false)} />
  }
  // The parent is unknown until the detail loads; show nothing rather than a misleading "Set parent".
  if (!ready) return null
  const hasParent = parent !== undefined
  return (
    <div className="flex min-w-0 items-center gap-1">
      {parentTitle !== undefined && parent !== undefined ? (
        <MetaLine>
          <ParentLink project={project} id={parent} title={parentTitle} />
        </MetaLine>
      ) : hasParent ? (
        <span className="text-muted-foreground/70 text-xs italic">parent missing</span>
      ) : null}
      {write.blocked !== undefined ? (
        <Blocked reason={write.blocked} />
      ) : (
        <Button
          onClick={() => setEditing(true)}
          aria-label={hasParent ? "Change parent" : "Set parent"}
          className="gap-1 px-1.5"
        >
          {hasParent ? (
            <Pencil className="size-3.5" />
          ) : (
            <>
              <Plus className="size-3.5" />
              Set parent
            </>
          )}
        </Button>
      )}
    </div>
  )
}

// The full task list is needed only to search for a new parent, so it is fetched here — when the
// picker opens — rather than on every detail view. Excludes self + descendants so a pick can't cycle.
function ParentPicker({
  project,
  id,
  branch,
  onClose,
}: {
  project: string
  id: string
  branch: string | undefined
  onClose: () => void
}) {
  const tasks = useQuery(tasksQuery(project))
  useEffect(() => {
    tasksQuery(project).refresh()
  }, [project])
  const all = tasks._tag === "success" ? tasks.value : NO_TASKS

  const buildOptions = useCallback(
    (query: string): ReadonlyArray<ComboOption> => {
      const self = all.find((task) => task.id === id)
      const excluded = descendantIds(all, id)
      excluded.add(id)
      const options: ComboOption[] = []
      if (self !== undefined && parentOf(self.metadata) !== undefined) {
        options.push({
          key: " clear",
          content: (
            <span className="text-muted-foreground flex items-center gap-2">
              <X className="size-4" />
              Top level (no parent)
            </span>
          ),
          onSelect: () => void runMutation(project, patchTask(project, id, { parent: null }, branch)),
        })
      }
      for (const { task, indices } of taskMatches(all, query, excluded)) {
        options.push({
          key: task.id,
          content: <ComboTaskRow task={task} indices={indices} />,
          onSelect: () => void runMutation(project, patchTask(project, id, { parent: task.id }, branch)),
        })
      }
      return options
    },
    [all, project, id, branch],
  )

  return (
    <Combobox
      placeholder="Change parent…"
      buildOptions={buildOptions}
      onClose={onClose}
      emptyLabel="No matching task"
      className="w-72"
    />
  )
}

// The direct children below the task body, plus an inline add box that either pulls an existing task
// in as a child or creates a fresh one.
function SubtasksSection({
  project,
  id,
  items,
  ready,
  write,
}: {
  project: string
  id: string
  items: ReadonlyArray<TaskChild>
  ready: boolean
  write: WriteHere
}) {
  const [adding, setAdding] = useState(false)
  useDetailAction("add-subtask", () => {
    if (write.blocked === undefined) setAdding(true)
  })
  useEffect(() => {
    if (write.blocked !== undefined) setAdding(false)
  }, [write.blocked])

  const childPaths = useMemo(() => items.map((child) => taskPath(project, child.id)), [project, items])
  const { index } = useSubtaskCursor(taskPath(project, id), childPaths)
  const activeRow = useRef<HTMLLIElement>(null)
  useEffect(() => {
    activeRow.current?.scrollIntoView({ block: "nearest" })
  }, [index])

  return (
    <Section
      title="Subtasks"
      count={items.length}
      action={
        write.blocked !== undefined ? (
          <Blocked reason={write.blocked} />
        ) : (
          <Button variant="accent" onClick={() => setAdding((open) => !open)}>
            <Plus className="size-3.5" />
            Add subtask
          </Button>
        )
      }
    >
      {adding && (
        <div className="mb-3">
          <SubtaskPicker project={project} id={id} branch={write.branch} onClose={() => setAdding(false)} />
        </div>
      )}
      {items.length === 0 ? (
        ready ? (
          <p className="text-muted-foreground text-sm">No subtasks yet.</p>
        ) : null
      ) : (
        <ul
          className="space-y-0.5"
          onMouseMove={() => {
            if (index !== -1) subtaskCursor.clear()
          }}
          onMouseLeave={hoveredRow.clear}
        >
          {items.map((child, i) => (
            <li
              key={child.id}
              ref={i === index ? activeRow : undefined}
              aria-selected={i === index}
              onMouseMove={() => hoveredRow.enter(childPaths[i])}
              onMouseLeave={() => hoveredRow.leave(childPaths[i])}
            >
              <Row
                as={Link}
                variant="option"
                active={i === index}
                hoverable
                to={childPaths[i]}
                onClick={() => subtaskCursor.focus(i)}
              >
                <TaskIdentity status={child.status} id={child.id} title={child.title} />
              </Row>
            </li>
          ))}
        </ul>
      )}
    </Section>
  )
}

// Opened on demand, so the full task list it searches is fetched only when adding — not per detail
// view. Making a task a child of `id` closes a cycle only when that task is an ancestor of `id`, so
// exclude the ancestor chain (and self); descendants are valid re-parent targets.
function SubtaskPicker({
  project,
  id,
  branch,
  onClose,
}: {
  project: string
  id: string
  branch: string | undefined
  onClose: () => void
}) {
  const tasks = useQuery(tasksQuery(project))
  useEffect(() => {
    tasksQuery(project).refresh()
  }, [project])
  const all = tasks._tag === "success" ? tasks.value : NO_TASKS

  const buildOptions = useCallback(
    (query: string): ReadonlyArray<ComboOption> => {
      const excluded = ancestorIds(all, id)
      excluded.add(id)
      const options: ComboOption[] = []
      if (query !== "") {
        options.push({
          key: " create",
          content: (
            <span className="flex items-center gap-2">
              <Plus className="text-muted-foreground size-4" />
              <span>
                Create <span className="font-medium">“{query}”</span> as a new subtask
              </span>
            </span>
          ),
          onSelect: () => void runMutation(project, createTask(project, { title: query, parent: id }, branch)),
        })
      }
      for (const { task, indices } of taskMatches(all, query, excluded)) {
        if (parentOf(task.metadata) === id) continue
        options.push({
          key: task.id,
          content: <ComboTaskRow task={task} indices={indices} />,
          // The parent's branch, not the child's: a parent the child's branch does not carry is no
          // parent at all there, so the pair has to move on the branch that holds the parent.
          onSelect: () => void runMutation(project, patchTask(project, task.id, { parent: id }, branch)),
        })
      }
      return options
    },
    [all, project, id, branch],
  )

  return (
    <Combobox
      placeholder="Find a task or type a new subtask title…"
      buildOptions={buildOptions}
      onClose={onClose}
      emptyLabel="Type a title to create a subtask"
      className="max-w-md"
      inline
    />
  )
}

function NotFound({ project, id }: { project: string; id: string }) {
  return (
    <div className="space-y-4">
      <Link to={boardPath(project)} className="text-muted-foreground text-sm hover:underline">
        ← {project}
      </Link>
      <EmptyState title="Task not found" detail={id} />
    </div>
  )
}

function stripTitle(body: string): string {
  const lines = body.split("\n")
  const first = lines.findIndex((line) => line.trim().length > 0)
  if (first >= 0 && lines[first].startsWith("# ")) {
    lines.splice(first, 1)
  }
  return lines.join("\n").trim()
}

import { keepPreviousData, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query"
import { Pencil, Plus, X } from "lucide-react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { Link, useNavigate, useParams, useSearchParams } from "react-router-dom"

import type { Board, Comment, TaskDetail, TaskListItem } from "@open-planner/api-client"
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
  UnresolvedMark,
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
import { createTask, getTask, listTasks, patchTask, TaskNotFound } from "../lib/api"
import { hoveredRow } from "../lib/copy-target"
import { useDetailAction } from "../lib/detail-actions"
import { type DetailRow, detailRows } from "../lib/detail-rows"
import { errorText } from "../lib/format"
import { useAbbreviation } from "../lib/projects"
import { boardKey, mergedBoardKey, taskKey, tasksKey, useProjectMutation } from "../lib/query-client"
import { detailCursor, useDetailCursor } from "../lib/row-cursor"
import { runtime } from "../lib/runtime"
import { taskMatches } from "../lib/task-search"

const NO_TASKS: ReadonlyArray<TaskListItem> = []
const NO_COMMENTS: ReadonlyArray<Comment> = []

function boardTasks(board: Board): ReadonlyArray<TaskListItem> {
  return board.groups.flatMap((group) => group.rows.map((row) => row.task))
}

function listItem(client: QueryClient, project: string, id: string): TaskListItem | undefined {
  for (const key of [mergedBoardKey, boardKey(project)]) {
    const board = client.getQueryData<Board>(key)
    const found =
      board === undefined ? undefined : boardTasks(board).find((task) => task.project === project && task.id === id)
    if (found !== undefined) return found
  }
  return undefined
}

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
  return <TaskRoute key={`${project}:${id}`} project={project} id={id} />
}

function TaskRoute({ project, id }: { project: string; id: string }) {
  const client = useQueryClient()
  // The selected branch lives in the URL (`?branch=`), so it is shareable and resets to the
  // headline on navigation without a render lag; absent means the headline (current-worktree)
  // version.
  const [params, setParams] = useSearchParams()
  const branch = params.get("branch") ?? undefined
  const onSelect = (next: string | undefined) =>
    setParams(next === undefined ? {} : { branch: next }, { replace: true })

  const task = useQuery({
    queryKey: taskKey(project, id, branch),
    queryFn: () => runtime.runPromise(getTask(project, id, branch)),
    placeholderData: keepPreviousData,
  })

  if (task.isError) {
    return task.error instanceof TaskNotFound ? (
      <NotFound project={project} id={task.error.id} />
    ) : (
      <EmptyState title="Could not load task" detail={errorText(task.error)} />
    )
  }
  const shown = task.data ?? null
  // The list cache already holds the header fields (title, status, branches); seed from it so the
  // header renders instantly and only the body and hierarchy stream in.
  const seed = shown ?? listItem(client, project, id)
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
  const writeKey = `${write.branch ?? ""}:${write.blocked ?? ""}`
  // One cursor walks the three lists in document order, so `j`, `k` and Enter reach every row on the
  // page. Each section renders a slice of it and offsets its own rows into it.
  const rows = useMemo(() => detailRows(project, detail), [project, detail])
  const { index } = useDetailCursor(taskPath(project, task.id), rows.paths)
  return (
    // Each column scrolls on its own, so the box keeps its frame and its header stays where it is
    // while the body runs. Stacked, the two are one page and the page scrolls instead.
    <div className="flex h-full flex-col gap-4 overflow-y-auto lg:flex-row lg:overflow-hidden">
      <Panel className="h-auto min-w-0 lg:h-full lg:w-[59rem]">
        <PanelHeader className="gap-2">
          <PanelTitle>
            <TaskIdentity variant="header" status={statusField(task.metadata)} id={task.id} title={task.title} />
          </PanelTitle>
          <div className="ml-auto min-w-0">
            <HeaderParent
              key={writeKey}
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
          {/* The tags sit level with the first line of the title: the row aligns to the top, and the
              chips centre inside a box as tall as that line. They wrap inside half the row rather
              than holding their width — a task carrying a handful of them squeezed the title to
              nothing. */}
          <div className="mb-1.5 flex items-start justify-between gap-4">
            <h1 className="min-w-0 text-2xl font-semibold tracking-tight">{task.title}</h1>
            <TagsField
              key={writeKey}
              project={project}
              id={task.id}
              metadata={task.metadata}
              branch={write.branch}
              blocked={write.blocked}
              className="min-h-8 max-w-[50%] justify-end"
            />
          </div>
          {/* `created` arrives with the full detail while `updated` is already on the seeded list
              item, so the line renders as soon as the header does and fills in rather than shifting
              the body twice. */}
          <MetaLine className="mb-4 h-4">
            <TaskTimes
              created={detail === null ? undefined : createdOf(detail.metadata)}
              updated={task.updated}
              problems={detail === null ? [] : problems(detail.metadata)}
            />
          </MetaLine>
          <BranchSwitcher branches={task.branches} selected={selected} headline={task.headline} onSelect={onSelect} />
          {/* The box is as wide as the reading measure, so the text fills it and the rule over an
              `h2` bleeds back over the padding to divide the whole box. */}
          {body === undefined ? (
            <BodySkeleton />
          ) : (
            <TaskBody
              project={project}
              markdown={stripTitle(body)}
              refs={detail?.refs}
              abbreviation={abbreviation}
              className="prose-h2:-mx-6 prose-h2:px-6"
              data-keys-ignore
            />
          )}
        </PanelBody>
      </Panel>
      {/* The relations and the comment log stand beside the task and share the width it leaves.
          Narrow enough and they drop under it instead. None of them wears a frame: a section leads
          with the rule that separates it from the one above, and the first has nothing above it to
          separate from. */}
      <aside className="min-w-0 lg:min-w-80 lg:flex-1 lg:overflow-y-auto [&>section:first-child]:mt-0 [&>section:first-child]:border-t-0 [&>section:first-child]:pt-0">
        <RefSection title="Depends on" rows={rows.dependsOn} cursor={index} />
        <RefSection title="Blocks" rows={rows.blocks} cursor={index} />
        <SubtasksSection
          key={writeKey}
          project={project}
          id={task.id}
          rows={rows.subtasks}
          cursor={index}
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
      </aside>
    </div>
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
  const tasks = useQuery({
    queryKey: tasksKey(project),
    queryFn: () => runtime.runPromise(listTasks(project)),
    refetchOnMount: "always",
  })
  const { mutate } = useProjectMutation(project)
  const all = tasks.data ?? NO_TASKS

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
          onSelect: () => mutate(patchTask(project, id, { parent: null }, branch)),
        })
      }
      for (const { task, indices } of taskMatches(all, query, excluded)) {
        options.push({
          key: task.id,
          content: <ComboTaskRow task={task} indices={indices} />,
          onSelect: () => mutate(patchTask(project, id, { parent: task.id }, branch)),
        })
      }
      return options
    },
    [all, project, id, branch, mutate],
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

// One list's rows, each carrying its own place in the page-wide cursor, so the sections agree on
// nothing but the order `detailRows` numbered them in.
function RowList({ rows, cursor }: { rows: ReadonlyArray<DetailRow>; cursor: number }) {
  const activeRow = useRef<HTMLLIElement>(null)
  useEffect(() => {
    activeRow.current?.scrollIntoView({ block: "nearest" })
  }, [cursor])

  return (
    <ul
      className="space-y-0.5"
      onMouseMove={() => {
        if (cursor !== -1) detailCursor.clear()
      }}
      onMouseLeave={hoveredRow.clear}
    >
      {rows.map((row) => (
        // A file may name the same dependency twice, so a row's place is the only unique key.
        <li
          key={row.at}
          ref={row.at === cursor ? activeRow : undefined}
          aria-selected={row.at === cursor}
          onMouseMove={() => hoveredRow.enter(row.path, row.at)}
          onMouseLeave={() => hoveredRow.leave(row.path, row.at)}
        >
          <Row
            as={Link}
            variant="option"
            active={row.at === cursor}
            hoverable
            to={row.path}
            onClick={() => detailCursor.focus(row.at)}
          >
            <TaskIdentity
              status={row.status}
              mark={row.unresolved ? <UnresolvedMark /> : undefined}
              id={row.id}
              title={row.title}
            />
          </Row>
        </li>
      ))}
    </ul>
  )
}

// The two dependency directions. Neither carries an action, so an empty one has nothing to say and
// stays hidden.
function RefSection({ title, rows, cursor }: { title: string; rows: ReadonlyArray<DetailRow>; cursor: number }) {
  if (rows.length === 0) return null
  return (
    <Section title={title} count={rows.length}>
      <RowList rows={rows} cursor={cursor} />
    </Section>
  )
}

// The direct children below the task body, plus an inline add box that either pulls an existing task
// in as a child or creates a fresh one.
function SubtasksSection({
  project,
  id,
  rows,
  cursor,
  ready,
  write,
}: {
  project: string
  id: string
  rows: ReadonlyArray<DetailRow>
  cursor: number
  ready: boolean
  write: WriteHere
}) {
  const [adding, setAdding] = useState(false)
  useDetailAction("add-subtask", () => {
    if (write.blocked === undefined) setAdding(true)
  })

  return (
    <Section
      title="Subtasks"
      count={rows.length}
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
      {rows.length === 0 ? (
        ready ? (
          <p className="text-muted-foreground text-sm">No subtasks yet.</p>
        ) : null
      ) : (
        <RowList rows={rows} cursor={cursor} />
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
  const tasks = useQuery({
    queryKey: tasksKey(project),
    queryFn: () => runtime.runPromise(listTasks(project)),
    refetchOnMount: "always",
  })
  const { mutate } = useProjectMutation(project)
  const all = tasks.data ?? NO_TASKS

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
          onSelect: () => mutate(createTask(project, { title: query, parent: id }, branch)),
        })
      }
      for (const { task, indices } of taskMatches(all, query, excluded)) {
        if (parentOf(task.metadata) === id) continue
        options.push({
          key: task.id,
          content: <ComboTaskRow task={task} indices={indices} />,
          // The parent's branch, not the child's: a parent the child's branch does not carry is no
          // parent at all there, so the pair has to move on the branch that holds the parent.
          onSelect: () => mutate(patchTask(project, task.id, { parent: id }, branch)),
        })
      }
      return options
    },
    [all, project, id, branch, mutate],
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

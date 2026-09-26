import { useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query"
import { Pencil, Plus, Undo2, Waypoints, X } from "lucide-react"
import { type MouseEvent, type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react"
import { Link, useLocation, useNavigate, useParams, useSearchParams } from "react-router-dom"

import type {
  Board,
  Comment,
  HistoryEntry,
  Metadata,
  TaskDetail,
  TaskListItem,
  TaskSnapshot,
} from "@openplan/api-client"
import {
  bodySegments,
  boardPath,
  CommentThread,
  createdOf,
  fieldConflict,
  frontmatterFields,
  parentOf,
  ParentLink,
  ProblemBanner,
  problems,
  REVISION_PARAM,
  RevisionMeta,
  shortRevision,
  statusField,
  TaskBodyWithConflicts,
  TaskChangeView,
  TaskAuthor,
  TaskIdentity,
  taskPath,
  TaskTags,
  TaskTimes,
  UnresolvedMark,
} from "@openplan/task-ui"
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
} from "@openplan/ui"

import { ConflictBanner, FieldConflictControl } from "../components/field-conflict"
import { BodySkeleton, DetailSkeleton } from "../components/states"
import { StatusControl } from "../components/status-control"
import { TagsField } from "../components/tags-field"
import { TaskContent } from "../components/task-content"
import { TaskHistory } from "../components/task-history"
import { createTask, getTask, listTasks, patchTask, TaskNotFound } from "../lib/api"
import { useDetailAction } from "../lib/detail-actions"
import { type DetailRow, detailRows } from "../lib/detail-rows"
import { taskFlowPath } from "../lib/flow-selection"
import { errorText } from "../lib/format"
import { taskChangeOf, useTaskHistory, useTaskRevision } from "../lib/history"
import { useAbbreviation } from "../lib/projects"
import { boardKey, mergedBoardKey, taskKey, tasksKey, useProjectMutation } from "../lib/query-client"
import { isOverLiveTask } from "../lib/revision-navigation"
import { detailCursor, useDetailCursor } from "../lib/row-cursor"
import { hoveredRow } from "../lib/row-target"
import { abortable } from "../lib/runtime"
import { NO_ROW } from "../lib/status-requests"
import { useTags } from "../lib/tags"
import { taskMatches } from "../lib/task-search"

const NO_TASKS: ReadonlyArray<TaskListItem> = []
const NO_COMMENTS: ReadonlyArray<Comment> = []
const NO_ROWS: ReadonlyArray<string> = []

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

export function DetailRoute() {
  const { project = "", id = "" } = useParams()
  return <TaskRoute key={`${project}:${id}`} project={project} id={id} />
}

function TaskRoute({ project, id }: { project: string; id: string }) {
  const [params] = useSearchParams()
  const revision = params.get(REVISION_PARAM)
  return revision === null ? (
    <LiveTask project={project} id={id} />
  ) : (
    <TaskAtRevision project={project} id={id} revision={revision} />
  )
}

function LiveTask({ project, id }: { project: string; id: string }) {
  const client = useQueryClient()
  const task = useQuery({
    queryKey: taskKey(project, id),
    queryFn: abortable(getTask(project, id)),
  })

  if (task.isError) {
    return task.error instanceof TaskNotFound ? (
      <NotFound project={project} id={task.error.id} />
    ) : (
      <EmptyState title="Could not load task" detail={errorText(task.error)} />
    )
  }
  const shown = task.data ?? null
  // The list cache already holds the header fields (title, status, tags); seed from it so the header
  // renders instantly and only the description and hierarchy stream in.
  const seed = shown ?? listItem(client, project, id)
  if (seed === undefined) return <DetailSkeleton />
  return <TaskDetailView project={project} task={seed} detail={shown} />
}

function TaskDetailView({
  project,
  task,
  detail,
}: {
  project: string
  task: TaskDetail | TaskListItem
  detail: TaskDetail | null
}) {
  const abbreviation = useAbbreviation(project)
  // One cursor walks the three lists in document order, so `j`, `k` and Enter reach every row on the
  // page. Each section renders a slice of it and offsets its own rows into it.
  const rows = useMemo(() => detailRows(project, detail), [project, detail])
  const { index } = useDetailCursor(taskPath(project, task.id), rows.paths)
  // `created` arrives with the full detail while `updated` is already on the seeded list item, so the
  // line renders as soon as the header does and fills in rather than shifting the body twice.
  const meta = (saveNote: ReactNode) => (
    <TimesAndTags>
      <MetaLine className={timesLine}>
        <TaskAuthor author={task.author} withAgent />
        <TaskTimes
          created={detail === null ? undefined : createdOf(detail.metadata)}
          updated={task.updated}
          problems={detail === null ? [] : problems(detail.metadata)}
        />
        <FieldConflictControl project={project} id={task.id} metadata={task.metadata} field="created" />
        {saveNote}
      </MetaLine>
      <TagsField project={project} id={task.id} metadata={task.metadata} className={tagsBox} />
    </TimesAndTags>
  )
  return (
    <DetailColumns
      main={
        <>
          <PanelHeader className="gap-2">
            <PanelTitle>
              <TaskIdentity
                variant="header"
                status={statusField(task.metadata)}
                mark={
                  <StatusControl
                    project={project}
                    id={task.id}
                    at={NO_ROW}
                    status={statusField(task.metadata)}
                    className="size-5"
                  />
                }
                id={task.id}
                title={task.title}
              />
            </PanelTitle>
            <FieldConflictControl
              project={project}
              id={task.id}
              metadata={task.metadata}
              field="status"
              trigger="Status conflict"
            />
            <FlowAction project={project} id={task.id} />
            <div className="flex min-w-0 items-center gap-1.5">
              <FieldConflictControl
                project={project}
                id={task.id}
                metadata={task.metadata}
                field="parent"
                trigger="Parent conflict"
                align="end"
              />
              <HeaderParent
                project={project}
                id={task.id}
                parent={detail === null ? undefined : parentOf(detail.metadata)}
                parentTitle={detail?.parent_title}
                ready={detail !== null}
              />
            </div>
          </PanelHeader>
          <PanelBody className="p-6">
            <ConflictBanner project={project} id={task.id} metadata={task.metadata} count={task.conflicts} />
            <ProblemBanner problems={task.problems} />
            {detail === null || abbreviation === undefined ? (
              <>
                <TaskTitle title={task.title} />
                {meta(null)}
                <BodySkeleton />
              </>
            ) : (
              <TaskContent
                project={project}
                id={task.id}
                title={detail.title}
                description={detail.description}
                refs={detail.refs}
                abbreviation={abbreviation}
                meta={meta}
              />
            )}
          </PanelBody>
        </>
      }
      aside={
        <>
          <RefSection
            project={project}
            title="Depends on"
            rows={rows.dependsOn}
            cursor={index}
            action={
              <FieldConflictControl
                project={project}
                id={task.id}
                metadata={task.metadata}
                field="dependencies"
                align="end"
              />
            }
            shown={dependenciesConflict(task.metadata)}
          />
          <RefSection project={project} title="Blocks" rows={rows.blocks} cursor={index} />
          <SubtasksSection project={project} id={task.id} rows={rows.subtasks} cursor={index} ready={detail !== null} />
          {detail !== null && (
            <CommentThread
              project={project}
              comments={detail.comments ?? NO_COMMENTS}
              refs={detail.refs}
              abbreviation={abbreviation}
            />
          )}
          {detail !== null && <TaskHistory project={project} id={task.id} selected={undefined} />}
        </>
      }
    />
  )
}

// Each column scrolls on its own, so the box keeps its frame and its header stays where it is while
// the body runs. Stacked, the two are one page and the page scrolls instead. The aside holds what
// stands beside the task and shares the width it leaves; narrow enough and it drops under it instead.
// No section in it wears a frame: a section leads with the rule that separates it from the one above,
// and the first has nothing above it to separate from.
function DetailColumns({ main, aside }: { main: ReactNode; aside: ReactNode }) {
  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto lg:flex-row lg:overflow-hidden">
      <Panel className="dark:[--surface:color-mix(in_srgb,var(--muted)_25%,var(--background))] h-auto min-w-0 lg:h-full lg:w-[59rem]">
        {main}
      </Panel>
      <aside className="min-w-0 lg:min-w-80 lg:flex-1 lg:overflow-y-auto [&>section:first-child]:mt-0 [&>section:first-child]:border-t-0 [&>section:first-child]:pt-0">
        {aside}
      </aside>
    </div>
  )
}

// A revision never changes, so nothing here can be edited: the marks carry no menu and the fields no
// controls.
function TaskAtRevision({ project, id, revision }: { project: string; id: string; revision: string }) {
  const snapshot = useTaskRevision(project, id, revision)
  const entry = useTaskHistory(project, id).data?.find((one) => one.revision.id === revision)
  const task = snapshot.data?.task
  // The page lists no rows, and the cursor would otherwise keep the ones the live task listed.
  useDetailCursor(`${taskPath(project, id)}@${revision}`, NO_ROWS)
  return (
    <DetailColumns
      main={
        <>
          <PanelHeader className="gap-2">
            <PanelTitle>
              <TaskIdentity
                variant="header"
                status={task === undefined ? undefined : statusField(task.metadata)}
                mark={task === undefined ? <UnresolvedMark /> : undefined}
                id={id}
                title={task?.title}
              />
            </PanelTitle>
            <CurrentVersionLink project={project} id={id} />
          </PanelHeader>
          <PanelBody className="p-6">
            <RevisionNotice project={project} id={id} revision={revision} entry={entry} />
            {snapshot.isPending ? (
              <BodySkeleton />
            ) : snapshot.isError ? (
              <EmptyState title="Could not load this revision" detail={errorText(snapshot.error)} />
            ) : task === undefined ? (
              <EmptyState title="The task did not exist at this revision" detail={id} />
            ) : (
              <Snapshot project={project} id={id} task={task} entry={entry} />
            )}
          </PanelBody>
        </>
      }
      aside={<TaskHistory project={project} id={id} selected={revision} />}
    />
  )
}

// The current version takes the place of the revisions, as they take the place of each other. Where
// the reader opened them from the current version, a Back returns to its entry.
function CurrentVersionLink({ project, id }: { project: string; id: string }) {
  const navigate = useNavigate()
  const { state } = useLocation()
  const back = (event: MouseEvent<HTMLAnchorElement>) => {
    if (!isOverLiveTask(state) || event.button !== 0) return
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
    event.preventDefault()
    navigate(-1)
  }
  return (
    <Link
      to={taskPath(project, id)}
      replace
      onClick={back}
      className="text-muted-foreground hover:text-foreground ml-auto inline-flex shrink-0 items-center gap-1.5 text-xs"
    >
      <Undo2 className="size-3.5" />
      Current version
    </Link>
  )
}

function RevisionNotice({
  project,
  id,
  revision,
  entry,
}: {
  project: string
  id: string
  revision: string
  entry: HistoryEntry | undefined
}) {
  const change = entry === undefined ? undefined : taskChangeOf(entry, id)
  const { byName: tags } = useTags(project)
  return (
    <div role="note" className="border-info/40 bg-info/5 mb-5 flex flex-col gap-1 rounded-md border px-3 py-2 text-xs">
      <p className="text-info">
        This is the task as revision <span className="font-mono">{shortRevision(revision)}</span> left it. You cannot
        change it here.
      </p>
      {entry !== undefined && (
        <>
          {change !== undefined && (
            <TaskChangeView change={change} tags={tags} className="text-foreground/90 text-sm" />
          )}
          <RevisionMeta revision={entry.revision} />
        </>
      )}
    </div>
  )
}

function Snapshot({
  project,
  id,
  task,
  entry,
}: {
  project: string
  id: string
  task: TaskSnapshot
  entry: HistoryEntry | undefined
}) {
  const abbreviation = useAbbreviation(project)
  const { byName: tags } = useTags(project)
  // The revision names tasks by key alone. The live task has them resolved to a title and a status,
  // which is the best a chip can show; a key it no longer holds reads as unresolved.
  const refs = useQueryClient().getQueryData<TaskDetail>(taskKey(project, id))?.refs
  return (
    <>
      <TaskTitle title={task.title} />
      <TimesAndTags>
        <MetaLine className={timesLine}>
          <TaskTimes
            created={createdOf(task.metadata)}
            updated={entry?.revision.at}
            problems={problems(task.metadata)}
          />
        </MetaLine>
        <TaskTags metadata={task.metadata} tags={tags} className={tagsBox} />
      </TimesAndTags>
      <TaskBodyWithConflicts
        segments={bodySegments(task.description)}
        project={project}
        refs={refs}
        abbreviation={abbreviation}
        proseClassName={PROSE}
        data-keys-ignore
      />
      {task.comments !== undefined && (
        <CommentThread project={project} comments={task.comments} refs={refs} abbreviation={abbreviation} />
      )}
    </>
  )
}

// The flow of this task alone, which grows to hold everything the task waits for.
function FlowAction({ project, id }: { project: string; id: string }) {
  return (
    <Link
      to={taskFlowPath(project, id)}
      className="text-muted-foreground hover:text-foreground ml-auto inline-flex shrink-0 items-center gap-1.5 text-xs"
    >
      <Waypoints className="size-3.5" />
      Flow
    </Link>
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
}: {
  project: string
  id: string
  parent: string | undefined
  parentTitle: string | undefined
  ready: boolean
}) {
  const navigate = useNavigate()
  const [editing, setEditing] = useState(false)
  useDetailAction("edit-parent", () => setEditing(true))
  useDetailAction("go-parent", () => {
    if (parent !== undefined && parentTitle !== undefined) navigate(taskPath(project, parent))
  })

  if (editing) {
    return <ParentPicker project={project} id={id} onClose={() => setEditing(false)} />
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
    </div>
  )
}

// The full task list is needed only to search for a new parent, so it is fetched here — when the
// picker opens — rather than on every detail view. Excludes self + descendants so a pick can't cycle.
function ParentPicker({ project, id, onClose }: { project: string; id: string; onClose: () => void }) {
  const tasks = useQuery({
    queryKey: tasksKey(project),
    queryFn: abortable(listTasks(project)),
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
          onSelect: () => mutate(patchTask(project, id, { parent: null })),
        })
      }
      for (const { task, indices } of taskMatches(all, query, excluded)) {
        options.push({
          key: task.id,
          content: <ComboTaskRow task={task} indices={indices} />,
          onSelect: () => mutate(patchTask(project, id, { parent: task.id })),
        })
      }
      return options
    },
    [all, project, id, mutate],
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
function RowList({ project, rows, cursor }: { project: string; rows: ReadonlyArray<DetailRow>; cursor: number }) {
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
              mark={
                row.unresolved ? (
                  <UnresolvedMark />
                ) : (
                  <StatusControl project={project} id={row.id} at={row.at} status={row.status} className="size-4" />
                )
              }
              id={row.id}
              title={row.title}
            />
          </Row>
        </li>
      ))}
    </ul>
  )
}

// The two dependency directions. An empty one has nothing to say and stays hidden, unless `shown`
// keeps it for the version of the list that a conflict holds apart.
function RefSection({
  project,
  title,
  rows,
  cursor,
  action,
  shown = false,
}: {
  project: string
  title: string
  rows: ReadonlyArray<DetailRow>
  cursor: number
  action?: ReactNode
  shown?: boolean
}) {
  if (rows.length === 0 && !shown) return null
  return (
    <Section title={title} count={rows.length} action={action}>
      {rows.length === 0 ? (
        <p className="text-muted-foreground text-sm">No dependencies in the version in force.</p>
      ) : (
        <RowList project={project} rows={rows} cursor={cursor} />
      )}
    </Section>
  )
}

const dependenciesConflict = (metadata: Metadata): boolean => {
  const dependencies = frontmatterFields(metadata)?.dependencies
  return dependencies !== undefined && fieldConflict(dependencies) !== undefined
}

// The direct children below the task body, plus an inline add box that either pulls an existing task
// in as a child or creates a fresh one.
function SubtasksSection({
  project,
  id,
  rows,
  cursor,
  ready,
}: {
  project: string
  id: string
  rows: ReadonlyArray<DetailRow>
  cursor: number
  ready: boolean
}) {
  const [adding, setAdding] = useState(false)
  useDetailAction("add-subtask", () => setAdding(true))

  return (
    <Section
      title="Subtasks"
      count={rows.length}
      action={
        <Button variant="accent" onClick={() => setAdding((open) => !open)}>
          <Plus className="size-3.5" />
          Add subtask
        </Button>
      }
    >
      {adding && (
        <div className="mb-3">
          <SubtaskPicker project={project} id={id} onClose={() => setAdding(false)} />
        </div>
      )}
      {rows.length === 0 ? (
        ready ? (
          <p className="text-muted-foreground text-sm">No subtasks yet.</p>
        ) : null
      ) : (
        <RowList project={project} rows={rows} cursor={cursor} />
      )}
    </Section>
  )
}

// Opened on demand, so the full task list it searches is fetched only when adding — not per detail
// view. Making a task a child of `id` closes a cycle only when that task is an ancestor of `id`, so
// exclude the ancestor chain (and self); descendants are valid re-parent targets.
function SubtaskPicker({ project, id, onClose }: { project: string; id: string; onClose: () => void }) {
  const tasks = useQuery({
    queryKey: tasksKey(project),
    queryFn: abortable(listTasks(project)),
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
          onSelect: () => mutate(createTask(project, { title: query, parent: id })),
        })
      }
      for (const { task, indices } of taskMatches(all, query, excluded)) {
        if (parentOf(task.metadata) === id) continue
        options.push({
          key: task.id,
          content: <ComboTaskRow task={task} indices={indices} />,
          onSelect: () => mutate(patchTask(project, task.id, { parent: id })),
        })
      }
      return options
    },
    [all, project, id, mutate],
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

// A deleted task keeps its history, and the history is the one way back to what the task said.
function NotFound({ project, id }: { project: string; id: string }) {
  return (
    <div className="h-full space-y-4 overflow-y-auto">
      <Link to={boardPath(project)} className="text-muted-foreground text-sm hover:underline">
        ← {project}
      </Link>
      <EmptyState title="Task not found" detail={id} />
      <TaskHistory project={project} id={id} selected={undefined} />
    </div>
  )
}

const PROSE = "prose-h2:-mx-6 prose-h2:px-6"

function TaskTitle({ title }: { title: string }) {
  return <h1 className="mb-1.5 text-2xl font-semibold tracking-tight">{title}</h1>
}

// As tall as a tag chip, so the body does not move when the tags load or the last one goes.
function TimesAndTags({ children }: { children: ReactNode }) {
  return <div className="mb-4 flex min-h-8 items-center justify-between gap-4">{children}</div>
}

const timesLine = "h-6 shrink-0"
const tagsBox = "min-w-0 justify-end"

import { useQuery, type UseQueryResult } from "@tanstack/react-query"
import { Activity, MessageSquare, Tags } from "lucide-react"
import { memo, useMemo, type ReactNode } from "react"
import { Link, useParams } from "react-router-dom"

import type { Board, BoardRow } from "@openplan/api-client"
import {
  activityPath,
  ConflictBadge,
  createdOf,
  parentOf,
  ParentLink,
  ProblemBadge,
  problems,
  statusField,
  StatusGroupHeader,
  statusGroupLabel,
  tagsPath,
  taskPath,
  TaskAuthor,
  TaskTags,
  TaskTimes,
} from "@openplan/task-ui"
import { EmptyState, MetaItem, MetaLine, Panel, PanelBody } from "@openplan/ui"

import { ListHeader, ListSkeleton } from "../components/list-header"
import { ChildGuide, GridRow, RowGrid, type RowSlot, TreeGuides } from "../components/row-grid"
import { StatusControl } from "../components/status-control"
import { getBoard, getMergedBoard } from "../lib/api"
import { errorText } from "../lib/format"
import { demotedReason, useProject, useProjects } from "../lib/projects"
import { boardKey, mergedBoardKey } from "../lib/query-client"
import { abortable } from "../lib/runtime"
import { type TagsByName, useTagRegistries } from "../lib/tags"

// `/` is every project at once and `/:project` is one of them. They differ only in which board they
// read; everything below the read is the same view.
export function ListRoute() {
  const { project } = useParams()
  return project === undefined ? <MergedBoard /> : <ProjectBoard project={project} />
}

function MergedBoard() {
  const projects = useProjects()
  const board = useQuery({
    queryKey: mergedBoardKey,
    queryFn: abortable(getMergedBoard),
  })
  if (projects !== undefined && projects.length === 0) {
    return <EmptyState title="No projects yet" detail="Register a repository with `openplan project add`." />
  }
  return <BoardState board={board} project={undefined} />
}

function ProjectBoard({ project }: { project: string }) {
  const projects = useProjects()
  const known = useProject(project)
  const board = useQuery({
    queryKey: boardKey(project),
    queryFn: abortable(getBoard(project)),
  })
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
    <BoardState
      board={board}
      project={project}
      action={
        <div className="flex items-center gap-4">
          <Link
            to={activityPath(project)}
            className="text-muted-foreground hover:text-foreground inline-flex items-center gap-1.5 text-xs normal-case"
          >
            <Activity className="size-3.5" />
            Activity
          </Link>
          <Link
            to={tagsPath(project)}
            className="text-muted-foreground hover:text-foreground inline-flex items-center gap-1.5 text-xs normal-case"
          >
            <Tags className="size-3.5" />
            Tags
          </Link>
        </div>
      }
    />
  )
}

function BoardState({
  board,
  project,
  action,
}: {
  board: UseQueryResult<Board>
  project: string | undefined
  action?: ReactNode
}) {
  const header = <ListHeader view="tasks" project={project} action={action} />
  if (board.isPending) return <ListSkeleton view="tasks" project={project} action={action} />
  if (board.isError) return <EmptyState title="Could not load tasks" detail={errorText(board.error)} />
  // A project with no tasks keeps its panel, because the header is the only way to the tag registry
  // and a project with nothing in it is exactly where the first tag gets registered.
  return board.data.groups.length === 0 ? (
    <Panel>
      {header}
      <PanelBody className="p-6">
        <EmptyState title="No tasks yet" detail="Create one with `openplan create`." />
      </PanelBody>
    </Panel>
  ) : (
    <TaskGrid board={board.data} header={header} />
  )
}

// The keys the id column is sized by, laid under the real one so every cell is as wide as the
// widest. Characters are not width — an abbreviation is proportional, and only the digits are
// tabular — and a merged board carries a key from every project. Within one project the count does
// decide it, because only the digits vary there; so one candidate per project sizes the column
// exactly, and that is a handful of spans rather than one per row.
function sizingKeys(rows: ReadonlyArray<BoardRow>): ReadonlyArray<string> {
  const widest = new Map<string, string>()
  for (const row of rows) {
    const held = widest.get(row.task.project)
    if (held === undefined || row.task.id.length > held.length) widest.set(row.task.project, row.task.id)
  }
  return [...widest.values()]
}

const boardPathOf = (row: BoardRow) => taskPath(row.task.project, row.task.id)
const boardDepthOf = (row: BoardRow) => row.depth

function TaskGrid({ board, header }: { board: Board; header: ReactNode }) {
  const rows = useMemo(() => board.groups.flatMap((group) => group.rows), [board])
  const sizers = useMemo(() => sizingKeys(rows), [rows])
  const projects = useMemo(() => [...new Set(rows.map((row) => row.task.project))], [rows])
  const tags = useTagRegistries(projects)
  // A task and its subtasks part ways when their statuses differ, so the tree the guides draw is the
  // one visible inside a group, not the whole parentage.
  const groups = useMemo(
    () =>
      board.groups.map((group) => ({
        key: group.status ?? "unreadable",
        label: statusGroupLabel(group.status),
        header: <StatusGroupHeader status={group.status} />,
        rows: group.rows,
      })),
    [board],
  )
  return (
    <RowGrid label="Tasks" header={header} groups={groups} pathOf={boardPathOf} depthOf={boardDepthOf}>
      {(row, slot) => <TaskRow {...slot} row={row} sizers={sizers} tags={tags[row.task.project]} />}
    </RowGrid>
  )
}

// Every prop keeps its identity while its row stays as it was, so a move of the cursor renders only
// the row it leaves and the row it reaches.
const TaskRow = memo(function TaskRow({
  row,
  sizers,
  tags,
  guides,
  ...slot
}: RowSlot & {
  row: BoardRow
  sizers: ReadonlyArray<string>
  tags: TagsByName | undefined
}) {
  const { path, at } = slot
  const { task, parent_title } = row
  const parent = parentOf(task.metadata)
  const created = createdOf(task.metadata)
  const broken = problems(task.metadata)

  return (
    <GridRow slot={slot}>
      <TreeGuides columns={guides.columns} />
      <div role="gridcell" className="relative flex shrink-0 items-center self-stretch">
        <StatusControl project={task.project} id={task.id} at={at} status={statusField(task.metadata)} />
        {guides.opensChildren && <ChildGuide />}
      </div>
      <div role="gridcell" className="text-muted-foreground grid shrink-0 self-center pl-3 text-xs tabular-nums">
        {/* Laying the board's sizing keys under this one, in the same grid cell, makes every such
            cell as wide as the widest of them — so the title clears the longest key without the
            shorter ones leaving a gap. Sizing the cell by the row's own key instead would shift each
            title separately. */}
        {sizers.map((label) => (
          <span key={label} aria-hidden className="invisible col-start-1 row-start-1">
            {label}
          </span>
        ))}
        <span className="col-start-1 row-start-1">{task.id}</span>
      </div>
      <div className="min-w-0 grow pr-4 pl-3" role="gridcell">
        <Link
          to={path}
          tabIndex={-1}
          // Narrow enough and the title takes the second line it needs; a wide row has the room to
          // keep every title on one.
          className="text-foreground/90 block line-clamp-2 text-sm font-normal sm:line-clamp-none sm:truncate"
        >
          {task.title}
        </Link>
        <MetaLine>
          {task.conflicts > 0 && <ConflictBadge count={task.conflicts} />}
          {task.problems.length > 0 && <ProblemBadge problems={task.problems} />}
          {parent_title !== undefined && parent !== undefined && (
            <ParentLink project={task.project} id={parent} title={parent_title} />
          )}
          <TaskAuthor author={task.author} withAgent={false} />
          <TaskTimes created={created} updated={task.updated} problems={broken} />
          {task.comment_count > 0 && (
            <MetaItem icon={MessageSquare} className="shrink-0 whitespace-nowrap tabular-nums">
              {task.comment_count}
            </MetaItem>
          )}
          <TaskTags metadata={task.metadata} tags={tags} />
        </MetaLine>
      </div>
    </GridRow>
  )
})

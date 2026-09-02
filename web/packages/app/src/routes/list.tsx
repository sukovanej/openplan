import { useQuery, type UseQueryResult } from "@tanstack/react-query"
import { MessageSquare, Tags } from "lucide-react"
import { useEffect, useMemo, useRef, type MouseEvent, type ReactNode, type Ref } from "react"
import { Link, useNavigate, useParams } from "react-router-dom"

import type { Board, BoardRow } from "@open-planner/api-client"
import {
  BranchBadges,
  createdOf,
  parentOf,
  ParentLink,
  problems,
  StatusField,
  StatusGroupHeader,
  statusGroupLabel,
  tagsPath,
  taskPath,
  TaskTags,
  TaskTimes,
} from "@open-planner/task-ui"
import { cn, EmptyState, MetaItem, MetaLine, Panel, PanelBody, PanelHeader, PanelTitle, Row } from "@open-planner/ui"

import { ListSkeleton } from "../components/states"
import { getBoard, getMergedBoard } from "../lib/api"
import { errorText } from "../lib/format"
import { demotedReason, useProject, useProjects } from "../lib/projects"
import { boardKey, mergedBoardKey } from "../lib/query-client"
import { rowCursor, useRowCursor } from "../lib/row-cursor"
import { hoveredRow } from "../lib/row-target"
import { runtime } from "../lib/runtime"
import { useTags } from "../lib/tags"
import { treeGuides, type RowGuides } from "../lib/tree-guides"

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
    queryFn: () => runtime.runPromise(getMergedBoard),
  })
  if (projects !== undefined && projects.length === 0) {
    return <EmptyState title="No projects yet" detail="Register a repository with `openplan project add`." />
  }
  return <BoardState board={board} title="All projects" />
}

function ProjectBoard({ project }: { project: string }) {
  const projects = useProjects()
  const known = useProject(project)
  const board = useQuery({
    queryKey: boardKey(project),
    queryFn: () => runtime.runPromise(getBoard(project)),
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
      title={project}
      action={
        <Link
          to={tagsPath(project)}
          className="text-muted-foreground hover:text-foreground inline-flex items-center gap-1.5 text-xs normal-case"
        >
          <Tags className="size-3.5" />
          Tags
        </Link>
      }
    />
  )
}

function BoardState({ board, title, action }: { board: UseQueryResult<Board>; title: string; action?: ReactNode }) {
  if (board.isPending) return <ListSkeleton />
  if (board.isError) return <EmptyState title="Could not load tasks" detail={errorText(board.error)} />
  // A project with no tasks keeps its panel, because the header is the only way to the tag registry
  // and a project with nothing in it is exactly where the first tag gets registered.
  return board.data.groups.length === 0 ? (
    <Panel>
      <PanelHeader className="gap-3">
        <PanelTitle>{title}</PanelTitle>
        {action !== undefined && <div className="ml-auto">{action}</div>}
      </PanelHeader>
      <PanelBody className="p-6">
        <EmptyState title="No tasks yet" detail="Create one with `openplan create`." />
      </PanelBody>
    </Panel>
  ) : (
    <TaskGrid board={board.data} title={title} action={action} />
  )
}

const rowDomId = (path: string) => `task-row-${path}`

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

function TaskGrid({ board, title, action }: { board: Board; title: string; action?: ReactNode }) {
  // The board arrives already grouped, ordered, and flattened; the cursor walks the concatenation of
  // every group's rows in that same visible order.
  const rows = useMemo(() => board.groups.flatMap((group) => group.rows), [board])
  const paths = useMemo(() => rows.map((row) => taskPath(row.task.project, row.task.id)), [rows])
  const { index } = useRowCursor(paths)
  const sizers = useMemo(() => sizingKeys(rows), [rows])
  const activeId = index >= 0 && index < paths.length ? rowDomId(paths[index]) : undefined

  const activeRow = useRef<HTMLDivElement>(null)
  useEffect(() => {
    activeRow.current?.scrollIntoView({ block: "nearest" })
  }, [index])

  // A task and its subtasks part ways when their statuses differ, so the tree the guides draw is the
  // one visible inside a group, not the whole parentage.
  const guides = useMemo(() => board.groups.map((group) => treeGuides(group.rows)), [board])

  let base = 0
  return (
    <Panel
      role="grid"
      aria-label="Tasks"
      aria-activedescendant={activeId}
      tabIndex={0}
      className="text-sm focus:outline-none"
    >
      <PanelHeader className="gap-3">
        <PanelTitle>{title}</PanelTitle>
        {action !== undefined && <div className="ml-auto">{action}</div>}
      </PanelHeader>
      <PanelBody onMouseLeave={hoveredRow.clear}>
        {board.groups.map((group, groupIndex) => {
          const lastGroup = groupIndex === board.groups.length - 1
          return (
            <div key={group.status ?? "unreadable"} role="rowgroup" aria-label={statusGroupLabel(group.status)}>
              <StatusGroupHeader status={group.status} />
              {group.rows.map((row, j) => {
                const i = base++
                return (
                  <TaskRow
                    key={paths[i]}
                    ref={i === index ? activeRow : undefined}
                    row={row}
                    path={paths[i]}
                    at={i}
                    sizers={sizers}
                    guides={guides[groupIndex][j]}
                    active={i === index}
                    // The pointer only marks the current row while the keyboard cursor is idle, so
                    // the two never claim it at once.
                    hoverable={index === -1}
                    tableLast={lastGroup && j === group.rows.length - 1}
                    onFocus={() => rowCursor.focus(i)}
                  />
                )
              })}
            </div>
          )
        })}
      </PanelBody>
    </Panel>
  )
}

// Tree lines run down the middle of the status icon they hang from — half an icon in from the indent
// that column occupies — and turn in at the row's own middle, where the icon they point at sits.
// Guides stretch to the row's content box, so each segment overhangs it by the row's padding to meet
// the segment in the row above or below: 12px up, and 12px plus the separator down.
const GUIDE_ROW_TOP = "-top-3"
const GUIDE_ROW_BOTTOM = "-bottom-[13px]"
const GUIDE_TO_MIDDLE = "h-[calc(50%+0.75rem)]"

function Guide({ className }: { className: string }) {
  return <span aria-hidden className={cn("border-muted-foreground/30 absolute left-[0.625rem]", className)} />
}

function TreeGuides({ columns }: { columns: ReadonlyArray<boolean> }) {
  return (
    <div aria-hidden className="flex shrink-0 self-stretch pl-4">
      {columns.map((continues, column) =>
        column === columns.length - 1 ? (
          <div key={column} className="relative w-6">
            <Guide className={cn(GUIDE_ROW_TOP, GUIDE_TO_MIDDLE, "right-0 border-b border-l")} />
            {continues && <Guide className={cn(GUIDE_ROW_BOTTOM, "top-1/2 border-l")} />}
          </div>
        ) : (
          <div key={column} className="relative w-6">
            {continues && <Guide className={cn(GUIDE_ROW_TOP, GUIDE_ROW_BOTTOM, "border-l")} />}
          </div>
        ),
      )}
    </div>
  )
}

function TaskRow({
  ref,
  row,
  path,
  at,
  sizers,
  guides,
  active,
  hoverable,
  tableLast,
  onFocus,
}: {
  ref?: Ref<HTMLDivElement>
  row: BoardRow
  path: string
  at: number
  sizers: ReadonlyArray<string>
  guides: RowGuides
  active: boolean
  hoverable: boolean
  tableLast: boolean
  onFocus: () => void
}) {
  const { task, parent_title } = row
  const parent = parentOf(task.metadata)
  const created = createdOf(task.metadata)
  const broken = problems(task.metadata)
  const { byName: tags } = useTags(task.project)
  const navigate = useNavigate()

  // The row opens its task from its own click rather than from a link stretched over it: an overlay
  // that size takes every hover in the row with it, leaving the tooltips underneath unreachable. The
  // links the row does contain answer their own clicks, and a modified click is the browser's.
  const open = (event: MouseEvent<HTMLDivElement>) => {
    onFocus()
    const link = event.target instanceof Element && event.target.closest("a") !== null
    if (link || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
    navigate(path)
  }

  return (
    <Row
      ref={ref}
      id={rowDomId(path)}
      role="row"
      aria-selected={active}
      active={active}
      hoverable={hoverable}
      last={tableLast}
      onClick={open}
      // Scrolling a list under a still pointer moves `:hover` to another row without a mousemove,
      // so the row the pointer marks is only in step with the store if entering counts too. It must
      // not count while the keyboard drives, or walking with `j` would hand rows it scrolls past
      // back to the pointer.
      onMouseEnter={() => {
        if (hoverable) hoveredRow.enter(path, at)
      }}
      // Moving the pointer is what hands the current row back to it — and only over a row, so a
      // nudge across a group header or the scrollbar leaves the keyboard's row where it was.
      onMouseMove={() => {
        hoveredRow.enter(path, at)
        rowCursor.clear()
      }}
      onMouseLeave={() => hoveredRow.leave(path, at)}
      // Wrapping lets the branches drop to a line of their own once the title has no room left
      // beside them, rather than the two columns overrunning each other.
      className="flex flex-wrap cursor-pointer items-start gap-y-2 py-3"
    >
      <TreeGuides columns={guides.columns} />
      <div role="gridcell" className="relative flex shrink-0 items-center self-stretch">
        <StatusField metadata={task.metadata} />
        {guides.opensChildren && <Guide className={cn(GUIDE_ROW_BOTTOM, "top-[calc(50%+0.625rem)] border-l")} />}
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
      <div className="min-w-0 grow basis-56 pr-4 pl-3 sm:pr-0" role="gridcell">
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
          {parent_title !== undefined && parent !== undefined && (
            <ParentLink project={task.project} id={parent} title={parent_title} />
          )}
          <TaskTimes created={created} updated={task.updated} problems={broken} />
          {task.comment_count > 0 && (
            <MetaItem icon={MessageSquare} className="shrink-0 whitespace-nowrap tabular-nums">
              {task.comment_count}
            </MetaItem>
          )}
          <TaskTags metadata={task.metadata} tags={tags} />
        </MetaLine>
        {task.branches.length > 0 && (
          <BranchBadges branches={task.branches} headline={task.headline} className="mt-2 sm:hidden" />
        )}
      </div>
      <div role="gridcell" className="ml-auto hidden shrink-0 self-center pr-4 pl-3 sm:block">
        <BranchBadges branches={task.branches} headline={task.headline} className="justify-end" />
      </div>
    </Row>
  )
}

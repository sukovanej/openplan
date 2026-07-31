import { useEffect, useMemo, useRef, type MouseEvent, type Ref } from "react"
import { Link, useNavigate } from "react-router-dom"

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
  taskPath,
  TaskTimes,
} from "@open-planner/task-ui"
import { cn, EmptyState, MetaLine, Panel, PanelBody, PanelHeader, Row } from "@open-planner/ui"

import { ListSkeleton } from "../components/states"
import { hoveredRow } from "../lib/copy-target"
import { errorText } from "../lib/format"
import { rowCursor, useRowCursor } from "../lib/row-cursor"
import { boardQuery, useQuery } from "../lib/store"
import { treeGuides, type RowGuides } from "../lib/tree-guides"

export function ListRoute() {
  const board = useQuery(boardQuery)
  switch (board._tag) {
    case "loading":
      return <ListSkeleton />
    case "failure":
      return <EmptyState title="Could not load tasks" detail={errorText(board.error)} />
    case "success":
      return board.value.groups.length === 0 ? (
        <EmptyState title="No tasks yet" detail="Create one with `oplan create`." />
      ) : (
        <TaskGrid board={board.value} />
      )
  }
}

const rowDomId = (id: string) => `task-row-${id}`

function TaskGrid({ board }: { board: Board }) {
  // The board arrives already grouped, ordered, and flattened; the cursor walks the concatenation of
  // every group's rows in that same visible order.
  const rows = useMemo(() => board.groups.flatMap((group) => group.rows), [board])
  const ids = useMemo(() => rows.map((row) => row.task.id), [rows])
  const { index } = useRowCursor(ids)
  const widestId = useMemo(() => ids.reduce((widest, id) => (id.length > widest.length ? id : widest), ""), [ids])
  const activeId = index >= 0 && index < rows.length ? rowDomId(rows[index].task.id) : undefined

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
      <PanelHeader>Tasks</PanelHeader>
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
                    key={row.task.id}
                    ref={i === index ? activeRow : undefined}
                    row={row}
                    widestId={widestId}
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
  widestId,
  guides,
  active,
  hoverable,
  tableLast,
  onFocus,
}: {
  ref?: Ref<HTMLDivElement>
  row: BoardRow
  widestId: string
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
  const navigate = useNavigate()

  // The row opens its task from its own click rather than from a link stretched over it: an overlay
  // that size takes every hover in the row with it, leaving the tooltips underneath unreachable. The
  // links the row does contain answer their own clicks, and a modified click is the browser's.
  const open = (event: MouseEvent<HTMLDivElement>) => {
    onFocus()
    const link = event.target instanceof Element && event.target.closest("a") !== null
    if (link || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
    navigate(taskPath(task.id))
  }

  return (
    <Row
      ref={ref}
      id={rowDomId(task.id)}
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
        if (hoverable) hoveredRow.enter(task.id)
      }}
      // Moving the pointer is what hands the current row back to it — and only over a row, so a
      // nudge across a group header or the scrollbar leaves the keyboard's row where it was.
      onMouseMove={() => {
        hoveredRow.enter(task.id)
        rowCursor.clear()
      }}
      onMouseLeave={() => hoveredRow.leave(task.id)}
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
        {/* Laying the widest id on the board under this one, in the same grid cell, makes every id
            cell that wide — so the title clears the longest id without the shorter ones leaving a
            gap. Sizing the cell by the row's own id instead would shift each title separately. */}
        <span aria-hidden className="invisible col-start-1 row-start-1">
          {widestId}
        </span>
        <span className="col-start-1 row-start-1">{task.id}</span>
      </div>
      <div className="min-w-0 grow basis-56 pr-4 pl-3 sm:pr-0" role="gridcell">
        <Link
          to={taskPath(task.id)}
          tabIndex={-1}
          // Narrow enough and the title takes the second line it needs; a wide row has the room to
          // keep every title on one.
          className="text-foreground/90 block line-clamp-2 text-sm font-normal sm:line-clamp-none sm:truncate"
        >
          {task.title}
        </Link>
        <MetaLine>
          {parent_title !== undefined && parent !== undefined && <ParentLink id={parent} title={parent_title} />}
          <TaskTimes created={created} updated={task.updated} problems={broken} />
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

import { useEffect, useMemo, useRef, type Ref } from "react"
import { Link } from "react-router-dom"

import type { Board, BoardRow, Status } from "@open-planner/api-client"

import { BranchBadges } from "../components/branch-badges"
import { ListSkeleton, Message } from "../components/states"
import { statusHeaderClass, StatusField, statusLabel } from "../components/status-badge"
import { MetaItem, MetaLine, PARENT_ICON, TaskTimes } from "../components/task-meta"
import { hoveredRow } from "../lib/copy-target"
import { errorText } from "../lib/format"
import { createdOf, parentOf, problems } from "../lib/metadata"
import { rowCursor, useRowCursor } from "../lib/row-cursor"
import { boardQuery, useQuery } from "../lib/store"
import { cn } from "../lib/utils"

export function ListRoute() {
  const board = useQuery(boardQuery)
  switch (board._tag) {
    case "loading":
      return <ListSkeleton />
    case "failure":
      return <Message title="Could not load tasks" detail={errorText(board.error)} />
    case "success":
      return board.value.groups.length === 0 ? (
        <Message title="No tasks yet" detail="Create one with `oplan create`." />
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
  const activeId = index >= 0 && index < rows.length ? rowDomId(rows[index].task.id) : undefined

  const activeRow = useRef<HTMLDivElement>(null)
  useEffect(() => {
    activeRow.current?.scrollIntoView({ block: "nearest" })
  }, [index])

  let base = 0
  return (
    <div
      role="grid"
      aria-label="Tasks"
      aria-activedescendant={activeId}
      tabIndex={0}
      // Box outline is an inset ring, not `border`, so the selected row's own inset ring lands on
      // the same pixels and reads as one line instead of doubling up against a container border.
      className="bg-muted/10 flex h-full flex-col overflow-hidden rounded-lg ring-1 ring-inset ring-border text-sm focus:outline-none"
    >
      <div className="bg-muted/30 flex h-11 shrink-0 items-center border-b px-4 text-xs font-medium tracking-wide uppercase text-muted-foreground">
        Tasks
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto" onMouseLeave={hoveredRow.clear}>
        {board.groups.map((group, groupIndex) => {
          const lastGroup = groupIndex === board.groups.length - 1
          return (
            <div key={group.status ?? "unreadable"} role="rowgroup" aria-label={groupLabel(group.status)}>
              <HeaderRow status={group.status} />
              {group.rows.map((row, j) => {
                const i = base++
                return (
                  <TaskRow
                    key={row.task.id}
                    ref={i === index ? activeRow : undefined}
                    row={row}
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
      </div>
    </div>
  )
}

// A group with no status holds the tasks whose own status could not be read; it is not a status
// they claimed, so it gets its own header rather than borrowing one.
const groupLabel = (status: Status | undefined) => (status === undefined ? "Unreadable" : statusLabel(status))

const UNREADABLE_HEADER = "bg-red-500/8 border-red-500/20 text-red-600/80 dark:text-red-300/80"

function HeaderRow({ status }: { status: Status | undefined }) {
  return (
    <div role="row" className="bg-muted/20 border-b p-2">
      <span
        role="columnheader"
        className={cn(
          "inline-block rounded-md border px-2 py-0.5 text-xs font-medium tracking-wide uppercase",
          status === undefined ? UNREADABLE_HEADER : statusHeaderClass(status),
        )}
      >
        {groupLabel(status)}
      </span>
    </div>
  )
}

// The current row loses its own bottom border — the outline draws that edge — and gets the outline
// as an absolutely-positioned overlay, offset up 1px to sit on the separator above, so neither
// participates in flow and the row keeps its height whether or not it is current.
const CURRENT_ROW =
  "border-transparent bg-muted/30 after:pointer-events-none after:absolute after:inset-x-0 after:-top-px after:bottom-px after:border after:border-blue-600/40 after:content-['']"

// The same treatment under the pointer, spelled out because Tailwind only emits classes it can read
// literally in the source.
const HOVERED_ROW =
  "hover:border-transparent hover:bg-muted/30 hover:after:pointer-events-none hover:after:absolute hover:after:inset-x-0 hover:after:-top-px hover:after:bottom-px hover:after:border hover:after:border-blue-600/40 hover:after:content-['']"

function TaskRow({
  ref,
  row,
  active,
  hoverable,
  tableLast,
  onFocus,
}: {
  ref?: Ref<HTMLDivElement>
  row: BoardRow
  active: boolean
  hoverable: boolean
  tableLast: boolean
  onFocus: () => void
}) {
  const { task, depth, parent_title } = row
  const parent = parentOf(task.metadata)
  const created = createdOf(task.metadata)
  const broken = problems(task.metadata)
  return (
    <div
      ref={ref}
      id={rowDomId(task.id)}
      role="row"
      aria-selected={active}
      onClick={onFocus}
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
      className={cn(
        "relative flex cursor-pointer items-center border-b transition-colors",
        // The last row drops its divider; it has no row below to separate it from.
        tableLast && "border-transparent",
        active && CURRENT_ROW,
        hoverable && HOVERED_ROW,
      )}
    >
      <div
        role="gridcell"
        className="shrink-0 py-3"
        // Indent nested tasks so the parent→child relationship reads at a glance.
        style={{ paddingLeft: `${1 + depth * 1.5}rem` }}
      >
        <StatusField metadata={task.metadata} />
      </div>
      {/* Wide enough for four digits, so a shorter id does not pull the titles below it out of line. */}
      <div role="gridcell" className="text-muted-foreground min-w-14 shrink-0 py-3 pl-3 text-xs tabular-nums">
        {task.id}
      </div>
      <div className="min-w-0 flex-1 py-3 pl-3" role="gridcell">
        <Link
          to={`/task/${task.id}`}
          tabIndex={-1}
          className="text-foreground/90 block truncate text-sm font-normal after:absolute after:inset-0"
        >
          {task.title}
        </Link>
        <MetaLine>
          {parent_title !== undefined && parent !== undefined && (
            <MetaItem icon={PARENT_ICON} title={`Subtask of ${parent_title}`}>
              <Link
                to={`/task/${parent}`}
                className="text-foreground/90 group relative z-10 flex min-w-0 items-center gap-1.5"
              >
                <span className="text-muted-foreground shrink-0 tabular-nums">{parent}</span>
                <span className="max-w-[15rem] truncate group-hover:underline">{parent_title}</span>
              </Link>
            </MetaItem>
          )}
          <TaskTimes created={created} updated={task.updated} problems={broken} />
        </MetaLine>
      </div>
      <div role="gridcell" className="shrink-0 py-3 pr-4 pl-3">
        <BranchBadges branches={task.branches} headline={task.headline} />
      </div>
    </div>
  )
}

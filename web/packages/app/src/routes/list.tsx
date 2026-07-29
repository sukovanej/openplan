import { useMemo } from "react"
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

  let base = 0
  return (
    <div
      role="grid"
      aria-label="Tasks"
      aria-activedescendant={activeId}
      tabIndex={0}
      onMouseMove={() => {
        if (index !== -1) rowCursor.clear()
      }}
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
                    row={row}
                    active={i === index}
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

function TaskRow({
  row,
  active,
  tableLast,
  onFocus,
}: {
  row: BoardRow
  active: boolean
  tableLast: boolean
  onFocus: () => void
}) {
  const { task, depth, parent_title } = row
  const parent = parentOf(task.metadata)
  const created = createdOf(task.metadata)
  const broken = problems(task.metadata)
  return (
    <div
      id={rowDomId(task.id)}
      role="row"
      aria-selected={active}
      onClick={onFocus}
      onMouseMove={() => hoveredRow.enter(task.id)}
      onMouseLeave={() => hoveredRow.leave(task.id)}
      className={cn(
        // Every row keeps a bottom border so its height never changes; it just goes transparent
        // for the selected row (the outline draws its edges) and the last row (no trailing divider).
        "relative flex cursor-pointer items-center border-b transition-colors",
        (active || tableLast) && "border-transparent",
        // The selection outline is an absolutely-positioned overlay, offset up 1px to sit on the
        // separator above, so it never participates in flow and can't shift any row's height.
        active
          ? "bg-muted/30 after:pointer-events-none after:absolute after:inset-x-0 after:-top-px after:bottom-px after:border after:border-blue-600/40 after:content-['']"
          : "hover:bg-muted/30",
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
                className="text-foreground/90 relative z-10 max-w-[15rem] truncate hover:underline"
              >
                {parent_title}
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

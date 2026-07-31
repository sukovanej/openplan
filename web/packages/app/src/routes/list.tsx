import { useEffect, useMemo, useRef, type Ref } from "react"
import { Link } from "react-router-dom"

import type { Board, BoardRow, Status } from "@open-planner/api-client"
import { cn, EmptyState, MetaItem, MetaLine } from "@open-planner/ui"

import { BranchBadges } from "../components/branch-badges"
import { ListSkeleton } from "../components/states"
import { statusHeaderClass, StatusField, statusLabel } from "../components/status-badge"
import { PARENT_ICON, TaskTimes } from "../components/task-meta"
import { hoveredRow } from "../lib/copy-target"
import { errorText } from "../lib/format"
import { createdOf, parentOf, problems } from "../lib/metadata"
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

// The current row loses its own bottom border — the outline draws that edge — and gets the outline
// as an absolutely-positioned overlay, so neither participates in flow and the row keeps its height
// whether or not it is current. The overlay is positioned against the row's padding box, which stops
// 1px short of each separator, so both offsets are -1px: the top edge lands on the separator above,
// the bottom edge on the row's own, exactly where the neighbouring rows draw theirs.
const CURRENT_ROW =
  "border-transparent bg-muted/30 after:pointer-events-none after:absolute after:inset-x-0 after:-top-px after:-bottom-px after:border after:border-blue-600/40 after:content-['']"

// The same treatment under the pointer, spelled out because Tailwind only emits classes it can read
// literally in the source.
const HOVERED_ROW =
  "hover:border-transparent hover:bg-muted/30 hover:after:pointer-events-none hover:after:absolute hover:after:inset-x-0 hover:after:-top-px hover:after:-bottom-px hover:after:border hover:after:border-blue-600/40 hover:after:content-['']"

function TaskRow({
  ref,
  row,
  guides,
  active,
  hoverable,
  tableLast,
  onFocus,
}: {
  ref?: Ref<HTMLDivElement>
  row: BoardRow
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
        // Wrapping lets the branches drop to a line of their own once the title has no room left
        // beside them, rather than the two columns overrunning each other.
        "relative flex flex-wrap cursor-pointer items-start gap-y-2 border-b py-3 transition-colors",
        // The last row drops its divider; it has no row below to separate it from.
        tableLast && "border-transparent",
        active && CURRENT_ROW,
        hoverable && HOVERED_ROW,
      )}
    >
      <TreeGuides columns={guides.columns} />
      <div role="gridcell" className="relative flex shrink-0 items-center self-stretch">
        <StatusField metadata={task.metadata} />
        {guides.opensChildren && <Guide className={cn(GUIDE_ROW_BOTTOM, "top-[calc(50%+0.625rem)] border-l")} />}
      </div>
      <div role="gridcell" className="text-muted-foreground shrink-0 self-center pl-3 text-xs tabular-nums">
        {task.id}
      </div>
      <div className="min-w-0 grow basis-56 pr-4 pl-3 sm:pr-0" role="gridcell">
        <Link
          to={`/task/${task.id}`}
          tabIndex={-1}
          // Narrow enough and the title takes the second line it needs; a wide row has the room to
          // keep every title on one.
          className="text-foreground/90 block line-clamp-2 text-sm font-normal after:absolute after:inset-0 sm:line-clamp-none sm:truncate"
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
        {task.branches.length > 0 && (
          <BranchBadges branches={task.branches} headline={task.headline} className="mt-2 sm:hidden" />
        )}
      </div>
      <div role="gridcell" className="ml-auto hidden shrink-0 self-center pr-4 pl-3 sm:block">
        <BranchBadges branches={task.branches} headline={task.headline} className="justify-end" />
      </div>
    </div>
  )
}

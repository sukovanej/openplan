import { useMemo } from "react"
import { Link } from "react-router-dom"

import { BranchBadges } from "@/components/branch-badges"
import { ListSkeleton, Message } from "@/components/states"
import { StatusIcon } from "@/components/status-badge"
import type { TaskListItem } from "@/lib/api"
import { errorText } from "@/lib/format"
import { type ForestRow, forestRows } from "@/lib/hierarchy"
import { rowCursor, useRowCursor } from "@/lib/row-cursor"
import { tasksQuery, useQuery } from "@/lib/store"
import { cn } from "@/lib/utils"

export function ListRoute() {
  const tasks = useQuery(tasksQuery)
  switch (tasks._tag) {
    case "loading":
      return <ListSkeleton />
    case "failure":
      return <Message title="Could not load tasks" detail={errorText(tasks.error)} />
    case "success":
      return tasks.value.length === 0
        ? <Message title="No tasks yet" detail="Create one with `oplan create`." />
        : <TaskGrid tasks={tasks.value} />
  }
}

const rowDomId = (id: string) => `task-row-${id}`

function TaskGrid({ tasks }: { tasks: ReadonlyArray<TaskListItem> }) {
  // Pre-order the parent→child forest so each task is followed by its rank-ordered subtree; the row
  // cursor then walks the same visible order.
  const rows = useMemo(() => forestRows(tasks), [tasks])
  const ids = useMemo(() => rows.map((row) => row.task.id), [rows])
  const { index } = useRowCursor(ids)
  const activeId = index >= 0 && index < rows.length ? rowDomId(rows[index].task.id) : undefined

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
      <div className="min-h-0 flex-1 overflow-y-auto">
        {rows.map((row, i) => (
          <TaskRow
            key={row.task.id}
            row={row}
            active={i === index}
            tableLast={i === rows.length - 1}
            onFocus={() => rowCursor.focus(i)}
          />
        ))}
      </div>
    </div>
  )
}

function TaskRow(
  { row, active, tableLast, onFocus }: {
    row: ForestRow
    active: boolean
    tableLast: boolean
    onFocus: () => void
  },
) {
  const { task, depth } = row
  return (
    <div
      id={rowDomId(task.id)}
      role="row"
      aria-selected={active}
      onClick={onFocus}
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
        <StatusIcon status={task.status} />
      </div>
      <div className="min-w-0 flex-1 py-3 pl-3" role="gridcell">
        <Link
          to={`/task/${task.id}`}
          tabIndex={-1}
          className="text-foreground/90 block truncate text-sm font-normal after:absolute after:inset-0"
        >
          {task.title}
        </Link>
      </div>
      <div role="gridcell" className="shrink-0 py-3 pr-4 pl-3">
        <BranchBadges branches={task.branches} headline={task.headline} />
      </div>
    </div>
  )
}

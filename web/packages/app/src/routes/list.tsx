import { useMemo } from "react"
import { Link } from "react-router-dom"

import { BranchBadges } from "@/components/branch-badges"
import { ListSkeleton, Message } from "@/components/states"
import { statusHeaderClass, StatusIcon, statusLabel, statusOrder } from "@/components/status-badge"
import type { Status, TaskListItem } from "@/lib/api"
import { errorText } from "@/lib/format"
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

interface Group {
  readonly status: Status
  readonly tasks: ReadonlyArray<TaskListItem>
}

function TaskGrid({ tasks }: { tasks: ReadonlyArray<TaskListItem> }) {
  const groups = useMemo<ReadonlyArray<Group>>(
    () =>
      statusOrder
        .map((status) => ({ status, tasks: tasks.filter((task) => task.status === status) }))
        .filter((group) => group.tasks.length > 0),
    [tasks],
  )
  const ordered = useMemo(() => groups.flatMap((group) => group.tasks), [groups])
  const ids = useMemo(() => ordered.map((task) => task.id), [ordered])
  const { index } = useRowCursor(ids)
  const activeId = index >= 0 && index < ordered.length ? rowDomId(ordered[index].id) : undefined

  let base = 0
  return (
    <div
      role="grid"
      aria-label="Tasks"
      aria-activedescendant={activeId}
      tabIndex={0}
      className="overflow-hidden rounded-lg text-sm focus:outline-none"
    >
      {groups.map((group, groupIndex) => {
        const groupBase = base
        base += group.tasks.length
        const lastGroup = groupIndex === groups.length - 1
        return (
          <div key={group.status} role="rowgroup" aria-label={statusLabel(group.status)}>
            <HeaderRow status={group.status} roundTop={groupIndex === 0} />
            {group.tasks.map((task, j) => (
              <TaskRow
                key={task.id}
                task={task}
                index={groupBase + j}
                active={groupBase + j === index}
                lastInGroup={j === group.tasks.length - 1}
                tableLast={lastGroup && j === group.tasks.length - 1}
              />
            ))}
          </div>
        )
      })}
    </div>
  )
}

function HeaderRow({ status, roundTop }: { status: Status; roundTop: boolean }) {
  return (
    <div
      role="row"
      className={cn("border px-4 py-2", roundTop && "rounded-t-lg", statusHeaderClass(status))}
    >
      <span role="columnheader" className="text-xs font-normal tracking-wide uppercase">
        {statusLabel(status)}
      </span>
    </div>
  )
}

function TaskRow(
  { task, index, active, lastInGroup, tableLast }: {
    task: TaskListItem
    index: number
    active: boolean
    lastInGroup: boolean
    tableLast: boolean
  },
) {
  return (
    <div
      id={rowDomId(task.id)}
      role="row"
      aria-selected={active}
      onClick={() => rowCursor.focus(index)}
      className={cn(
        "relative flex cursor-pointer items-center border-x transition-colors",
        (!lastInGroup || tableLast) && "border-b",
        tableLast && "rounded-b-lg",
        active ? "bg-accent" : "bg-muted/30 hover:bg-accent",
      )}
    >
      <div role="gridcell" className="shrink-0 px-4 py-3">
        <StatusIcon status={task.status} />
      </div>
      <div role="gridcell" className="min-w-0 flex-1 py-3">
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

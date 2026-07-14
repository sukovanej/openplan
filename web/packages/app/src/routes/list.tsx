import { useMemo } from "react"
import { Link } from "react-router-dom"

import { BranchBadges } from "@/components/branch-badges"
import { ListSkeleton, Message } from "@/components/states"
import { StatusBadge } from "@/components/status-badge"
import type { TaskListItem } from "@/lib/api"
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
        : <TaskTable tasks={tasks.value} />
  }
}

const rowDomId = (id: string) => `task-row-${id}`

function TaskTable({ tasks }: { tasks: ReadonlyArray<TaskListItem> }) {
  const ids = useMemo(() => tasks.map((task) => task.id), [tasks])
  const { index } = useRowCursor(ids)
  const activeId = index >= 0 && index < tasks.length ? rowDomId(tasks[index].id) : undefined

  return (
    <div className="overflow-hidden rounded-lg">
      <table
        role="grid"
        aria-label="Tasks"
        aria-activedescendant={activeId}
        tabIndex={0}
        className="w-full table-fixed border-separate border-spacing-0 text-sm focus:outline-none"
      >
        <tbody role="rowgroup">
          {tasks.map((task, i) => {
            const active = i === index
            const first = i === 0
            const last = i === tasks.length - 1
            const aboveActive = i === index - 1
            const edge = cn(
              (first || active) && "border-t",
              !aboveActive && "border-b",
              active ? "border-ring" : "border-border",
            )
            return (
              <tr
                key={task.id}
                id={rowDomId(task.id)}
                role="row"
                aria-selected={active}
                onClick={() => rowCursor.focus(i)}
                className={cn("relative cursor-pointer transition-colors", active ? "bg-accent" : "hover:bg-accent")}
              >
                <td
                  role="gridcell"
                  className={cn("border-l px-4 py-3", edge, first && "rounded-tl-lg", last && "rounded-bl-lg")}
                >
                  <Link
                    to={`/task/${task.id}`}
                    tabIndex={-1}
                    className="block truncate font-medium after:absolute after:inset-0"
                  >
                    {task.title}
                  </Link>
                </td>
                <td
                  role="gridcell"
                  className={cn("w-72 border-r px-4 py-3", edge, first && "rounded-tr-lg", last && "rounded-br-lg")}
                >
                  <div className="flex items-center justify-end gap-2">
                    <BranchBadges branches={task.branches} />
                    <StatusBadge status={task.status} />
                  </div>
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}

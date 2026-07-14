import { Link } from "react-router-dom"

import { ListSkeleton, Message } from "@/components/states"
import { StatusBadge } from "@/components/status-badge"
import { Card } from "@/components/ui/card"
import type { TaskSummary } from "@/lib/api"
import { errorText } from "@/lib/format"
import { tasksQuery, useQuery } from "@/lib/store"

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
        : <TaskList tasks={tasks.value} />
  }
}

function TaskList({ tasks }: { tasks: ReadonlyArray<TaskSummary> }) {
  return (
    <ul className="space-y-2">
      {tasks.map((task) => (
        <li key={task.id}>
          <Link to={`/task/${task.id}`} className="block">
            <Card className="hover:bg-accent flex-row items-center justify-between gap-4 px-4 py-3 transition-colors">
              <span className="truncate font-medium">{task.title}</span>
              <StatusBadge status={task.status} />
            </Card>
          </Link>
        </li>
      ))}
    </ul>
  )
}

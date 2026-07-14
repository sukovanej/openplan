import { useMemo } from "react"
import { Link, useParams } from "react-router-dom"

import { DetailSkeleton, Message } from "@/components/states"
import { StatusBadge } from "@/components/status-badge"
import { TaskBody } from "@/components/task-body"
import { TaskNotFound, type TaskView } from "@/lib/api"
import { errorText } from "@/lib/format"
import { taskQuery, useQuery } from "@/lib/store"

export function DetailRoute() {
  const { id } = useParams()
  const task = useQuery(useMemo(() => taskQuery(id ?? ""), [id]))
  switch (task._tag) {
    case "loading":
      return <DetailSkeleton />
    case "failure":
      return task.error instanceof TaskNotFound
        ? <NotFound id={task.error.id} />
        : <Message title="Could not load task" detail={errorText(task.error)} />
    case "success":
      return <TaskDetail task={task.value} />
  }
}

function TaskDetail({ task }: { task: TaskView }) {
  return (
    <div className="space-y-6">
      <BackLink />
      <div className="flex flex-wrap items-center gap-3">
        <h1 className="text-2xl font-semibold tracking-tight">{task.title}</h1>
        <StatusBadge status={task.status} />
      </div>
      <TaskBody markdown={stripTitle(task.body)} />
    </div>
  )
}

function NotFound({ id }: { id: string }) {
  return (
    <div className="space-y-4">
      <BackLink />
      <Message title="Task not found" detail={id} />
    </div>
  )
}

function BackLink() {
  return (
    <Link to="/" className="text-muted-foreground text-sm hover:underline">
      ← All tasks
    </Link>
  )
}

function stripTitle(body: string): string {
  const lines = body.split("\n")
  const first = lines.findIndex((line) => line.trim().length > 0)
  if (first >= 0 && lines[first].startsWith("# ")) {
    lines.splice(first, 1)
  }
  return lines.join("\n").trim()
}

import { useMemo, useRef } from "react"
import { Link, useParams, useSearchParams } from "react-router-dom"

import { BranchSwitcher } from "@/components/branch-switcher"
import { DetailSkeleton, Message } from "@/components/states"
import { StatusIcon } from "@/components/status-badge"
import { TaskBody } from "@/components/task-body"
import { type TaskDetail, TaskNotFound } from "@/lib/api"
import { errorText } from "@/lib/format"
import { taskQuery, useQuery } from "@/lib/store"

export function DetailRoute() {
  const { id = "" } = useParams()
  // The selected branch lives in the URL (`?branch=`), so it is shareable and resets to the
  // headline on navigation without a render lag; absent means the headline (current-worktree)
  // version.
  const [params, setParams] = useSearchParams()
  const branch = params.get("branch") ?? undefined
  const onSelect = (next: string | undefined) =>
    setParams(next === undefined ? {} : { branch: next }, { replace: true })

  const task = useQuery(useMemo(() => taskQuery(id, branch), [id, branch]))

  // Switching branch mints a fresh query that starts in `loading`. Keep the last loaded version on
  // screen while it resolves — a branch click updates the card in place instead of flashing the
  // skeleton. Only a first load, or a different task, falls back to the skeleton.
  const lastShown = useRef<{ id: string; value: TaskDetail } | null>(null)
  if (task._tag === "success") lastShown.current = { id, value: task.value }

  if (task._tag === "failure") {
    return task.error instanceof TaskNotFound
      ? <NotFound id={task.error.id} />
      : <Message title="Could not load task" detail={errorText(task.error)} />
  }
  const shown = task._tag === "success"
    ? task.value
    : lastShown.current?.id === id
    ? lastShown.current.value
    : null
  return shown === null
    ? <DetailSkeleton />
    : <TaskDetailView task={shown} selected={branch} onSelect={onSelect} />
}

function TaskDetailView(
  { task, selected, onSelect }: {
    task: TaskDetail
    selected: string | undefined
    onSelect: (branch: string | undefined) => void
  },
) {
  return (
    <div className="space-y-6">
      <BackLink />
      <div className="bg-muted/30 overflow-hidden rounded-lg border p-6">
        <div className="mb-4 flex items-center gap-3">
          <StatusIcon status={task.status} />
          <h1 className="text-2xl font-semibold tracking-tight">{task.title}</h1>
        </div>
        <BranchSwitcher
          branches={task.branches}
          selected={selected}
          headline={task.headline}
          onSelect={onSelect}
        />
        <TaskBody markdown={stripTitle(task.body)} />
      </div>
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

import { useMemo, useRef } from "react"
import { Link, useParams, useSearchParams } from "react-router-dom"

import { BranchSwitcher } from "@/components/branch-switcher"
import { BodySkeleton, DetailSkeleton, Message } from "@/components/states"
import { StatusIcon } from "@/components/status-badge"
import { TaskBody } from "@/components/task-body"
import { type TaskDetail, type TaskListItem, TaskNotFound } from "@/lib/api"
import { errorText } from "@/lib/format"
import { listItem, taskQuery, useQuery } from "@/lib/store"

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
  // The list cache already holds the header fields (title, status, branches); seed from it so the
  // header renders instantly and only the body streams in, instead of flashing a full skeleton.
  const seed = shown ?? listItem(id)
  if (seed === undefined) return <DetailSkeleton />
  return (
    <TaskDetailView
      task={seed}
      body={shown?.body}
      selected={branch}
      onSelect={onSelect}
    />
  )
}

function TaskDetailView(
  { task, body, selected, onSelect }: {
    task: TaskDetail | TaskListItem
    body: string | undefined
    selected: string | undefined
    onSelect: (branch: string | undefined) => void
  },
) {
  return (
    <div className="bg-muted/10 flex h-full flex-col overflow-hidden rounded-lg ring-1 ring-inset ring-border">
      <div className="bg-muted/30 flex h-11 shrink-0 items-center gap-2 border-b px-4 text-xs font-medium tracking-wide uppercase text-muted-foreground">
        <StatusIcon status={task.status} />
        {task.title}
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto p-6">
        <h1 className="mb-4 text-2xl font-semibold tracking-tight">{task.title}</h1>
        <BranchSwitcher
          branches={task.branches}
          selected={selected}
          headline={task.headline}
          onSelect={onSelect}
        />
        {body === undefined ? <BodySkeleton /> : <TaskBody markdown={stripTitle(body)} />}
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

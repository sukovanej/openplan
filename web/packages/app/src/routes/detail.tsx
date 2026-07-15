import { type FormEvent, useMemo, useRef, useState } from "react"
import { Link, useParams, useSearchParams } from "react-router-dom"

import { BranchSwitcher } from "@/components/branch-switcher"
import { BodySkeleton, DetailSkeleton, Message } from "@/components/states"
import { StatusIcon } from "@/components/status-badge"
import { TaskBody } from "@/components/task-body"
import { createTask, patchTask, type TaskDetail, type TaskListItem, TaskNotFound } from "@/lib/api"
import { errorText } from "@/lib/format"
import { childrenOf } from "@/lib/hierarchy"
import { listItem, runMutation, taskQuery, tasksQuery, useQuery } from "@/lib/store"

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
        <Breadcrumb id={task.id} />
        <h1 className="mb-4 text-2xl font-semibold tracking-tight">{task.title}</h1>
        <BranchSwitcher
          branches={task.branches}
          selected={selected}
          headline={task.headline}
          onSelect={onSelect}
        />
        <Hierarchy id={task.id} />
        {body === undefined ? <BodySkeleton /> : <TaskBody markdown={stripTitle(body)} />}
      </div>
    </div>
  )
}

// The ancestor chain (root → parent), each a clickable link, so a subtask shows where it lives.
// Cycle-safe: a corrupt parent loop stops at the first repeat.
function Breadcrumb({ id }: { id: string }) {
  const tasks = useQuery(tasksQuery)
  if (tasks._tag !== "success") return null
  const byId = new Map(tasks.value.map((task) => [task.id, task]))
  const chain: TaskListItem[] = []
  const seen = new Set<string>([id])
  let cursor = byId.get(id)?.parent
  while (cursor !== undefined && byId.has(cursor) && !seen.has(cursor)) {
    seen.add(cursor)
    const ancestor = byId.get(cursor)!
    chain.unshift(ancestor)
    cursor = ancestor.parent
  }
  if (chain.length === 0) return null
  return (
    <nav className="text-muted-foreground mb-2 flex flex-wrap items-center gap-1 text-sm">
      {chain.map((ancestor) => (
        <span key={ancestor.id} className="flex items-center gap-1">
          <Link to={`/task/${ancestor.id}`} className="hover:text-foreground hover:underline">
            {ancestor.title}
          </Link>
          <span aria-hidden>/</span>
        </span>
      ))}
    </nav>
  )
}

// Reparent / unparent controls plus the direct children, all derived from the live task list so the
// section stays in sync with external edits.
function Hierarchy({ id }: { id: string }) {
  const tasks = useQuery(tasksQuery)
  if (tasks._tag !== "success") return null
  const all = tasks.value
  const self = all.find((task) => task.id === id)
  const children = childrenOf(all, id)
  const descendants = descendantIds(all, id)
  const parentOptions = all.filter((task) => task.id !== id && !descendants.has(task.id))

  return (
    <section className="border-border/60 mb-6 space-y-4 rounded-md border p-4">
      <ParentControl
        id={id}
        currentParent={self?.parent}
        options={parentOptions}
      />
      <ChildrenList items={children} />
      <CreateChild parentId={id} />
    </section>
  )
}

function ParentControl(
  { id, currentParent, options }: {
    id: string
    currentParent: string | undefined
    options: ReadonlyArray<TaskListItem>
  },
) {
  const [pending, setPending] = useState(false)
  const reparent = (value: string) => {
    setPending(true)
    runMutation(patchTask(id, { parent: value === "" ? null : value }))
      .catch(() => {})
      .finally(() => setPending(false))
  }
  return (
    <label className="flex flex-wrap items-center gap-2 text-sm">
      <span className="text-muted-foreground">Parent</span>
      <select
        value={currentParent ?? ""}
        disabled={pending}
        onChange={(event) => reparent(event.target.value)}
        className="border-border bg-background max-w-full rounded-md border px-2 py-1 text-sm disabled:opacity-50"
      >
        <option value="">— none (top level) —</option>
        {options.map((option) => (
          <option key={option.id} value={option.id}>
            {option.title}
          </option>
        ))}
      </select>
    </label>
  )
}

function ChildrenList({ items }: { items: ReadonlyArray<TaskListItem> }) {
  if (items.length === 0) {
    return <p className="text-muted-foreground text-sm">No subtasks.</p>
  }
  return (
    <div className="space-y-1">
      <p className="text-muted-foreground text-xs font-medium tracking-wide uppercase">Subtasks</p>
      <ul className="space-y-0.5">
        {items.map((child) => (
          <li key={child.id}>
            <Link
              to={`/task/${child.id}`}
              className="hover:bg-muted/40 flex items-center gap-2 rounded-md px-2 py-1 text-sm"
            >
              <StatusIcon status={child.status} className="size-4" />
              <span className="truncate">{child.title}</span>
            </Link>
          </li>
        ))}
      </ul>
    </div>
  )
}

function CreateChild({ parentId }: { parentId: string }) {
  const [title, setTitle] = useState("")
  const [pending, setPending] = useState(false)
  const submit = (event: FormEvent) => {
    event.preventDefault()
    const trimmed = title.trim()
    if (trimmed === "" || pending) return
    setPending(true)
    runMutation(createTask({ title: trimmed, parent: parentId }))
      .then(() => setTitle(""))
      .catch(() => {})
      .finally(() => setPending(false))
  }
  return (
    <form onSubmit={submit} className="flex flex-wrap items-center gap-2">
      <input
        value={title}
        onChange={(event) => setTitle(event.target.value)}
        placeholder="New subtask title"
        disabled={pending}
        className="border-border bg-background min-w-0 flex-1 rounded-md border px-2 py-1 text-sm disabled:opacity-50"
      />
      <button
        type="submit"
        disabled={pending || title.trim() === ""}
        className="border-border hover:bg-muted/40 rounded-md border px-3 py-1 text-sm disabled:opacity-50"
      >
        Add subtask
      </button>
    </form>
  )
}

function descendantIds(tasks: ReadonlyArray<TaskListItem>, root: string): Set<string> {
  const children = new Map<string, string[]>()
  for (const task of tasks) {
    if (task.parent !== undefined) {
      const bucket = children.get(task.parent)
      if (bucket === undefined) children.set(task.parent, [task.id])
      else bucket.push(task.id)
    }
  }
  const out = new Set<string>()
  const stack = [root]
  while (stack.length > 0) {
    const current = stack.pop()!
    for (const child of children.get(current) ?? []) {
      if (!out.has(child)) {
        out.add(child)
        stack.push(child)
      }
    }
  }
  return out
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

import { Check, CloudOff, CloudUpload, LoaderCircle, TriangleAlert } from "lucide-react"
import { useRef, useState } from "react"
import { Link } from "react-router-dom"

import type { Published } from "@openplan/api-client"
import { ChangeMark, ROLLING_UPDATES_LABEL, taskPath } from "@openplan/task-ui"
import { Button, cn, CountPill, Tooltip, useDismissOnOutsideClick } from "@openplan/ui"

import { useConnection } from "../lib/connection"
import {
  conflicted,
  pendingCount,
  type ProjectUpdates,
  type SyncState,
  syncState,
  usePublish,
  useRollingUpdates,
} from "../lib/rolling-updates"

const icons = {
  idle: Check,
  pending: CloudUpload,
  syncing: LoaderCircle,
  blocked: TriangleAlert,
  offline: CloudOff,
} satisfies Record<SyncState, typeof Check>

const colors: Record<SyncState, string> = {
  idle: "text-muted-foreground/60",
  pending: "text-info",
  syncing: "text-info",
  blocked: "text-warning",
  offline: "text-muted-foreground/40",
}

function tooltip(state: SyncState, count: number): string {
  switch (state) {
    case "idle":
      return "Nothing to publish."
    case "pending":
      return `${count} ${count === 1 ? "change" : "changes"} ready to publish.`
    case "syncing":
      return "Publishing."
    case "blocked":
      return "A conflict holds the rolling-updates branch."
    case "offline":
      return "The daemon is down."
  }
}

export function RollingUpdates() {
  const updates = useRollingUpdates()
  const live = useConnection() === "live"
  const publish = usePublish()
  const [open, setOpen] = useState(false)
  const root = useRef<HTMLDivElement>(null)
  useDismissOnOutsideClick(root, open ? () => setOpen(false) : undefined)

  if (updates.length === 0) return null
  const count = pendingCount(updates)
  const state = syncState(updates, live, publish.isPending)
  const Icon = icons[state]

  return (
    <div
      ref={root}
      className="relative"
      onKeyDown={(event) => {
        if (event.key === "Escape") setOpen(false)
      }}
    >
      <Tooltip content={tooltip(state, count)}>
        <Button
          aria-label={ROLLING_UPDATES_LABEL}
          aria-expanded={open}
          disabled={state === "offline"}
          onClick={() => setOpen(!open)}
          className={cn("gap-1.5 px-1.5 py-1.5", colors[state])}
        >
          <Icon className={cn("size-4", state === "syncing" && "animate-spin")} aria-hidden />
          {count > 0 && state !== "offline" && (
            <CountPill count={count} className={cn(state === "blocked" ? "text-warning" : "text-info", "bg-muted")} />
          )}
        </Button>
      </Tooltip>
      {open && (
        <div className="bg-popover absolute top-full right-0 z-30 mt-1.5 w-96 rounded-md border p-2 shadow-md">
          <Review updates={updates} publish={publish} />
        </div>
      )}
    </div>
  )
}

type Publish = ReturnType<typeof usePublish>

function Review({ updates, publish }: { updates: ReadonlyArray<ProjectUpdates>; publish: Publish }) {
  const named = updates.length > 1
  const empty = conflicted(updates).length === 0 && pendingCount(updates) === 0
  if (empty) {
    return (
      <p className="text-muted-foreground px-2 py-3 text-xs">
        Nothing to publish. Every edit is on the default branch.
      </p>
    )
  }
  return (
    <div className="flex flex-col gap-3">
      {updates
        .filter((one) => one.conflict !== undefined || one.pending.length > 0)
        .map((one) => (
          <Group key={one.project} updates={one} named={named} publish={publish} />
        ))}
    </div>
  )
}

function Group({ updates, named, publish }: { updates: ProjectUpdates; named: boolean; publish: Publish }) {
  const { project, pending, conflict } = updates
  const published = publish.variables === project && publish.data !== undefined ? publish.data : undefined
  return (
    <section className="flex flex-col gap-1.5">
      {named && <h2 className="text-muted-foreground px-2 text-xs font-medium">{project}</h2>}
      <ul className="flex flex-col">
        {pending.map((cell) => (
          <li key={cell.task.id}>
            <Link
              to={taskPath(project, cell.task.id)}
              className="hover:bg-muted flex items-baseline gap-2 rounded-sm px-2 py-1 text-sm"
            >
              <span className="text-muted-foreground shrink-0 text-xs">{cell.task.id}</span>
              <span className="min-w-0 flex-1 truncate">{cell.task.title}</span>
              <ChangeMark kind={cell.kind} />
            </Link>
          </li>
        ))}
      </ul>
      {conflict === undefined ? (
        <Publisher project={project} count={pending.length} published={published} publish={publish} />
      ) : (
        <Blocked files={conflict.files} worktree={conflict.worktree} />
      )}
    </section>
  )
}

function Publisher({
  project,
  count,
  published,
  publish,
}: {
  project: string
  count: number
  published: Published | undefined
  publish: Publish
}) {
  // The count does not fall on a publish. It falls when a person merges the pull request and the
  // branch rebases onto the moved default branch.
  return (
    <div className="flex flex-col items-start gap-0.5 px-2">
      <Button
        variant="accent"
        disabled={publish.isPending || count === 0}
        onClick={() => publish.mutate(project)}
        className="-mx-2 disabled:opacity-40"
      >
        {publish.isPending ? "Publishing" : `Publish ${count} ${count === 1 ? "change" : "changes"}`}
      </Button>
      {published === undefined ? (
        <span className="text-muted-foreground text-xs">Pushes a branch and opens a pull request.</span>
      ) : (
        <span className="text-muted-foreground text-xs">
          Pushed {published.branch} to {published.remote}.{" "}
          {published.pull_request != null && (
            <a href={published.pull_request} target="_blank" rel="noreferrer" className="text-info underline">
              Open the pull request
            </a>
          )}
        </span>
      )}
    </div>
  )
}

function Blocked({ files, worktree }: { files: ReadonlyArray<string>; worktree: string }) {
  return (
    <div className="border-warning/40 bg-warning/5 mx-2 flex flex-col gap-1 rounded-md border px-2 py-1.5 text-xs">
      <p className="text-warning">A rebase stopped. Fix these files, then continue it.</p>
      <ul className="text-muted-foreground font-mono">
        {files.map((file) => (
          <li key={file} className="truncate">
            {file}
          </li>
        ))}
      </ul>
      <div className="text-muted-foreground mt-1 flex flex-col border-t pt-1 font-mono">
        <code className="break-all">cd {worktree}</code>
        <code>git rebase --continue</code>
      </div>
    </div>
  )
}

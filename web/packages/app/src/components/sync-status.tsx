import { useQuery } from "@tanstack/react-query"
import { CloudAlert, CloudCheck, CloudOff, CloudSync, LoaderCircle, RefreshCw } from "lucide-react"
import { useRef, useState } from "react"
import { Link } from "react-router-dom"

import { activityPath, conflictCount, statusField, TaskIdentity, taskPath } from "@openplan/task-ui"
import { Button, cn, CountPill, TimeAgo, Tooltip, useDismissOnOutsideClick } from "@openplan/ui"

import { listTasks } from "../lib/api"
import { useConnection } from "../lib/connection"
import { tasksKey } from "../lib/query-client"
import { runtime } from "../lib/runtime"
import {
  type ProjectSync,
  type SyncState,
  syncResultText,
  syncState,
  tasksInConflict,
  useProjectSyncs,
  useSyncNow,
  waitingCount,
} from "../lib/sync"

export const SYNC_LABEL = "Sync with the remote"

const icons = {
  idle: CloudCheck,
  waiting: CloudSync,
  syncing: LoaderCircle,
  failed: CloudAlert,
  offline: CloudOff,
} satisfies Record<SyncState, typeof CloudCheck>

const colors: Record<SyncState, string> = {
  idle: "text-muted-foreground/60",
  waiting: "text-info",
  syncing: "text-info",
  failed: "text-warning",
  offline: "text-muted-foreground/40",
}

function tooltip(state: SyncState, count: number): string {
  switch (state) {
    case "idle":
      return "The tasks are in sync with the remote."
    case "waiting":
      return `${count} ${count === 1 ? "revision waits" : "revisions wait"} for the next sync.`
    case "syncing":
      return "Syncing."
    case "failed":
      return "The last sync failed."
    case "offline":
      return "The daemon is down."
  }
}

export function SyncStatus() {
  const syncs = useProjectSyncs()
  const live = useConnection() === "live"
  const syncNow = useSyncNow()
  const [open, setOpen] = useState(false)
  const root = useRef<HTMLDivElement>(null)
  useDismissOnOutsideClick(root, open ? () => setOpen(false) : undefined)

  if (syncs.length === 0) return null
  const count = waitingCount(syncs)
  const state = syncState(syncs, live, syncNow.isPending)
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
          aria-label={SYNC_LABEL}
          aria-expanded={open}
          disabled={state === "offline"}
          onClick={() => setOpen(!open)}
          className={cn("gap-1.5 px-1.5 py-1.5", colors[state])}
        >
          <Icon className={cn("size-4", state === "syncing" && "animate-spin")} aria-hidden />
          {state === "waiting" && <CountPill count={count} className="text-info bg-muted" />}
        </Button>
      </Tooltip>
      {open && (
        <div className="bg-popover absolute top-full right-0 z-30 mt-1.5 flex w-[26rem] max-w-[calc(100vw-1rem)] flex-col gap-4 rounded-md border p-3 shadow-md">
          {syncs.map((sync) => (
            <ProjectSyncView
              key={sync.project}
              sync={sync}
              named={syncs.length > 1}
              syncNow={syncNow}
              onLeave={() => setOpen(false)}
            />
          ))}
        </div>
      )}
    </div>
  )
}

function ProjectSyncView({
  sync,
  named,
  syncNow,
  onLeave,
}: {
  sync: ProjectSync
  named: boolean
  syncNow: ReturnType<typeof useSyncNow>
  onLeave: () => void
}) {
  const { project, view } = sync
  const mine = syncNow.variables === project
  const running = mine && syncNow.isPending
  const result = mine ? syncNow.data : undefined
  return (
    <section aria-label={project} className="flex flex-col gap-2">
      {named && <h2 className="text-muted-foreground text-xs font-medium">{project}</h2>}
      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-xs">
        <dt className="text-muted-foreground">Remote</dt>
        <dd className="truncate font-mono">{view.remote}</dd>
        <dt className="text-muted-foreground">Last successful sync</dt>
        <dd>{view.last_success === undefined ? "Never" : <TimeAgo iso={view.last_success} label="Synced" />}</dd>
        <dt className="text-muted-foreground">Revisions to send</dt>
        <dd className="tabular-nums">{view.ahead}</dd>
        <dt className="text-muted-foreground">Revisions to receive</dt>
        <dd className="tabular-nums">{view.behind}</dd>
      </dl>
      {view.error !== undefined && (
        <div
          role="alert"
          className="border-warning/40 bg-warning/5 flex flex-col gap-1 rounded-md border px-2 py-1.5 text-xs"
        >
          <p className="text-warning">
            The last sync failed
            {view.last_attempt !== undefined && (
              <>
                {" "}
                <TimeAgo iso={view.last_attempt} label="Tried" />
              </>
            )}
            .
          </p>
          <p className="text-muted-foreground font-mono break-words">{view.error}</p>
        </div>
      )}
      <ConflictedTasks project={project} onLeave={onLeave} />
      <div className="-ml-2 flex items-center gap-2">
        <Button variant="accent" disabled={syncNow.isPending} onClick={() => syncNow.mutate(project)}>
          <RefreshCw className={cn("size-3.5", running && "animate-spin")} aria-hidden />
          {running ? "Syncing" : "Sync now"}
        </Button>
        {result !== undefined && !running && (
          <span className="text-muted-foreground min-w-0 text-xs">{syncResultText(result)}</span>
        )}
        <Link
          to={activityPath(project)}
          onClick={onLeave}
          className="text-muted-foreground hover:text-foreground ml-auto shrink-0 text-xs"
        >
          Activity
        </Link>
      </div>
    </section>
  )
}

// Read only while the panel is open, so the header costs no task list.
function ConflictedTasks({ project, onLeave }: { project: string; onLeave: () => void }) {
  const tasks = useQuery({
    queryKey: tasksKey(project),
    queryFn: () => runtime.runPromise(listTasks(project)),
  })
  const conflicted = tasksInConflict(tasks.data ?? [])
  if (conflicted.length === 0) return null
  return (
    <div
      role="note"
      className="border-warning/40 bg-warning/5 flex flex-col gap-1 rounded-md border px-2 py-1.5 text-xs"
    >
      <p className="text-warning">
        {conflicted.length === 1 ? "1 task holds" : `${conflicted.length} tasks hold`} conflicts from a sync.
      </p>
      <ul className="-mx-1">
        {conflicted.map((task) => (
          <li key={task.id}>
            <Link
              to={taskPath(project, task.id)}
              onClick={onLeave}
              className="hover:bg-muted flex min-w-0 items-center gap-2 rounded px-1 py-0.5"
            >
              <TaskIdentity status={statusField(task.metadata)} id={task.id} title={task.title} />
              <span className="text-warning ml-auto shrink-0 tabular-nums">{conflictCount(task.conflicts)}</span>
            </Link>
          </li>
        ))}
      </ul>
    </div>
  )
}

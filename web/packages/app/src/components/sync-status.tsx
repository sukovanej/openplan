import { CloudAlert, RefreshCw } from "lucide-react"
import { useRef, useState } from "react"
import { Link } from "react-router-dom"

import { activityPath } from "@openplan/task-ui"
import { Button, cn, CountPill, TimeAgo, Tooltip, useDismissOnOutsideClick } from "@openplan/ui"

import { useConnection } from "../lib/connection"
import { type ProjectSync, type SyncState, syncState, useProjectSyncs, useSyncNow, waitingCount } from "../lib/sync"

export const SYNC_LABEL = "Sync with the remote"

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
          <SyncIcon state={state} className="size-4" />
          {state === "waiting" && <CountPill count={count} className="text-info bg-muted" />}
        </Button>
      </Tooltip>
      {open && (
        <ul className="bg-popover absolute top-full right-0 z-30 mt-1.5 flex w-[24rem] max-w-[calc(100vw-1rem)] flex-col gap-1 rounded-md border p-2 shadow-md">
          {syncs.map((sync) => (
            <ProjectSyncRow
              key={sync.project}
              sync={sync}
              live={live}
              syncNow={syncNow}
              onLeave={() => setOpen(false)}
            />
          ))}
        </ul>
      )}
    </div>
  )
}

function SyncIcon({ state, className }: { state: SyncState; className: string }) {
  return <RefreshCw className={cn(className, state === "syncing" && "animate-spin")} aria-hidden />
}

function ProjectSyncRow({
  sync,
  live,
  syncNow,
  onLeave,
}: {
  sync: ProjectSync
  live: boolean
  syncNow: ReturnType<typeof useSyncNow>
  onLeave: () => void
}) {
  const { project, view } = sync
  const state = syncState([sync], live, syncNow.variables === project && syncNow.isPending)
  return (
    <li aria-label={project} className="flex items-center gap-3 pl-1 text-xs">
      <Link to={activityPath(project)} onClick={onLeave} className="hover:text-foreground min-w-0 truncate font-medium">
        {project}
      </Link>
      <span className="text-muted-foreground ml-auto shrink-0">
        {view.last_success === undefined ? (
          "Never synced"
        ) : (
          <>
            Synced <TimeAgo iso={view.last_success} label="Synced" />
          </>
        )}
      </span>
      {view.error !== undefined && (
        <Tooltip content={`The last sync failed: ${view.error}`}>
          <CloudAlert role="img" aria-label="The last sync failed" className="text-warning size-3.5" />
        </Tooltip>
      )}
      <Button
        disabled={state === "offline" || state === "syncing" || syncNow.isPending}
        onClick={() => syncNow.mutate(project)}
        className={colors[state]}
      >
        <SyncIcon state={state} className="size-3.5" />
        Sync now
      </Button>
    </li>
  )
}

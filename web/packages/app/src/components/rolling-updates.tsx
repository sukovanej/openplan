import { Check, ChevronRight, CloudOff, CloudUpload, LoaderCircle, Trash2, TriangleAlert } from "lucide-react"
import { useRef, useState } from "react"
import { Link } from "react-router-dom"

import type { MatrixCell, Published } from "@openplan/api-client"
import { ChangeMark, ROLLING_UPDATES_LABEL, taskPath } from "@openplan/task-ui"
import { Button, cn, CountPill, DiffView, Skeleton, Tooltip, useDismissOnOutsideClick } from "@openplan/ui"

import { useConnection } from "../lib/connection"
import {
  conflicted,
  pendingCount,
  type ProjectUpdates,
  type SyncState,
  syncState,
  useDiscard,
  usePublish,
  useRollingUpdates,
  useTaskDiff,
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
  const discard = useDiscard()
  const [open, setOpen] = useState(false)
  const root = useRef<HTMLDivElement>(null)
  useDismissOnOutsideClick(root, open ? () => setOpen(false) : undefined)

  if (updates.length === 0) return null
  const count = pendingCount(updates)
  const state = syncState(updates, live, publish.isPending || discard.isPending)
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
        <div className="bg-popover absolute top-full right-0 z-30 mt-1.5 w-[40rem] max-w-[calc(100vw-1rem)] rounded-md border p-2 shadow-md">
          <Review updates={updates} publish={publish} discard={discard} />
        </div>
      )}
    </div>
  )
}

type Publish = ReturnType<typeof usePublish>
type Discard = ReturnType<typeof useDiscard>

export function Review({
  updates,
  publish,
  discard,
}: {
  updates: ReadonlyArray<ProjectUpdates>
  publish: Publish
  discard: Discard
}) {
  const named = updates.length > 1
  const empty = conflicted(updates).length === 0 && pendingCount(updates) === 0
  // One diff at a time. Two open at once would push the rest of the list, and the button that
  // publishes them, off the bottom of the popover.
  const [open, setOpen] = useState<string>()
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
          <Group
            key={one.project}
            updates={one}
            named={named}
            publish={publish}
            discard={discard}
            open={open}
            onOpen={(row) => setOpen(row === open ? undefined : row)}
          />
        ))}
    </div>
  )
}

function Group({
  updates,
  named,
  publish,
  discard,
  open,
  onOpen,
}: {
  updates: ProjectUpdates
  named: boolean
  publish: Publish
  discard: Discard
  open: string | undefined
  onOpen: (row: string) => void
}) {
  const { project, pending, conflict } = updates
  const published = publish.variables === project && publish.data !== undefined ? publish.data : undefined
  return (
    <section className="flex flex-col gap-1.5">
      {named && <h2 className="text-muted-foreground px-2 text-xs font-medium">{project}</h2>}
      <ul className="flex flex-col">
        {pending.map((cell) => {
          const row = `${project}/${cell.task.id}`
          return (
            <Pending
              key={cell.task.id}
              project={project}
              cell={cell}
              expanded={open === row}
              onOpen={() => onOpen(row)}
              discard={discard}
            />
          )
        })}
      </ul>
      {conflict === undefined ? (
        <Publisher project={project} count={pending.length} published={published} publish={publish} discard={discard} />
      ) : (
        <Blocked project={project} files={conflict.files} worktree={conflict.worktree} discard={discard} />
      )}
    </section>
  )
}

function Pending({
  project,
  cell,
  expanded,
  onOpen,
  discard,
}: {
  project: string
  cell: MatrixCell
  expanded: boolean
  onOpen: () => void
  discard: Discard
}) {
  const [asked, setAsked] = useState(false)
  return (
    <li>
      <div className="group hover:bg-muted flex items-baseline gap-1 rounded-sm pr-2">
        <button
          type="button"
          aria-label={`Diff of ${cell.task.id}`}
          aria-expanded={expanded}
          onClick={onOpen}
          className="text-muted-foreground hover:text-foreground self-center p-1"
        >
          <ChevronRight className={cn("size-3.5 transition-transform", expanded && "rotate-90")} aria-hidden />
        </button>
        <Link to={taskPath(project, cell.task.id)} className="flex min-w-0 flex-1 items-baseline gap-2 py-1 text-sm">
          <span className="text-muted-foreground shrink-0 text-xs">{cell.task.id}</span>
          <span className="min-w-0 flex-1 truncate">{cell.task.title}</span>
        </Link>
        {/* One line box whichever tail it holds, so arming the question cannot move the rows below. */}
        <span className="flex h-5 shrink-0 items-center gap-1 self-center">
          {asked ? (
            <Confirm
              onDiscard={() => {
                setAsked(false)
                discard.mutate({ project, id: cell.task.id })
              }}
              onKeep={() => setAsked(false)}
            />
          ) : (
            <>
              <ChangeMark kind={cell.kind} />
              <Button
                size="icon"
                aria-label={`Discard the change to ${cell.task.id}`}
                disabled={discard.isPending}
                onClick={() => setAsked(true)}
                className="size-5 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
              >
                <Trash2 className="size-3.5" aria-hidden />
              </Button>
            </>
          )}
        </span>
      </div>
      {expanded && (
        <div className="bg-muted/30 mx-2 mb-1 max-h-80 overflow-auto rounded-md border">
          <Diff project={project} task={cell.task.id} />
        </div>
      )}
    </li>
  )
}

function Diff({ project, task }: { project: string; task: string }) {
  const diff = useTaskDiff(project, task)
  if (diff.isPending) return <Skeleton className="m-2 h-16" />
  if (diff.isError) {
    return <p className="text-muted-foreground px-2 py-1.5 text-xs">The diff of {task} could not be read.</p>
  }
  return <DiffView diff={diff.data} className="py-1" />
}

// The row tail itself asks, rather than a modal: a discard undoes one pending edit, and the branch
// keeps every commit it already has.
function Confirm({ onDiscard, onKeep }: { onDiscard: () => void; onKeep: () => void }) {
  return (
    <span className="flex h-5 shrink-0 items-center gap-1 text-xs">
      <span className="text-muted-foreground">Discard?</span>
      <Button variant="danger" onClick={onDiscard} className="text-danger px-1.5 py-0.5 font-medium">
        Discard
      </Button>
      <Button onClick={onKeep} className="px-1.5 py-0.5">
        Keep
      </Button>
    </span>
  )
}

function Publisher({
  project,
  count,
  published,
  publish,
  discard,
}: {
  project: string
  count: number
  published: Published | undefined
  publish: Publish
  discard: Discard
}) {
  // The count does not fall on a publish. It falls when a person merges the pull request and the
  // branch rebases onto the moved default branch.
  return (
    <div className="flex flex-col items-start gap-0.5 px-2">
      <div className="-mx-2 flex items-center gap-1">
        <Button
          variant="accent"
          disabled={publish.isPending || count === 0}
          onClick={() => publish.mutate(project)}
          className="disabled:opacity-40"
        >
          {publish.isPending ? "Publishing" : `Publish ${count} ${count === 1 ? "change" : "changes"}`}
        </Button>
        <DiscardAll project={project} label="Discard all" disabled={count === 0} discard={discard} />
      </div>
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

function DiscardAll({
  project,
  label,
  disabled,
  discard,
}: {
  project: string
  label: string
  disabled?: boolean
  discard: Discard
}) {
  const [asked, setAsked] = useState(false)
  if (asked) {
    return (
      <Confirm
        onDiscard={() => {
          setAsked(false)
          discard.mutate({ project })
        }}
        onKeep={() => setAsked(false)}
      />
    )
  }
  return (
    <Button
      disabled={disabled === true || discard.isPending}
      onClick={() => setAsked(true)}
      className="disabled:opacity-40"
    >
      {label}
    </Button>
  )
}

// The commands are what a person runs to keep the work. The discard is the other way out, and the
// only one the UI itself can take.
function Blocked({
  project,
  files,
  worktree,
  discard,
}: {
  project: string
  files: ReadonlyArray<string>
  worktree: string
  discard: Discard
}) {
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
      <div className="mt-1 flex items-center border-t pt-1">
        <DiscardAll project={project} label="Abort the rebase and discard everything" discard={discard} />
      </div>
    </div>
  )
}

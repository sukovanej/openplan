import { Bot, Tag as TagIcon } from "lucide-react"
import { memo, type MouseEvent } from "react"
import { Link, useNavigate } from "react-router-dom"

import type { HistoryEntry, RevisionView, TaskRef } from "@openplan/api-client"
import {
  AgentTag,
  DocumentChangeView,
  revisionPath,
  TagChangeView,
  TaskChangeView,
  TaskIdentity,
  UnresolvedMark,
} from "@openplan/task-ui"
import { cn, TimeAgo, Tooltip } from "@openplan/ui"

import { type ActivityRow, activityRows, changePath, taskChangeOf } from "../lib/history"

// With `task`, the list is the history of that one task: a line needs not name the task, and each
// revision opens the task as that revision left it.
export function RevisionList({
  project,
  entries,
  refs,
  task,
  selected,
}: {
  project: string
  entries: ReadonlyArray<HistoryEntry>
  // `undefined` until the board is read, and until then every task counts as one that exists.
  refs?: ReadonlyMap<string, TaskRef>
  task?: string
  selected?: string
}) {
  return (
    <ol aria-label="Revisions" className="text-sm">
      {entries.map((entry) => (
        <Revision
          key={entry.revision.id}
          project={project}
          entry={entry}
          refs={refs}
          task={task}
          current={entry.revision.id === selected}
        />
      ))}
    </ol>
  )
}

const lineKey = (line: ActivityRow): string => {
  switch (line.kind) {
    case "task":
      return `task:${line.change.task}`
    case "tag":
      return `tag:${line.change.tag}`
    case "document":
      return `document:${line.change.path}`
    case "more":
      return "more"
  }
}

// When and who lead the revision once, and its changes stand beside them one to a line. A revision
// never changes, so it renders again only when the titles on the board do.
const Revision = memo(function Revision({
  project,
  entry,
  refs,
  task,
  current,
}: {
  project: string
  entry: HistoryEntry
  refs: ReadonlyMap<string, TaskRef> | undefined
  task: string | undefined
  current: boolean
}) {
  const navigate = useNavigate()
  const lines: ReadonlyArray<ActivityRow> =
    task === undefined
      ? activityRows(entry)
      : [{ kind: "task", change: taskChangeOf(entry, task) ?? { task, kind: "modified" } }]
  const to = task === undefined ? undefined : revisionPath(project, task, entry.revision.id)
  // The revision opens from its own click, as a board row opens its task; a link in it answers its
  // own click, and a modified click is the browser's.
  const open = (event: MouseEvent<HTMLLIElement>) => {
    const link = event.target instanceof Element && event.target.closest("a") !== null
    if (to === undefined || link || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
    navigate(to)
  }
  const when = <TimeAgo iso={entry.revision.at} label="Changed" />
  return (
    <li
      onClick={open}
      className={cn(
        "flex flex-wrap items-start gap-x-3 gap-y-1 border-b px-2 py-2.5 last:border-b-0",
        to !== undefined && "hover:bg-muted/30 cursor-pointer",
        current && "bg-muted/40",
      )}
    >
      <span className="flex shrink-0 items-center gap-3 leading-6">
        <span className="text-muted-foreground text-xs whitespace-nowrap">
          {to === undefined ? (
            when
          ) : (
            <Link to={to} aria-current={current ? "page" : undefined} className="hover:text-foreground">
              {when}
            </Link>
          )}
        </span>
        <Who revision={entry.revision} compact={task !== undefined} />
      </span>
      <ul aria-label="Changes" className="flex min-w-0 flex-1 basis-40 flex-col gap-1">
        {lines.map((line) => (
          <li key={lineKey(line)} className="flex max-w-full min-w-0 flex-wrap items-center gap-x-3 gap-y-1 leading-6">
            <ChangeLine project={project} entry={entry} line={line} refs={refs} withWhat={task === undefined} />
          </li>
        ))}
      </ul>
    </li>
  )
})

function ChangeLine({
  project,
  entry,
  line,
  refs,
  withWhat,
}: {
  project: string
  entry: HistoryEntry
  line: ActivityRow
  refs: ReadonlyMap<string, TaskRef> | undefined
  withWhat: boolean
}) {
  const change = "text-foreground/80"
  switch (line.kind) {
    case "task":
      return (
        <>
          <TaskChangeView change={line.change} className={change} />
          {withWhat && <TaskName project={project} entry={entry} line={line} refs={refs} />}
        </>
      )
    case "tag":
      return (
        <>
          <TagChangeView change={line.change} className={change} />
          {withWhat && (
            <span className="flex min-w-0 items-center gap-2">
              <TagIcon aria-hidden className="text-muted-foreground size-4 shrink-0" />
              <span className="truncate">{line.change.tag}</span>
            </span>
          )}
        </>
      )
    case "document":
      return (
        <>
          <DocumentChangeView kind={line.change.kind} className={change} />
          {withWhat && (
            <span className="text-muted-foreground min-w-0 truncate font-mono text-xs">{line.change.path}</span>
          )}
        </>
      )
    case "more":
      return <span className="text-muted-foreground text-xs">and {line.count} more</span>
  }
}

// A task that is gone, or that the board does not hold yet, has no status to wear.
function TaskName({
  project,
  entry,
  line,
  refs,
}: {
  project: string
  entry: HistoryEntry
  line: Extract<ActivityRow, { kind: "task" }>
  refs: ReadonlyMap<string, TaskRef> | undefined
}) {
  const { change } = line
  const task = change.kind === "removed" ? undefined : refs?.get(change.task)
  const to = changePath(project, entry, change, refs === undefined || refs.has(change.task))
  const identity = (
    <TaskIdentity
      status={task?.status}
      mark={task === undefined ? <UnresolvedMark /> : undefined}
      id={change.task}
      title={task?.title ?? change.title}
    />
  )
  return to === undefined ? (
    identity
  ) : (
    <Link to={to} className="text-foreground/90 hover:text-foreground max-w-full min-w-0">
      {identity}
    </Link>
  )
}

// A narrow list names the agent in a tooltip, so the change keeps the room it needs.
function Who({ revision, compact }: { revision: RevisionView; compact: boolean }) {
  const author = <span className="text-foreground/90 whitespace-nowrap">{revision.author}</span>
  const agent =
    revision.agent === undefined ? undefined : compact ? (
      <Tooltip content={`via ${revision.agent}`}>
        <Bot aria-label={`via ${revision.agent}`} className="text-muted-foreground size-3.5 shrink-0" />
      </Tooltip>
    ) : (
      <AgentTag agent={revision.agent} />
    )
  return (
    <span className="flex min-w-0 flex-wrap items-center gap-x-1.5 gap-y-1 leading-5">
      {revision.email === undefined ? author : <Tooltip content={revision.email}>{author}</Tooltip>}
      {agent}
    </span>
  )
}

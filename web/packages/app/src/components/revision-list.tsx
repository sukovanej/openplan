import { Bot, FileText, FolderGit2, Tag as TagIcon } from "lucide-react"
import { memo, type MouseEvent, useMemo } from "react"
import { Link, useLocation, useNavigate } from "react-router-dom"

import type { DocChange, HistoryEntry, RevisionView, TagChange, TagView, TaskRef } from "@openplan/api-client"
import {
  AgentTag,
  DocChangeView,
  DocumentChangeView,
  docRevisionPath,
  revisionPath,
  TagChangeView,
  TagChip,
  TaskChangeView,
  TaskIdentity,
  UnresolvedMark,
} from "@openplan/task-ui"
import { cn, HoverCard, MetaItem, TimeAgo, Tooltip } from "@openplan/ui"

import {
  type ActivityRow,
  activityRows,
  changePath,
  diffTarget,
  docChangePath,
  type ProjectEntry,
  taskChangeOf,
} from "../lib/history"
import { type RevisionNavigation, revisionNavigation } from "../lib/revision-navigation"
import { useTagRegistries, useTags } from "../lib/tags"
import { ChangeDiff } from "./change-diff"

// With `task` or `doc`, the list is the history of that one task or doc: a line needs not name it,
// and each revision opens it as that revision left it.
export function RevisionList({
  project,
  entries,
  refs,
  docTitles,
  task,
  doc,
  selected,
}: {
  project: string
  entries: ReadonlyArray<HistoryEntry>
  // `undefined` until the board is read, and until then every task counts as one that exists.
  refs?: ReadonlyMap<string, TaskRef>
  // The title of each doc by its name. `undefined` until the docs are read, and until then every doc
  // counts as one that exists.
  docTitles?: ReadonlyMap<string, string>
  task?: string
  doc?: string
  selected?: string
}) {
  const { state } = useLocation()
  const onRevision = selected !== undefined
  const navigation = useMemo(() => revisionNavigation(onRevision, state), [onRevision, state])
  const { byName: tags } = useTags(project)
  return (
    <ol aria-label="Revisions" className="text-sm">
      {entries.map((entry) => (
        <Revision
          key={entry.revision.id}
          project={project}
          entry={entry}
          refs={refs}
          docTitles={docTitles}
          tags={tags}
          task={task}
          doc={doc}
          current={entry.revision.id === selected}
          navigation={navigation}
          showProject={false}
        />
      ))}
    </ol>
  )
}

const NO_REFS: ReadonlyMap<string, TaskRef> = new Map()
const NO_TITLES: ReadonlyMap<string, string> = new Map()

// The revisions of every project, each read against the board, the docs, and the tags of its own.
export function MergedRevisionList({
  entries,
  refs,
  docTitles,
}: {
  entries: ReadonlyArray<ProjectEntry>
  refs?: ReadonlyMap<string, ReadonlyMap<string, TaskRef>>
  docTitles?: ReadonlyMap<string, ReadonlyMap<string, string>>
}) {
  const { state } = useLocation()
  const navigation = useMemo(() => revisionNavigation(false, state), [state])
  const projects = useMemo(() => [...new Set(entries.map((entry) => entry.project))], [entries])
  const tags = useTagRegistries(projects)
  return (
    <ol aria-label="Revisions" className="text-sm">
      {entries.map(({ project, entry }) => (
        <Revision
          key={`${project} ${entry.revision.id}`}
          project={project}
          entry={entry}
          refs={refs === undefined ? undefined : (refs.get(project) ?? NO_REFS)}
          docTitles={docTitles === undefined ? undefined : (docTitles.get(project) ?? NO_TITLES)}
          tags={tags[project]}
          task={undefined}
          doc={undefined}
          current={false}
          navigation={navigation}
          showProject
        />
      ))}
    </ol>
  )
}

const LINE = "flex min-h-6 max-w-full min-w-0 flex-wrap items-center gap-x-3 gap-y-1 leading-6"

const lineKey = (line: ActivityRow): string => {
  switch (line.kind) {
    case "task":
      return `task:${line.change.task}`
    case "tag":
      return `tag:${line.change.tag}`
    case "doc":
      return `doc:${line.change.doc}`
    case "document":
      return `document:${line.change.path}`
    case "more":
      return "more"
  }
}

// When and who lead the revision once, and its changes stand beside them one to a line. A revision
// never changes, so it renders again only when the titles on the board, the doc titles, or the tag
// registry do.
const Revision = memo(function Revision({
  project,
  entry,
  refs,
  docTitles,
  tags,
  task,
  doc,
  current,
  navigation,
  showProject,
}: {
  project: string
  entry: HistoryEntry
  refs: ReadonlyMap<string, TaskRef> | undefined
  docTitles: ReadonlyMap<string, string> | undefined
  tags: ReadonlyMap<string, TagView> | undefined
  task: string | undefined
  doc: string | undefined
  current: boolean
  navigation: RevisionNavigation
  showProject: boolean
}) {
  const navigate = useNavigate()
  const one = task !== undefined || doc !== undefined
  const lines: ReadonlyArray<ActivityRow> =
    task !== undefined
      ? [{ kind: "task", change: taskChangeOf(entry, task) ?? { task, kind: "modified" } }]
      : doc !== undefined
        ? // The daemon keeps the one change of the doc, under the name it had at that revision.
          [{ kind: "doc", change: entry.docs[0] ?? { doc, kind: "modified" } }]
        : activityRows(entry)
  const to =
    task !== undefined
      ? revisionPath(project, task, entry.revision.id)
      : doc !== undefined
        ? docRevisionPath(project, doc, entry.revision.id)
        : undefined
  // The revision opens from its own click, as a board row opens its task; a link in it answers its
  // own click, and a modified click is the browser's.
  const open = (event: MouseEvent<HTMLLIElement>) => {
    const link = event.target instanceof Element && event.target.closest("a") !== null
    if (to === undefined || link || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
    navigate(to, navigation)
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
      <span className="flex min-h-6 shrink-0 items-center gap-3">
        <span className="text-muted-foreground text-xs whitespace-nowrap">
          {to === undefined ? (
            when
          ) : (
            <Link
              to={to}
              replace={navigation.replace}
              state={navigation.state}
              aria-current={current ? "page" : undefined}
              className="hover:text-foreground"
            >
              {when}
            </Link>
          )}
        </span>
        <Who revision={entry.revision} compact={one} />
        {showProject && (
          <MetaItem icon={FolderGit2} className="text-muted-foreground text-xs">
            {project}
          </MetaItem>
        )}
      </span>
      <ul aria-label="Changes" className="flex min-w-0 flex-1 basis-40 flex-col gap-1">
        {lines.map((line) => {
          const content = (
            <ChangeLine
              project={project}
              entry={entry}
              line={line}
              refs={refs}
              docTitles={docTitles}
              tags={tags}
              withWhat={!one}
            />
          )
          const target = diffTarget(entry, line)
          return target === undefined ? (
            <li key={lineKey(line)} className={LINE}>
              {content}
            </li>
          ) : (
            <li key={lineKey(line)}>
              <HoverCard
                label={`Diff of ${target.path}`}
                content={<ChangeDiff project={project} revision={entry.revision.id} target={target} />}
                className={LINE}
                cardClassName="max-h-80 w-[40rem] max-w-[calc(100vw-12px)]"
              >
                {content}
              </HoverCard>
            </li>
          )
        })}
      </ul>
    </li>
  )
})

function ChangeLine({
  project,
  entry,
  line,
  refs,
  docTitles,
  tags,
  withWhat,
}: {
  project: string
  entry: HistoryEntry
  line: ActivityRow
  refs: ReadonlyMap<string, TaskRef> | undefined
  docTitles: ReadonlyMap<string, string> | undefined
  tags: ReadonlyMap<string, TagView> | undefined
  withWhat: boolean
}) {
  const change = "text-foreground/80"
  switch (line.kind) {
    case "task":
      return (
        <>
          <TaskChangeView change={line.change} tags={tags} className={change} />
          {withWhat && <TaskName project={project} entry={entry} line={line} refs={refs} />}
        </>
      )
    case "tag":
      return (
        <>
          <TagChangeView change={line.change} tags={tags} className={change} />
          {withWhat && <TagName change={line.change} tags={tags} />}
        </>
      )
    case "doc":
      return (
        <>
          <DocChangeView change={line.change} className={change} />
          {withWhat && <DocName project={project} entry={entry} change={line.change} titles={docTitles} />}
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

// A rename already shows the tag under both of its names.
function TagName({ change, tags }: { change: TagChange; tags: ReadonlyMap<string, TagView> | undefined }) {
  if (tags === undefined) {
    return (
      <span className="flex min-w-0 items-center gap-2">
        <TagIcon aria-hidden className="text-muted-foreground size-4 shrink-0" />
        <span className="truncate">{change.tag}</span>
      </span>
    )
  }
  return change.renamed_from === undefined ? <TagChip name={change.tag} tag={tags.get(change.tag)} /> : null
}

function DocName({
  project,
  entry,
  change,
  titles,
}: {
  project: string
  entry: HistoryEntry
  change: DocChange
  titles: ReadonlyMap<string, string> | undefined
}) {
  const title = change.kind === "removed" ? undefined : titles?.get(change.doc)
  const to = docChangePath(project, entry, change, titles === undefined || titles.has(change.doc))
  const name = (
    <span className="flex min-w-0 items-center gap-2">
      <FileText aria-hidden className="text-muted-foreground size-4 shrink-0" />
      <span className="truncate">{title ?? change.doc}</span>
    </span>
  )
  return to === undefined ? (
    name
  ) : (
    <Link to={to} className="text-foreground/90 hover:text-foreground max-w-full min-w-0">
      {name}
    </Link>
  )
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

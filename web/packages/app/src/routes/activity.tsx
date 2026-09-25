import { useQuery } from "@tanstack/react-query"
import { memo } from "react"
import { Link, useParams } from "react-router-dom"

import type { Board, FieldError, HistoryEntry, TaskRef } from "@openplan/api-client"
import {
  boardPath,
  ChangeMark,
  RevisionMeta,
  shortRevision,
  statusField,
  tagChangeText,
  taskChangeText,
  TaskRefChip,
} from "@openplan/task-ui"
import { EmptyState, Panel, PanelBody, PanelHeader, PanelTitle, Row, SkeletonList } from "@openplan/ui"

import { OlderRevisions } from "../components/older-revisions"
import { getBoard } from "../lib/api"
import { errorText } from "../lib/format"
import { changePath, otherChanges, useProjectHistory } from "../lib/history"
import { demotedReason, useProject, useProjects } from "../lib/projects"
import { boardKey } from "../lib/query-client"
import { useRowCursor } from "../lib/row-cursor"
import { abortable } from "../lib/runtime"

// This page holds no task rows, and the cursor is the board's — left as it was, `j` then Enter here
// would open a task the reader can no longer see.
const NO_ROWS: ReadonlyArray<string> = []
// One revision can write a hundred tasks at once, and the list is for reading what happened.
const SHOWN_CHANGES = 12
const UNREAD: FieldError = { kind: "missing" }

// The revisions name tasks by key alone, and the board holds each one's title and status.
const refsOf = (board: Board): ReadonlyMap<string, TaskRef> =>
  new Map(
    board.groups.flatMap((group) =>
      group.rows.map(({ task }): [string, TaskRef] => [
        task.id,
        { id: task.id, title: task.title, status: statusField(task.metadata) ?? UNREAD },
      ]),
    ),
  )

export function ActivityRoute() {
  const { project = "" } = useParams()
  const projects = useProjects()
  const known = useProject(project)
  useRowCursor(NO_ROWS)

  // Until the list arrives every name is equally plausible, so an unknown one is only unknown once
  // the daemon has answered.
  if (projects !== undefined && known === undefined) {
    return <EmptyState title="No such project" detail={project} />
  }
  const reason = demotedReason(known)
  if (reason !== undefined) {
    return <EmptyState title={`${project} is not being served`} detail={reason} />
  }
  return <Activity project={project} />
}

function Activity({ project }: { project: string }) {
  const history = useProjectHistory(project)
  const refs = useQuery({
    queryKey: boardKey(project),
    queryFn: abortable(getBoard(project)),
    select: refsOf,
  }).data
  return (
    <Panel>
      <PanelHeader className="gap-3">
        <PanelTitle>Activity</PanelTitle>
        <Link to={boardPath(project)} className="text-muted-foreground hover:text-foreground ml-auto text-xs">
          ← {project}
        </Link>
      </PanelHeader>
      <PanelBody className="p-6">
        {history.isPending ? (
          <SkeletonList count={5} className="h-16 w-full" />
        ) : history.isError ? (
          <EmptyState title="Could not load the activity" detail={errorText(history.error)} />
        ) : history.data.length === 0 ? (
          <p className="text-muted-foreground text-sm">No revisions yet.</p>
        ) : (
          <>
            <ol aria-label="Revisions">
              {history.data.map((entry, index) => (
                <li key={entry.revision.id}>
                  <Revision project={project} entry={entry} refs={refs} last={index === history.data.length - 1} />
                </li>
              ))}
            </ol>
            <OlderRevisions history={history} className="mt-3" />
          </>
        )}
      </PanelBody>
    </Panel>
  )
}

// A revision never changes, so a row renders again only when the titles on the board do.
const Revision = memo(function Revision({
  project,
  entry,
  refs,
  last,
}: {
  project: string
  entry: HistoryEntry
  // `undefined` until the board is read, and until then every task counts as one that exists.
  refs: ReadonlyMap<string, TaskRef> | undefined
  last: boolean
}) {
  const others = otherChanges(entry)
  const shownTasks = entry.tasks.slice(0, SHOWN_CHANGES)
  const shownTags = entry.tags.slice(0, SHOWN_CHANGES - shownTasks.length)
  const shownOthers = others.slice(0, SHOWN_CHANGES - shownTasks.length - shownTags.length)
  const shown = shownTasks.length + shownTags.length + shownOthers.length
  const hidden = entry.tasks.length + entry.tags.length + others.length - shown
  return (
    <Row variant="divided" last={last} className="flex flex-col gap-1.5 px-2 py-3">
      {shown === 0 ? (
        <span className="text-muted-foreground font-mono text-sm">{shortRevision(entry.revision.id)}</span>
      ) : (
        <ul aria-label="Changes" className="flex flex-col gap-1.5">
          {shownTasks.map((change) => {
            const task = refs?.get(change.task)
            const to = changePath(project, entry, change, refs === undefined || task !== undefined)
            return (
              <li key={change.task} className="flex max-w-full min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
                {to === undefined ? (
                  <span className="text-muted-foreground text-xs tabular-nums">{change.task}</span>
                ) : (
                  <TaskRefChip to={to} id={change.task} task={change.kind === "removed" ? undefined : task} />
                )}
                <span className="text-muted-foreground min-w-0 text-xs">{taskChangeText(change)}</span>
              </li>
            )
          })}
          {shownTags.map((change) => (
            <li key={change.tag} className="text-muted-foreground text-xs">
              {tagChangeText(change)}
            </li>
          ))}
          {shownOthers.map((change) => (
            <li key={change.path} className="flex max-w-full min-w-0 items-center gap-1.5">
              <ChangeMark kind={change.kind} />
              <span className="text-muted-foreground truncate font-mono text-xs">{change.path}</span>
            </li>
          ))}
          {hidden > 0 && <li className="text-muted-foreground text-xs">and {hidden} more</li>}
        </ul>
      )}
      <RevisionMeta revision={entry.revision} />
    </Row>
  )
})

import { useQuery } from "@tanstack/react-query"
import { Link, useParams } from "react-router-dom"

import type { Board, FieldError, TaskRef } from "@openplan/api-client"
import { boardPath, statusField } from "@openplan/task-ui"
import { EmptyState, Panel, PanelBody, PanelHeader, PanelTitle, SkeletonList } from "@openplan/ui"

import { OlderRevisions } from "../components/older-revisions"
import { RevisionList } from "../components/revision-list"
import { getBoard } from "../lib/api"
import { errorText } from "../lib/format"
import { useProjectHistory } from "../lib/history"
import { demotedReason, useProject, useProjects } from "../lib/projects"
import { boardKey } from "../lib/query-client"
import { useRowCursor } from "../lib/row-cursor"
import { abortable } from "../lib/runtime"

// This page holds no task rows, and the cursor is the board's — left as it was, `j` then Enter here
// would open a task the reader can no longer see.
const NO_ROWS: ReadonlyArray<string> = []
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

export function Activity({ project }: { project: string }) {
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
            <RevisionList project={project} entries={history.data} refs={refs} />
            <OlderRevisions history={history} className="mt-3" />
          </>
        )}
      </PanelBody>
    </Panel>
  )
}

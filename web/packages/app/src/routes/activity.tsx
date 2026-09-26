import { type UseInfiniteQueryResult, useQuery } from "@tanstack/react-query"
import type { ReactNode } from "react"
import { Link, useParams } from "react-router-dom"

import type { Board, DocListItem, FieldError, TaskRef } from "@openplan/api-client"
import { boardPath, statusField } from "@openplan/task-ui"
import { EmptyState, Panel, PanelBody, PanelHeader, PanelTitle, SkeletonList } from "@openplan/ui"

import { OlderRevisions } from "../components/older-revisions"
import { MergedRevisionList, RevisionList } from "../components/revision-list"
import { getBoard, getMergedBoard, listAllDocs } from "../lib/api"
import { errorText } from "../lib/format"
import { useMergedHistory, useProjectHistory } from "../lib/history"
import { demotedReason, useProject, useProjects } from "../lib/projects"
import { allDocsKey, boardKey, mergedBoardKey } from "../lib/query-client"
import { useRowCursor } from "../lib/row-cursor"
import { abortable } from "../lib/runtime"

// This page holds no task rows, and the cursor is the board's — left as it was, `j` then Enter here
// would open a task the reader can no longer see.
const NO_ROWS: ReadonlyArray<string> = []
const UNREAD: FieldError = { kind: "missing" }
const EVERY_PROJECT = "All projects"

// The revisions name tasks by key alone, and the board holds each one's title and status. Two
// projects can hold the same key, so each project has a map of its own.
const refsByProject = (board: Board): ReadonlyMap<string, ReadonlyMap<string, TaskRef>> => {
  const refs = new Map<string, Map<string, TaskRef>>()
  for (const { task } of board.groups.flatMap((group) => group.rows)) {
    const held = refs.get(task.project) ?? new Map<string, TaskRef>()
    held.set(task.id, { id: task.id, title: task.title, status: statusField(task.metadata) ?? UNREAD })
    refs.set(task.project, held)
  }
  return refs
}

const titlesByProject = (docs: ReadonlyArray<DocListItem>): ReadonlyMap<string, ReadonlyMap<string, string>> => {
  const titles = new Map<string, Map<string, string>>()
  for (const doc of docs) {
    const held = titles.get(doc.project) ?? new Map<string, string>()
    held.set(doc.name, doc.title)
    titles.set(doc.project, held)
  }
  return titles
}

export function ActivityRoute() {
  const { project } = useParams()
  useRowCursor(NO_ROWS)
  return project === undefined ? <EveryActivity /> : <ProjectActivity project={project} />
}

function ProjectActivity({ project }: { project: string }) {
  const projects = useProjects()
  const known = useProject(project)
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
    select: (board: Board) => refsByProject(board).get(project) ?? new Map<string, TaskRef>(),
  }).data
  const docTitles = useQuery({
    queryKey: allDocsKey(project),
    queryFn: abortable(listAllDocs([project])),
    select: (docs: ReadonlyArray<DocListItem>) => titlesByProject(docs).get(project) ?? new Map<string, string>(),
  }).data
  return (
    <ActivityPanel project={project}>
      <Revisions history={history}>
        {(entries) => <RevisionList project={project} entries={entries} refs={refs} docTitles={docTitles} />}
      </Revisions>
    </ActivityPanel>
  )
}

// A project the daemon does not serve has no history to read, and one failed read would fail them
// all.
function EveryActivity() {
  const projects = useProjects()
  if (projects === undefined) {
    return (
      <ActivityPanel project={undefined}>
        <SkeletonList count={5} className="h-16 w-full" />
      </ActivityPanel>
    )
  }
  const served = projects.filter((project) => demotedReason(project) === undefined).map((project) => project.name)
  return <MergedActivity projects={served} />
}

function MergedActivity({ projects }: { projects: ReadonlyArray<string> }) {
  const history = useMergedHistory(projects)
  const refs = useQuery({
    queryKey: mergedBoardKey,
    queryFn: abortable(getMergedBoard),
    select: refsByProject,
  }).data
  const docTitles = useQuery({
    queryKey: allDocsKey(),
    queryFn: abortable(listAllDocs([])),
    select: titlesByProject,
  }).data
  return (
    <ActivityPanel project={undefined}>
      <Revisions history={history}>
        {(entries) => <MergedRevisionList entries={entries} refs={refs} docTitles={docTitles} />}
      </Revisions>
    </ActivityPanel>
  )
}

function ActivityPanel({ project, children }: { project: string | undefined; children: ReactNode }) {
  return (
    <Panel>
      <PanelHeader className="gap-3">
        <PanelTitle>Activity</PanelTitle>
        <Link to={boardPath(project)} className="text-muted-foreground hover:text-foreground ml-auto text-xs">
          ← {project ?? EVERY_PROJECT}
        </Link>
      </PanelHeader>
      <PanelBody className="p-6">{children}</PanelBody>
    </Panel>
  )
}

function Revisions<T>({
  history,
  children,
}: {
  history: UseInfiniteQueryResult<ReadonlyArray<T>>
  children: (entries: ReadonlyArray<T>) => ReactNode
}) {
  if (history.isPending) return <SkeletonList count={5} className="h-16 w-full" />
  if (history.isError) return <EmptyState title="Could not load the activity" detail={errorText(history.error)} />
  if (history.data.length === 0) return <p className="text-muted-foreground text-sm">No revisions yet.</p>
  return (
    <>
      {children(history.data)}
      <OlderRevisions history={history} className="mt-3" />
    </>
  )
}

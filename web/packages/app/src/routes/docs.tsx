import { useQuery } from "@tanstack/react-query"
import { FileText, FolderGit2 } from "lucide-react"
import { memo, useMemo } from "react"
import { Link, useSearchParams } from "react-router-dom"

import type { DocListItem } from "@openplan/api-client"
import {
  ConflictBadge,
  docCreatedOf,
  docPath,
  docProblems,
  docsPath,
  ProblemBadge,
  TaskAuthor,
  TaskTimes,
} from "@openplan/task-ui"
import {
  cn,
  EmptyState,
  MetaItem,
  MetaLine,
  Panel,
  PanelBody,
  PanelHeader,
  PanelTitle,
  SkeletonList,
  Tooltip,
} from "@openplan/ui"

import { ChildGuide, GridRow, RowGrid, type RowSlot, TreeGuides } from "../components/row-grid"
import { listAllDocs } from "../lib/api"
import { docTree, type DocTreeRow } from "../lib/doc-tree"
import { errorText } from "../lib/format"
import { demotedReason, useProjects } from "../lib/projects"
import { allDocsKey } from "../lib/query-client"
import { useRowCursor } from "../lib/row-cursor"
import { abortable } from "../lib/runtime"

const ALL = "All projects"
const NO_ROWS: ReadonlyArray<string> = []

export function DocsRoute() {
  const [params] = useSearchParams()
  const project = params.get("project") ?? undefined
  const projects = useProjects()
  const docs = useQuery({
    queryKey: allDocsKey(project),
    queryFn: abortable(listAllDocs(project === undefined ? [] : [project])),
  })

  const known = projects?.find((entry) => entry.name === project)
  if (project !== undefined && projects !== undefined && known === undefined) {
    return <EmptyState title="No such project" detail={project} />
  }
  if (docs.isError) {
    return <EmptyState title="Could not load docs" detail={errorText(docs.error)} />
  }
  if (docs.isPending) return <DocsSkeleton project={project} />
  return <DocGrid docs={docs.data} project={project} />
}

function DocsSkeleton({ project }: { project: string | undefined }) {
  useRowCursor(NO_ROWS)
  return (
    <Panel>
      <PanelHeader className="gap-3">
        <PanelTitle>Docs</PanelTitle>
        <ProjectFilter selected={project} />
      </PanelHeader>
      <PanelBody className="p-6">
        <SkeletonList count={3} className="h-10 w-full" />
      </PanelBody>
    </Panel>
  )
}

const docPathOf = (row: DocTreeRow) => docPath(row.doc.project, row.doc.name)
const docDepthOf = (row: DocTreeRow) => row.depth

function DocGrid({ docs, project }: { docs: ReadonlyArray<DocListItem>; project: string | undefined }) {
  const rows = useMemo(() => docTree(docs), [docs])
  const groups = useMemo(() => [{ key: "docs", label: "Docs", rows }], [rows])
  return (
    <RowGrid
      label="Docs"
      title="Docs"
      action={<ProjectFilter selected={project} />}
      groups={groups}
      lead={
        rows.length === 0 && (
          <p className="text-muted-foreground p-6 text-sm">
            {project === undefined ? "No docs yet." : `No docs in ${project} yet.`} Create one with{" "}
            <code>openplan doc create</code>.
          </p>
        )
      }
      pathOf={docPathOf}
      depthOf={docDepthOf}
    >
      {(row, slot) => <DocRow {...slot} row={row} showProject={project === undefined} />}
    </RowGrid>
  )
}

// The same shape as the header's project switcher, because it answers the same question — which
// store am I looking at — for a page that is not under one.
function ProjectFilter({ selected }: { selected: string | undefined }) {
  const projects = useProjects()
  if (projects === undefined || projects.length === 0) return null
  return (
    <nav
      aria-label="Filter by project"
      className="bg-muted ml-auto flex min-w-0 items-center gap-0.5 overflow-x-auto rounded-md p-0.5"
    >
      <Entry to={docsPath()} label={ALL} active={selected === undefined} />
      {projects.map((project) => (
        <Entry
          key={project.name}
          to={docsPath(project.name)}
          label={project.name}
          active={selected === project.name}
          reason={demotedReason(project)}
        />
      ))}
    </nav>
  )
}

function Entry({ to, label, active, reason }: { to: string; label: string; active: boolean; reason?: string }) {
  const link = (
    <Link
      to={to}
      aria-current={active ? "page" : undefined}
      className={cn(
        "focus-visible:ring-ring inline-flex items-center gap-1.5 rounded-sm px-2 py-1 text-xs whitespace-nowrap transition-colors focus-visible:ring-2 focus-visible:outline-none",
        active ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground",
      )}
    >
      {label}
    </Link>
  )
  return reason === undefined ? link : <Tooltip content={reason}>{link}</Tooltip>
}

const DocRow = memo(function DocRow({
  row,
  showProject,
  guides,
  ...slot
}: RowSlot & { row: DocTreeRow; showProject: boolean }) {
  const { doc } = row
  return (
    <GridRow slot={slot}>
      <TreeGuides columns={guides.columns} />
      <div role="gridcell" className="relative flex shrink-0 items-center self-stretch">
        <FileText className="text-muted-foreground size-4" />
        {guides.opensChildren && <ChildGuide />}
      </div>
      <div className="min-w-0 grow pr-4 pl-3" role="gridcell">
        <Link
          to={slot.path}
          tabIndex={-1}
          className="text-foreground/90 block line-clamp-2 text-sm font-normal sm:line-clamp-none sm:truncate"
        >
          {doc.title || doc.name}
        </Link>
        <MetaLine>
          {doc.conflicts > 0 && <ConflictBadge count={doc.conflicts} />}
          {doc.problems.length > 0 && <ProblemBadge problems={doc.problems} />}
          {showProject && <MetaItem icon={FolderGit2}>{doc.project}</MetaItem>}
          <TaskAuthor author={doc.author} withAgent={false} />
          <TaskTimes created={docCreatedOf(doc.metadata)} updated={doc.updated} problems={docProblems(doc.metadata)} />
        </MetaLine>
      </div>
    </GridRow>
  )
})

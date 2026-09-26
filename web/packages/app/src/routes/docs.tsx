import { useQuery } from "@tanstack/react-query"
import { FileText, FolderGit2 } from "lucide-react"
import { memo, useMemo } from "react"
import { Link, useParams } from "react-router-dom"

import type { DocListItem } from "@openplan/api-client"
import {
  ConflictBadge,
  docCreatedOf,
  docPath,
  docProblems,
  ProblemBadge,
  TaskAuthor,
  TaskTimes,
} from "@openplan/task-ui"
import { EmptyState, MetaItem, MetaLine } from "@openplan/ui"

import { ListHeader, ListSkeleton } from "../components/list-header"
import { ChildGuide, GridRow, RowGrid, type RowSlot, TreeGuides } from "../components/row-grid"
import { listAllDocs } from "../lib/api"
import { docTree, type DocTreeRow } from "../lib/doc-tree"
import { errorText } from "../lib/format"
import { useProjects } from "../lib/projects"
import { allDocsKey } from "../lib/query-client"
import { abortable } from "../lib/runtime"

export function DocsRoute() {
  const { project } = useParams()
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
  if (docs.isPending) return <ListSkeleton view="docs" project={project} />
  return <DocGrid docs={docs.data} project={project} />
}

const docPathOf = (row: DocTreeRow) => docPath(row.doc.project, row.doc.name)
const docDepthOf = (row: DocTreeRow) => row.depth

function DocGrid({ docs, project }: { docs: ReadonlyArray<DocListItem>; project: string | undefined }) {
  const rows = useMemo(() => docTree(docs), [docs])
  const groups = useMemo(() => [{ key: "docs", label: "Docs", rows }], [rows])
  return (
    <RowGrid
      label="Docs"
      header={<ListHeader view="docs" project={project} />}
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

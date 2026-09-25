import { useParams, useSearchParams } from "react-router-dom"

import { REVISION_PARAM } from "@openplan/task-ui"

import { TaskAtRevision, TaskView } from "./task-view"

export function DetailRoute() {
  const { project = "", id = "" } = useParams()
  return <DetailPage key={`${project}:${id}`} project={project} id={id} />
}

// The revision lives in the URL, so the browser's Back returns to the version the reader saw before.
function DetailPage({ project, id }: { project: string; id: string }) {
  const [params] = useSearchParams()
  const revision = params.get(REVISION_PARAM)
  if (revision !== null) return <TaskAtRevision project={project} id={id} revision={revision} />
  return <TaskView project={project} id={id} />
}

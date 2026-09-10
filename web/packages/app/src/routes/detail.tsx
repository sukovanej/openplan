import { useParams, useSearchParams } from "react-router-dom"

import { TaskView } from "./task-view"

export function DetailRoute() {
  const { project = "", id = "" } = useParams()
  // The selected branch lives in the URL (`?branch=`), so it is shareable and resets to the
  // headline on navigation without a render lag; absent means the headline (current-worktree)
  // version.
  const [params, setParams] = useSearchParams()
  const branch = params.get("branch") ?? undefined
  const onSelect = (next: string | undefined) =>
    setParams(next === undefined ? {} : { branch: next }, { replace: true })
  return <TaskView key={`${project}:${id}`} project={project} id={id} branch={branch} onSelect={onSelect} agentLink />
}

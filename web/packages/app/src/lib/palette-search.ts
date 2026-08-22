import type { SearchHit } from "@open-planner/api-client"
import { taskPath } from "@open-planner/task-ui"

// Where selecting a hit goes. A hit found on a branch other than the headline pins that branch, so
// the page shows the version the query matched rather than a different one under the same key.
export function hitPath(hit: SearchHit): string {
  const path = taskPath(hit.task.project, hit.task.id)
  return hit.branch === hit.task.headline ? path : `${path}?branch=${encodeURIComponent(hit.branch)}`
}

// A key is unique only inside its project, and one task can be hit on one branch at a time, so the
// three together name a row.
export function hitKey(hit: SearchHit): string {
  return `${hit.task.project} ${hit.task.id} ${hit.branch}`
}

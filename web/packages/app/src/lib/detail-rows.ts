import type { Field_Status, TaskDetail, TaskRef } from "@open-planner/api-client"
import { dependenciesOf, taskPath, taskReference } from "@open-planner/task-ui"

// One row of the three task lists below the task body. A dependency the store cannot resolve has no
// title and no status to show — only the key the file spells.
export interface DetailRow {
  readonly path: string
  readonly id: string
  readonly title: string | undefined
  readonly status: Field_Status | undefined
  readonly unresolved: boolean
}

// The three lists in document order, plus their rows as the one sequence a single cursor walks.
export interface DetailRows {
  readonly dependsOn: ReadonlyArray<DetailRow>
  readonly blocks: ReadonlyArray<DetailRow>
  readonly subtasks: ReadonlyArray<DetailRow>
  readonly paths: ReadonlyArray<string>
}

function resolvedRow(project: string, task: TaskRef): DetailRow {
  return { path: taskPath(project, task.id), id: task.id, title: task.title, status: task.status, unresolved: false }
}

// Every dependency the file lists, in the order it lists them, because the author wrote that
// sequence. One that names no task stays in the list: the task waits for it either way.
function dependsOnRows(project: string, detail: TaskDetail): ReadonlyArray<DetailRow> {
  const resolved = new Map((detail.depends_on ?? []).map((ref) => [ref.id, ref]))
  return dependenciesOf(detail.metadata).map((entry) => {
    const { id, section } = taskReference(entry)
    const ref = resolved.get(id)
    // The section is the row's link, not its name: the row names the task, which is what waits.
    const path = taskPath(project, id, section)
    return ref === undefined
      ? { path, id, title: undefined, status: undefined, unresolved: true }
      : { ...resolvedRow(project, ref), path }
  })
}

export function detailRows(project: string, detail: TaskDetail | null): DetailRows {
  if (detail === null) return { dependsOn: [], blocks: [], subtasks: [], paths: [] }
  const dependsOn = dependsOnRows(project, detail)
  const blocks = (detail.blocks ?? []).map((ref) => resolvedRow(project, ref))
  const subtasks = (detail.children ?? []).map((child) => resolvedRow(project, child))
  return { dependsOn, blocks, subtasks, paths: [...dependsOn, ...blocks, ...subtasks].map((row) => row.path) }
}

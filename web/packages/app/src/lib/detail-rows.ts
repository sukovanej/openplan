import type { Field_Status, TaskDetail, TaskRef } from "@openplan/api-client"
import { dependenciesOf, taskPath, taskReference } from "@openplan/task-ui"

// One row of the three task lists below the task body. `at` is the row's place in the page-wide
// sequence that one cursor walks, so a section renders its rows without knowing where its own list
// starts. A dependency the store cannot resolve has no title and no status to show — only the key
// the file spells.
export interface DetailRow {
  readonly at: number
  readonly path: string
  readonly id: string
  readonly title: string | undefined
  readonly status: Field_Status | undefined
  readonly unresolved: boolean
}

// The three lists in document order, plus their rows as the one sequence the cursor walks.
export interface DetailRows {
  readonly dependsOn: ReadonlyArray<DetailRow>
  readonly blocks: ReadonlyArray<DetailRow>
  readonly subtasks: ReadonlyArray<DetailRow>
  readonly paths: ReadonlyArray<string>
}

export function detailRows(project: string, detail: TaskDetail | null): DetailRows {
  const resolved = new Map((detail?.depends_on ?? []).map((ref) => [ref.id, ref]))
  let next = 0
  const row = (id: string, path: string, task: TaskRef | undefined): DetailRow => ({
    at: next++,
    path,
    id,
    title: task?.title,
    status: task?.status,
    unresolved: task === undefined,
  })
  // Every dependency the file lists, in the order it lists them, because the author wrote that
  // sequence. One that names no task keeps its row: the task waits for it either way. A section
  // (`OPP-42#Design`) is the row's link, not its name — the row names the task, which is what waits.
  const dependsOn = (detail === null ? [] : dependenciesOf(detail.metadata)).map((entry) => {
    const { id, section } = taskReference(entry)
    return row(id, taskPath(project, id, section), resolved.get(id))
  })
  const blocks = (detail?.blocks ?? []).map((task) => row(task.id, taskPath(project, task.id), task))
  const subtasks = (detail?.children ?? []).map((task) => row(task.id, taskPath(project, task.id), task))
  return { dependsOn, blocks, subtasks, paths: [...dependsOn, ...blocks, ...subtasks].map((each) => each.path) }
}

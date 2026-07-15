import type { TaskListItem } from "./api"

// Siblings sort by `rank` ascending; a task without a rank sorts last, ties broken by id — the same
// order the CLI/`tree` endpoint use, so the UI matches server output.
export function siblingCompare(a: TaskListItem, b: TaskListItem): number {
  const ra = a.rank
  const rb = b.rank
  if (ra !== undefined && rb !== undefined) {
    if (ra !== rb) return ra < rb ? -1 : 1
  } else if (ra !== undefined) {
    return -1
  } else if (rb !== undefined) {
    return 1
  }
  return a.id < b.id ? -1 : a.id > b.id ? 1 : 0
}

export function childrenOf(
  tasks: ReadonlyArray<TaskListItem>,
  parentId: string,
): TaskListItem[] {
  return tasks.filter((task) => task.parent === parentId).sort(siblingCompare)
}

export interface ForestRow {
  readonly task: TaskListItem
  readonly depth: number
  readonly hasChildren: boolean
}

// A pre-order flattening of the parent→child forest: each task is immediately followed by its
// `rank`-ordered subtree. A task whose `parent` is absent from the set (top-level, or a dangling
// pointer) is placed at the root. Cycle-safe via a visited set so a corrupt parent loop can't hang
// the render.
export function forestRows(tasks: ReadonlyArray<TaskListItem>): ForestRow[] {
  const byId = new Set(tasks.map((task) => task.id))
  const children = new Map<string, TaskListItem[]>()
  const roots: TaskListItem[] = []
  for (const task of tasks) {
    if (task.parent !== undefined && byId.has(task.parent)) {
      const bucket = children.get(task.parent)
      if (bucket === undefined) children.set(task.parent, [task])
      else bucket.push(task)
    } else {
      roots.push(task)
    }
  }
  roots.sort(siblingCompare)
  for (const bucket of children.values()) bucket.sort(siblingCompare)

  const rows: ForestRow[] = []
  const seen = new Set<string>()
  const visit = (task: TaskListItem, depth: number): void => {
    if (seen.has(task.id)) return
    seen.add(task.id)
    const kids = children.get(task.id) ?? []
    rows.push({ task, depth, hasChildren: kids.length > 0 })
    for (const kid of kids) visit(kid, depth + 1)
  }
  for (const root of roots) visit(root, 0)
  // A task trapped in a parent cycle (no reachable root) would otherwise vanish; surface any such
  // leftover at the top level so every task is always shown exactly once.
  for (const task of tasks) {
    if (!seen.has(task.id)) visit(task, 0)
  }
  return rows
}

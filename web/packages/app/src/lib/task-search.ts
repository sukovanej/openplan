import type { TaskListItem } from "@open-planner/api-client"
import { fuzzyMatch } from "@open-planner/ui"

const MAX_MATCHES = 8

// A query is a key search only once it carries a whole prefix and its separator, so pasting `OPP-42`
// finds that task while `o`, `op`, and `opp` stay title searches — every id starts with the
// abbreviation, so treating those as key matches would swallow the entire list. Case-insensitive: the
// prefix is the only part a typist has to shout.
const KEY_QUERY = /^[A-Za-z]{3}-[0-9]*$/

export function keyMatch(id: string, query: string): boolean {
  return KEY_QUERY.test(query) && id.toLowerCase().startsWith(query.toLowerCase())
}

export interface TaskMatch {
  readonly task: TaskListItem
  readonly indices: ReadonlyArray<number>
}

// Ranked matches over the task list, minus `excluded`: a key the query prefixes comes first, then
// fuzzy title matches. Capped so the popover stays scannable.
export function taskMatches(tasks: ReadonlyArray<TaskListItem>, query: string, excluded: Set<string>): TaskMatch[] {
  const scored: Array<{ task: TaskListItem; score: number; indices: ReadonlyArray<number> }> = []
  for (const task of tasks) {
    if (excluded.has(task.id)) continue
    if (query !== "" && keyMatch(task.id, query)) {
      scored.push({ task, score: -1, indices: [] })
      continue
    }
    const match = fuzzyMatch(query, task.title)
    if (match !== null) scored.push({ task, score: match.score, indices: match.indices })
  }
  scored.sort((a, b) => a.score - b.score || a.task.title.localeCompare(b.task.title))
  return scored.slice(0, MAX_MATCHES).map(({ task, indices }) => ({ task, indices }))
}

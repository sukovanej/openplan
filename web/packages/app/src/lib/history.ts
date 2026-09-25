import { type InfiniteData, useInfiniteQuery, useQuery } from "@tanstack/react-query"
import type { Effect } from "effect"
import type { HttpClient } from "effect/unstable/http"

import type { DocumentChange, DocumentChangeKind, HistoryEntry } from "@openplan/api-client"
import { revisionPath, taskPath } from "@openplan/task-ui"

import { type ApiError, getProjectHistory, getTaskHistory, getTaskRevision, type HistoryPage } from "./api"
import { historyKey, revisionKey, taskHistoryKey } from "./query-client"
import { abortable, runtime } from "./runtime"

export const PROJECT_HISTORY_PAGE = 50
export const TASK_HISTORY_PAGE = 20

// A page shorter than the limit is the last one. A full page can be the last one too; the read after
// it then comes back empty, and the paging stops there.
export function olderThan(page: ReadonlyArray<HistoryEntry>, limit: number): string | undefined {
  return page.length < limit ? undefined : page.at(-1)?.revision.id
}

type Read = (page: HistoryPage) => Effect.Effect<ReadonlyArray<HistoryEntry>, ApiError, HttpClient.HttpClient>

const everyEntry = (data: InfiniteData<ReadonlyArray<HistoryEntry>, string | undefined>) => data.pages.flat()

function usePagedHistory(queryKey: ReadonlyArray<unknown>, read: Read, limit: number) {
  return useInfiniteQuery({
    queryKey,
    queryFn: ({ pageParam, signal }) => runtime.runPromise(read({ before: pageParam, limit }), { signal }),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (last) => olderThan(last, limit),
    select: everyEntry,
  })
}

export function useProjectHistory(project: string) {
  return usePagedHistory(historyKey(project), (page) => getProjectHistory(project, page), PROJECT_HISTORY_PAGE)
}

export function useTaskHistory(project: string, id: string) {
  return usePagedHistory(taskHistoryKey(project, id), (page) => getTaskHistory(project, id, page), TASK_HISTORY_PAGE)
}

export function useTaskRevision(project: string, id: string, revision: string) {
  return useQuery({
    queryKey: revisionKey(project, id, revision),
    queryFn: abortable(getTaskRevision(project, id, revision)),
  })
}

export interface TaskChange {
  readonly id: string
  readonly kind: DocumentChangeKind
}

export interface RevisionChanges {
  readonly tasks: ReadonlyArray<TaskChange>
  readonly others: ReadonlyArray<DocumentChange>
}

// A new title moves the task to a file with a new name, which the revision records as one file
// removed and one added. Both name the same task, and for the task that is one modification.
export function revisionChanges(entry: HistoryEntry): RevisionChanges {
  const kinds = new Map<string, DocumentChangeKind>()
  const others: Array<DocumentChange> = []
  for (const change of entry.changes) {
    if (change.task === undefined) {
      others.push(change)
      continue
    }
    const held = kinds.get(change.task)
    kinds.set(change.task, held === undefined || held === change.kind ? change.kind : "modified")
  }
  return { tasks: [...kinds].map(([id, kind]) => ({ id, kind })), others }
}

// A task that is gone has no page of its own, so its link opens the task as the revision left it — or,
// where the revision removed it, as it was just before.
export function changePath(
  project: string,
  entry: HistoryEntry,
  change: TaskChange,
  exists: boolean,
): string | undefined {
  if (change.kind !== "removed") {
    return exists ? taskPath(project, change.id) : revisionPath(project, change.id, entry.revision.id)
  }
  const before = entry.revision.parents[0]
  return before === undefined ? undefined : revisionPath(project, change.id, before)
}

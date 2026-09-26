import { type InfiniteData, useInfiniteQuery, useQuery } from "@tanstack/react-query"
import type { Effect } from "effect"
import type { HttpClient } from "effect/unstable/http"

import type { DocChange, DocumentChange, HistoryEntry, TagChange, TaskChange } from "@openplan/api-client"
import { docPath, docRevisionPath, revisionPath, taskPath } from "@openplan/task-ui"

import {
  type ApiError,
  type DiffTarget,
  getProjectHistory,
  getRevisionDiff,
  getDocHistory,
  getDocRevision,
  getTaskHistory,
  getTaskRevision,
  type HistoryPage,
} from "./api"
import { diffKey, docHistoryKey, docRevisionKey, historyKey, revisionKey, taskHistoryKey } from "./query-client"
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

export function useDocHistory(project: string, name: string) {
  return usePagedHistory(docHistoryKey(project, name), (page) => getDocHistory(project, name, page), TASK_HISTORY_PAGE)
}

export function useDocRevision(project: string, name: string, revision: string) {
  return useQuery({
    queryKey: docRevisionKey(project, name, revision),
    queryFn: abortable(getDocRevision(project, name, revision)),
  })
}

export function useTaskRevision(project: string, id: string, revision: string) {
  return useQuery({
    queryKey: revisionKey(project, id, revision),
    queryFn: abortable(getTaskRevision(project, id, revision)),
  })
}

export function useChangeDiff(project: string, revision: string, target: DiffTarget) {
  return useQuery({
    queryKey: diffKey(project, revision, target.from, target.path),
    queryFn: abortable(getRevisionDiff(project, revision, target)),
  })
}

// The daemon reads the tasks, the tags, and the docs into changes of their own, and this is the rest,
// such as the config and the assets.
export const otherChanges = (entry: HistoryEntry): ReadonlyArray<DocumentChange> =>
  entry.changes.filter((change) => change.task === undefined && change.tag === undefined && change.doc === undefined)

export const taskChangeOf = (entry: HistoryEntry, id: string): TaskChange | undefined =>
  entry.tasks.find((change) => change.task === id)

export type ActivityRow =
  | { readonly kind: "task"; readonly change: TaskChange }
  | { readonly kind: "tag"; readonly change: TagChange }
  | { readonly kind: "doc"; readonly change: DocChange }
  | { readonly kind: "document"; readonly change: DocumentChange }
  | { readonly kind: "more"; readonly count: number }

// One revision can write a hundred tasks at once, and the table is for reading what happened.
export const SHOWN_CHANGES = 12

export function activityRows(entry: HistoryEntry): ReadonlyArray<ActivityRow> {
  const rows: ReadonlyArray<ActivityRow> = [
    ...entry.tasks.map((change) => ({ kind: "task", change }) as const),
    ...entry.tags.map((change) => ({ kind: "tag", change }) as const),
    ...entry.docs.map((change) => ({ kind: "doc", change }) as const),
    ...otherChanges(entry).map((change) => ({ kind: "document", change }) as const),
  ]
  return rows.length <= SHOWN_CHANGES
    ? rows
    : [...rows.slice(0, SHOWN_CHANGES), { kind: "more", count: rows.length - SHOWN_CHANGES }]
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
    return exists ? taskPath(project, change.task) : revisionPath(project, change.task, entry.revision.id)
  }
  const before = entry.revision.parents[0]
  return before === undefined ? undefined : revisionPath(project, change.task, before)
}

// No page shows a doc as a revision left it, so only a doc that still exists has a link.
// As `changePath` does for a task: a doc that is gone opens as the revision before the delete left it.
export function docChangePath(
  project: string,
  entry: HistoryEntry,
  change: DocChange,
  exists: boolean,
): string | undefined {
  if (change.kind !== "removed") {
    return exists ? docPath(project, change.doc) : docRevisionPath(project, change.doc, entry.revision.id)
  }
  const before = entry.revision.parents[0]
  return before === undefined ? undefined : docRevisionPath(project, change.doc, before)
}

// A task, a tag, or a doc that moved to a file with a new name lists two documents: the one it left,
// and the one it moved to. A sync merge that gave a task a new number moved it from a file of its old
// key.
export function diffTarget(entry: HistoryEntry, row: ActivityRow): DiffTarget | undefined {
  switch (row.kind) {
    case "task": {
      const renumbered = row.change.fields?.find((field) => field.field === "number")
      return moved(
        entry.changes.filter((change) => change.task === row.change.task),
        entry.changes.filter((change) => change.task === (renumbered?.from ?? row.change.task)),
      )
    }
    case "tag":
      return moved(
        entry.changes.filter((change) => change.tag === row.change.tag),
        entry.changes.filter((change) => change.tag === (row.change.renamed_from ?? row.change.tag)),
      )
    case "doc":
      return moved(
        entry.changes.filter((change) => change.doc === row.change.doc),
        entry.changes.filter((change) => change.doc === (row.change.renamed_from ?? row.change.doc)),
      )
    case "document":
      return { path: row.change.path }
    case "more":
      return undefined
  }
}

function moved(now: ReadonlyArray<DocumentChange>, was: ReadonlyArray<DocumentChange>): DiffTarget | undefined {
  const after = now.find((change) => change.kind !== "removed")
  const before = was.find((change) => change.kind === "removed" && change.path !== after?.path)
  if (after === undefined) return before === undefined ? undefined : { path: before.path }
  return before === undefined ? { path: after.path } : { path: after.path, from: before.path }
}

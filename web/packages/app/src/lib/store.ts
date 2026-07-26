import { Effect, Result } from "effect"
import type { HttpClient } from "effect/unstable/http"
import { useSyncExternalStore } from "react"

import { type Board, getBoard, getTask, listTasks, type TaskDetail, type TaskListItem } from "./api"
import type { Invalidator } from "./events"
import { runtime } from "./runtime"

export type QueryState<A> =
  | { readonly _tag: "loading" }
  | { readonly _tag: "success"; readonly value: A }
  | { readonly _tag: "failure"; readonly error: unknown }

interface Refreshable {
  readonly refresh: () => void
}

const mounted = new Set<Refreshable>()

export class Query<A> {
  private state: QueryState<A> = { _tag: "loading" }
  private readonly listeners = new Set<() => void>()
  private started = false
  private inFlight = false
  private token = 0

  constructor(
    private readonly effect: Effect.Effect<A, unknown, HttpClient.HttpClient>,
    private readonly onIdle?: () => void,
  ) {}

  hasListeners(): boolean {
    return this.listeners.size > 0
  }

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    mounted.add(this)
    if (!this.started) {
      this.started = true
      this.load()
    } else if (this.state._tag === "failure" && !this.inFlight) {
      this.load()
    }
    return () => {
      this.listeners.delete(listener)
      if (this.listeners.size === 0) {
        mounted.delete(this)
        this.onIdle?.()
      }
    }
  }

  readonly getSnapshot = (): QueryState<A> => this.state

  readonly refresh = (): void => {
    if (this.started) this.load()
  }

  // A run token discards a stale response that resolves after a newer refresh.
  private load(): void {
    const token = ++this.token
    this.inFlight = true
    runtime.runPromise(Effect.result(this.effect)).then(
      (result) =>
        this.settle(
          token,
          Result.isSuccess(result)
            ? { _tag: "success", value: result.success }
            : { _tag: "failure", error: result.failure },
        ),
      // Effect.result folds typed failures into the value; only a defect rejects here.
      (defect: unknown) => this.settle(token, { _tag: "failure", error: defect }),
    )
  }

  private settle(token: number, state: QueryState<A>): void {
    if (token !== this.token) return
    this.inFlight = false
    this.state = state
    for (const listener of this.listeners) listener()
  }
}

export function useQuery<A>(query: Query<A>): QueryState<A> {
  return useSyncExternalStore(query.subscribe, query.getSnapshot)
}

// The whole flat task set. Only the parent / add-subtask pickers need it, and only while open, so it
// is fetched lazily on subscribe rather than on every detail view.
export const tasksQuery = new Query(listTasks)

// The list view's grouped/ordered/nested rows — the sole always-loaded task read; the detail page
// gets its hierarchy from the per-task `taskQuery` instead.
export const boardQuery: Query<Board> = new Query(getBoard)

export function boardTasks(board: Board): ReadonlyArray<TaskListItem> {
  return board.groups.flatMap((group) => group.rows.map((row) => row.task))
}

export function listItem(id: string): TaskListItem | undefined {
  const snapshot = boardQuery.getSnapshot()
  return snapshot._tag === "success" ? boardTasks(snapshot.value).find((task) => task.id === id) : undefined
}

const MAX_TASK_QUERIES = 64
const taskQueries = new Map<string, Query<TaskDetail>>()

// A task id is a slug with no spaces, so a space cleanly separates it from an optional branch —
// one query per (id, branch) so the detail view can hold several branch versions at once.
function taskKey(id: string, branch: string | undefined): string {
  return branch === undefined ? id : `${id} ${branch}`
}

export function taskQuery(id: string, branch?: string): Query<TaskDetail> {
  const key = taskKey(id, branch)
  const existing = taskQueries.get(key)
  if (existing !== undefined) return existing
  const query = new Query(getTask(id, branch), () => taskQueries.delete(key))
  taskQueries.set(key, query)
  evictOrphanedTaskQueries(key)
  return query
}

// A concurrent render can build a query that never subscribes (so onIdle never fires); cap the
// map by dropping unmounted entries once it grows past the bound.
function evictOrphanedTaskQueries(keep: string): void {
  if (taskQueries.size <= MAX_TASK_QUERIES) return
  for (const [key, query] of taskQueries) {
    if (taskQueries.size <= MAX_TASK_QUERIES) break
    if (key !== keep && !query.hasListeners()) taskQueries.delete(key)
  }
}

function refreshTaskQueries(id: string): void {
  const prefix = `${id} `
  for (const [key, query] of taskQueries) {
    if (key === id || key.startsWith(prefix)) query.refresh()
  }
}

// The reason the last write was refused, held until it is dismissed or a later write succeeds. A
// picker's exclusions are built from a snapshot that can be stale by the time the server sees the
// write, so a refusal is reachable through the UI and must be shown rather than dropped.
class MutationError {
  private state: unknown = undefined
  private readonly listeners = new Set<() => void>()

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  readonly getSnapshot = (): unknown => this.state

  readonly report = (error: unknown): void => this.set(error)
  readonly clear = (): void => this.set(undefined)

  private set(next: unknown): void {
    if (next === this.state) return
    this.state = next
    for (const listener of this.listeners) listener()
  }
}

export const mutationError = new MutationError()

export function useMutationError(): unknown {
  return useSyncExternalStore(mutationError.subscribe, mutationError.getSnapshot)
}

// Run a write, then refresh the list and every open task detail so the change shows at once —
// without waiting for the daemon's SSE echo (which also refreshes, and covers external edits). A
// refused write refreshes too, so the view snaps back to what the server actually holds.
export function runMutation<A>(effect: Effect.Effect<A, unknown, HttpClient.HttpClient>): Promise<void> {
  const refresh = () => {
    boardQuery.refresh()
    if (tasksQuery.hasListeners()) tasksQuery.refresh()
    for (const query of taskQueries.values()) query.refresh()
  }
  return runtime.runPromise(effect).then(
    () => {
      mutationError.clear()
      refresh()
    },
    (error: unknown) => {
      mutationError.report(error)
      refresh()
    },
  )
}

export const storeInvalidator: Invalidator = {
  refreshList: () => {
    boardQuery.refresh()
    if (tasksQuery.hasListeners()) tasksQuery.refresh()
  },
  refreshTask: (id) => refreshTaskQueries(id),
  refreshVisible: () => {
    for (const query of mounted) query.refresh()
  },
}

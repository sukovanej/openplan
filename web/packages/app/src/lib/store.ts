import { Effect, Result } from "effect"
import type { HttpClient } from "effect/unstable/http"
import { useSyncExternalStore } from "react"

import { getTask, listTasks, type TaskView } from "./api"
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

  readonly subscribe = (listener: () => void): () => void => {
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

export const tasksQuery = new Query(listTasks)

const MAX_TASK_QUERIES = 64
const taskQueries = new Map<string, Query<TaskView>>()

export function taskQuery(id: string): Query<TaskView> {
  const existing = taskQueries.get(id)
  if (existing !== undefined) return existing
  const query = new Query(getTask(id), () => taskQueries.delete(id))
  taskQueries.set(id, query)
  evictOrphanedTaskQueries(id)
  return query
}

// A concurrent render can build a query that never subscribes (so onIdle never fires); cap the
// map by dropping unmounted entries once it grows past the bound.
function evictOrphanedTaskQueries(keep: string): void {
  if (taskQueries.size <= MAX_TASK_QUERIES) return
  for (const [id, query] of taskQueries) {
    if (taskQueries.size <= MAX_TASK_QUERIES) break
    if (id !== keep && !query.hasListeners()) taskQueries.delete(id)
  }
}

export const storeInvalidator: Invalidator = {
  refreshList: () => tasksQuery.refresh(),
  refreshTask: (id) => taskQueries.get(id)?.refresh(),
  refreshVisible: () => {
    for (const query of mounted) query.refresh()
  },
}

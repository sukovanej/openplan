import { Effect, Result } from "effect"
import type { HttpClient } from "effect/unstable/http"
import { useSyncExternalStore } from "react"

import { getTask, listTasks, type TaskError, type TaskView } from "./api"
import type { Invalidator } from "./events"
import { runtime } from "./runtime"

export type QueryState<A, E> =
  | { readonly _tag: "loading" }
  | { readonly _tag: "success"; readonly value: A }
  | { readonly _tag: "failure"; readonly error: E }

export class Query<A, E> {
  private state: QueryState<A, E> = { _tag: "loading" }
  private readonly listeners = new Set<() => void>()
  private started = false
  private token = 0

  constructor(private readonly effect: Effect.Effect<A, E, HttpClient.HttpClient>) {}

  readonly subscribe = (listener: () => void): () => void => {
    this.listeners.add(listener)
    if (!this.started) {
      this.started = true
      this.load()
    }
    return () => {
      this.listeners.delete(listener)
    }
  }

  readonly getSnapshot = (): QueryState<A, E> => this.state

  readonly refresh = (): void => {
    if (this.started) this.load()
  }

  // A run token discards a stale response that resolves after a newer refresh.
  private load(): void {
    const token = ++this.token
    void runtime.runPromise(Effect.result(this.effect)).then((result) => {
      if (token !== this.token) return
      this.state = Result.isSuccess(result)
        ? { _tag: "success", value: result.success }
        : { _tag: "failure", error: result.failure }
      for (const listener of this.listeners) listener()
    })
  }
}

export function useQuery<A, E>(query: Query<A, E>): QueryState<A, E> {
  return useSyncExternalStore(query.subscribe, query.getSnapshot)
}

export const tasksQuery = new Query(listTasks)

const taskQueries = new Map<string, Query<TaskView, TaskError>>()

export function taskQuery(id: string): Query<TaskView, TaskError> {
  const existing = taskQueries.get(id)
  if (existing !== undefined) return existing
  const query = new Query(getTask(id))
  taskQueries.set(id, query)
  return query
}

export const storeInvalidator: Invalidator = {
  refreshList: () => tasksQuery.refresh(),
  refreshTask: (id) => taskQueries.get(id)?.refresh(),
}

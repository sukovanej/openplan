import { Effect, Result } from "effect"
import type { HttpClient } from "effect/unstable/http"
import { useSyncExternalStore } from "react"

import type { Board, TaskDetail, TaskListItem } from "@open-planner/api-client"

import { getBoard, getMergedBoard, getTask, listProjects, listTasks } from "./api"
import type { Invalidator } from "./events"
import { projectsStore } from "./projects"
import { runtime } from "./runtime"

export type QueryState<A> =
  | { readonly _tag: "loading" }
  | { readonly _tag: "success"; readonly value: A }
  | { readonly _tag: "failure"; readonly error: unknown }

interface Refreshable {
  // The project a query reads, or `undefined` for one that reads across all of them. A change in
  // one project must leave the queries of every other project alone, and the merged reads are the
  // only ones every change touches.
  readonly project: string | undefined
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
    readonly project: string | undefined,
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

// The projects the daemon serves, and the abbreviation each spells its keys with. Read on its own
// rather than as a `Query` because nothing renders it directly: it decides how other data is read.
export function loadProjects(): void {
  void runtime.runPromise(Effect.result(listProjects)).then((result) => {
    if (Result.isSuccess(result)) projectsStore.set(result.success)
  })
}

// A per-project map rather than one query: two projects hold different task sets, and a change in
// one must not refetch the other. Each map is keyed by project name and grows only as the UI asks
// for a project.
function keyed<A>(build: (project: string) => Query<A>): (project: string) => Query<A> {
  const queries = new Map<string, Query<A>>()
  return (project) => {
    const existing = queries.get(project)
    if (existing !== undefined) return existing
    const query = build(project)
    queries.set(project, query)
    return query
  }
}

// The whole flat task set of a project. Only the parent / add-subtask pickers need it, and only
// while open, so it is fetched lazily on subscribe rather than on every detail view.
export const tasksQuery = keyed((project) => new Query(project, listTasks(project)))

// The list view's grouped/ordered/nested rows — the sole always-loaded task read; the detail page
// gets its hierarchy from the per-task `taskQuery` instead.
export const boardQuery = keyed((project) => new Query<Board>(project, getBoard(project)))

// Every servable project at once, which is what `/` opens on. It reads across all of them, so every
// project's changes refresh it.
export const mergedBoardQuery: Query<Board> = new Query(undefined, getMergedBoard)

function boardTasks(board: Board): ReadonlyArray<TaskListItem> {
  return board.groups.flatMap((group) => group.rows.map((row) => row.task))
}

// The list row a detail view can render its header from at once. Either board can hold it: the
// merged one when the detail was opened from `/`, that project's own when it was opened from
// `/:project`.
export function listItem(project: string, id: string): TaskListItem | undefined {
  for (const query of [mergedBoardQuery, boardQuery(project)]) {
    const snapshot = query.getSnapshot()
    if (snapshot._tag !== "success") continue
    const found = boardTasks(snapshot.value).find((task) => task.project === project && task.id === id)
    if (found !== undefined) return found
  }
  return undefined
}

const MAX_TASK_QUERIES = 64
const taskQueries = new Map<string, Query<TaskDetail>>()

// Neither a project name nor a key holds whitespace, so spaces cleanly separate the three parts —
// one query per (project, id, branch) so the detail view can hold several branch versions at once.
function taskKey(project: string, id: string, branch: string | undefined): string {
  const key = `${project} ${id}`
  return branch === undefined ? key : `${key} ${branch}`
}

export function taskQuery(project: string, id: string, branch?: string): Query<TaskDetail> {
  const key = taskKey(project, id, branch)
  const existing = taskQueries.get(key)
  if (existing !== undefined) return existing
  const query = new Query(project, getTask(project, id, branch), () => taskQueries.delete(key))
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

function refreshTaskQueries(project: string, id: string): void {
  const exact = taskKey(project, id, undefined)
  const prefix = `${exact} `
  for (const [key, query] of taskQueries) {
    if (key === exact || key.startsWith(prefix)) query.refresh()
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

// Run a write, then refresh the written project's reads and every open task detail of it so the
// change shows at once — without waiting for the daemon's SSE echo (which also refreshes, and
// covers external edits). A refused write refreshes too, so the view snaps back to what the server
// actually holds.
export function runMutation<A>(
  project: string,
  effect: Effect.Effect<A, unknown, HttpClient.HttpClient>,
): Promise<void> {
  const refresh = () => {
    refreshBoards(project)
    for (const query of taskQueries.values()) {
      if (query.project === project) query.refresh()
    }
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

function refreshBoards(project: string): void {
  mergedBoardQuery.refresh()
  boardQuery(project).refresh()
  const tasks = tasksQuery(project)
  if (tasks.hasListeners()) tasks.refresh()
}

export const storeInvalidator: Invalidator = {
  refreshProjects: loadProjects,
  refreshList: refreshBoards,
  refreshTask: refreshTaskQueries,
  // A project's own reads plus the merged ones, which every project's changes touch. Without a
  // project, everything on screen: nothing says which project the change was in.
  refreshVisible: (project) => {
    for (const query of mounted) {
      if (project === undefined || query.project === undefined || query.project === project) query.refresh()
    }
  },
}

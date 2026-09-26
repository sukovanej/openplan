import {
  focusManager,
  type InfiniteData,
  type Query,
  QueryClient,
  useMutation,
  useQueryClient,
} from "@tanstack/react-query"
import { Effect } from "effect"
import type { HttpClient } from "effect/unstable/http"

import { connectionStore } from "./connection"
import type { Invalidator } from "./events"
import { readSelection, selectionShows } from "./flow-selection"
import { runtime } from "./runtime"

export type Write = Effect.Effect<unknown, unknown, HttpClient.HttpClient>

const REVISION = "revision"

export const projectKey = (project: string) => ["project", project] as const
export const projectsKey = ["projects"] as const
export const mergedKey = ["merged"] as const
export const projectMutationsKey = ["mutation", "project"] as const
export const mergedBoardKey = [...mergedKey, "board"] as const
// The flow spans every project a query names, so it lives beside the merged board rather than under
// one project.
export const flowsKey = [...mergedKey, "flow"] as const
export const flowKey = (query: string, width: number, height: number) => [...flowsKey, query, width, height] as const
// A drawing follows from its source alone, so no change to a task makes one stale.
export const diagramKey = (source: string) => ["diagram", source] as const
export const boardKey = (project: string) => [...projectKey(project), "board"] as const
export const tasksKey = (project: string) => [...projectKey(project), "tasks"] as const
export const tagsKey = (project: string) => [...projectKey(project), "tags"] as const
export const syncKey = (project: string) => [...projectKey(project), "sync"] as const
export const historyKey = (project: string) => [...projectKey(project), "history"] as const
export const taskKey = (project: string, id: string) => [...projectKey(project), "task", id] as const
// Under the task, so whatever re-reads the task re-reads the revisions that changed it.
export const taskHistoryKey = (project: string, id: string) => [...taskKey(project, id), "history"] as const
// Apart from the task, because a revision never changes: a change to the task leaves it as it was.
export const revisionKey = (project: string, id: string, revision: string) =>
  [...projectKey(project), REVISION, id, revision] as const

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: Infinity,
      retry: false,
      // Only a change marks a read stale, and a hidden tab leaves it so. The tab re-reads those reads
      // when it shows again, and nothing else.
      refetchOnWindowFocus: true,
      refetchOnReconnect: false,
    },
  },
})

interface Reads {
  readonly queryKey: ReadonlyArray<unknown>
  readonly predicate?: (query: Query) => boolean
}

const isPaged = (data: unknown): data is InfiniteData<unknown> =>
  typeof data === "object" && data !== null && "pages" in data && "pageParams" in data

// A paged read re-reads every page it holds, and the reader paged back through the old revisions,
// which a change does not touch. So a change keeps only the newest page to re-read.
function keepNewestPage(client: QueryClient, reads: Reads): void {
  for (const query of client.getQueryCache().findAll(reads)) {
    const data = query.state.data
    if (isPaged(data) && data.pages.length > 1) {
      client.setQueryData(query.queryKey, { pages: data.pages.slice(0, 1), pageParams: data.pageParams.slice(0, 1) })
    }
  }
}

function invalidate(client: QueryClient, reads: Reads): Promise<void> {
  keepNewestPage(client, reads)
  return client.invalidateQueries({ ...reads, refetchType: focusManager.isFocused() ? "active" : "none" })
}

const changeable = (query: Query): boolean => query.queryKey[2] !== REVISION

const flowsShowing = (project: string): Reads => ({
  queryKey: flowsKey,
  predicate: (query) => selectionShows(readSelection(new URLSearchParams(String(query.queryKey[2]))), project),
})

function refreshScreen(client: QueryClient, project?: string): Promise<unknown> {
  if (project === undefined) {
    return Promise.all([
      invalidate(client, { queryKey: ["project"], predicate: changeable }),
      invalidate(client, { queryKey: mergedKey }),
    ])
  }
  return Promise.all([
    invalidate(client, { queryKey: projectKey(project), predicate: changeable }),
    invalidate(client, { queryKey: mergedBoardKey }),
    invalidate(client, flowsShowing(project)),
  ])
}

export function useProjectMutation(project: string) {
  const client = useQueryClient()
  return useMutation({
    mutationKey: [...projectMutationsKey, project],
    mutationFn: (effect: Write) => runtime.runPromise(effect),
    // A live stream brings a write back as the events of the change, and they re-read what it changed.
    // A second re-read here would read the same things again. A refusal changes nothing and sends no
    // event, but it can mean that the screen is behind the daemon, so it re-reads the screen.
    onSettled: (_result, error) =>
      error === null && connectionStore.getSnapshot() === "live" ? undefined : refreshScreen(client, project),
  })
}

const refresh = (reads: Reads) => {
  void invalidate(queryClient, reads)
}

export const queryInvalidator: Invalidator = {
  refreshProjects: () => refresh({ queryKey: projectsKey }),
  refreshList: (project) => {
    refresh({ queryKey: boardKey(project) })
    refresh({ queryKey: tasksKey(project) })
    refresh({ queryKey: mergedBoardKey })
    refresh(flowsShowing(project))
  },
  refreshTask: (project, id) => refresh({ queryKey: taskKey(project, id) }),
  refreshHistory: (project) => refresh({ queryKey: historyKey(project) }),
  refreshSync: (project) => refresh({ queryKey: syncKey(project) }),
  refreshVisible: (project) => {
    void refreshScreen(queryClient, project)
  },
}

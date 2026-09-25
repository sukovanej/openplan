// @vitest-environment happy-dom

import { focusManager, InfiniteQueryObserver, QueryClientProvider, QueryObserver } from "@tanstack/react-query"
import { Effect } from "effect"
import { act, createElement } from "react"
import { createRoot, type Root } from "react-dom/client"
import { afterEach, describe, expect, it, vi } from "vitest"

import { MutationError } from "../src/components/mutation-error"
import { connectionStore } from "../src/lib/connection"
import {
  boardKey,
  flowKey,
  historyKey,
  mergedBoardKey,
  queryClient,
  queryInvalidator,
  revisionKey,
  taskKey,
  useProjectMutation,
} from "../src/lib/query-client"

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

let root: Root | undefined
const subscriptions: Array<() => void> = []

function observe(queryKey: ReadonlyArray<unknown>) {
  let version = 0
  const observer = new QueryObserver(queryClient, {
    queryKey,
    queryFn: async () => ++version,
  })
  subscriptions.push(observer.subscribe(() => {}))
  return observer
}

function mountMutation(project: string, showErrors = false) {
  let mutation!: ReturnType<typeof useProjectMutation>
  const Harness = ({ onRender }: { onRender: (rendered: typeof mutation) => void }) => {
    onRender(useProjectMutation(project))
    return null
  }
  const container = document.createElement("div")
  const mounted = createRoot(container)
  root = mounted
  act(() =>
    mounted.render(
      createElement(
        QueryClientProvider,
        { client: queryClient },
        createElement(Harness, {
          onRender: (rendered) => {
            mutation = rendered
          },
        }),
        showErrors ? createElement(MutationError) : undefined,
      ),
    ),
  )
  return { container, mutation: () => mutation }
}

afterEach(() => {
  for (const unsubscribe of subscriptions.splice(0)) unsubscribe()
  if (root !== undefined) {
    const mounted = root
    act(() => mounted.unmount())
    root = undefined
  }
  focusManager.setFocused(undefined)
  connectionStore.set("connecting")
  queryClient.clear()
})

const settled = () => new Promise((resume) => setTimeout(resume, 20))

describe("invalidation", () => {
  it("reloads a changed task when its detail opens again", async () => {
    const project = "deferred"
    const id = "DEF-1"
    let version = 0
    const observer = new QueryObserver(queryClient, {
      queryKey: taskKey(project, id),
      queryFn: async () => ++version,
    })

    const first = observer.subscribe(() => {})
    subscriptions.push(first)
    await vi.waitFor(() => expect(observer.getCurrentResult().data).toBe(1))
    first()

    queryInvalidator.refreshTask(project, id)
    expect(version).toBe(1)

    const second = observer.subscribe(() => {})
    subscriptions.push(second)
    await vi.waitFor(() => expect(observer.getCurrentResult().data).toBe(2))

    queryInvalidator.refreshTask(project, id)
    await vi.waitFor(() => expect(observer.getCurrentResult().data).toBe(3))
    second()
  })

  // A flow that names no project spans them all.
  it("re-reads a flow only after a change in a project that the flow shows", async () => {
    const alpha = observe(flowKey("project=alpha"))
    const every = observe(flowKey(""))
    await vi.waitFor(() => {
      expect(alpha.getCurrentResult().data).toBe(1)
      expect(every.getCurrentResult().data).toBe(1)
    })

    queryInvalidator.refreshList("beta")
    await vi.waitFor(() => expect(every.getCurrentResult().data).toBe(2))
    expect(alpha.getCurrentResult().data).toBe(1)

    queryInvalidator.refreshVisible("alpha")
    await vi.waitFor(() => {
      expect(alpha.getCurrentResult().data).toBe(2)
      expect(every.getCurrentResult().data).toBe(3)
    })

    queryInvalidator.refreshVisible()
    await vi.waitFor(() => {
      expect(alpha.getCurrentResult().data).toBe(3)
      expect(every.getCurrentResult().data).toBe(4)
    })
  })

  it("leaves a revision alone, because a revision never changes", async () => {
    const revision = observe(revisionKey("alpha", "ALP-1", "r1"))
    const board = observe(boardKey("alpha"))
    await vi.waitFor(() => expect(revision.getCurrentResult().data).toBe(1))

    queryInvalidator.refreshVisible("alpha")
    queryInvalidator.refreshVisible()
    await vi.waitFor(() => expect(board.getCurrentResult().data).toBe(3))
    expect(revision.getCurrentResult().data).toBe(1)
  })

  // The reader paged back through old revisions, and a change adds a revision at the top alone.
  it("re-reads only the newest page of a paged read after a change", async () => {
    const reads: Array<number> = []
    const history = new InfiniteQueryObserver(queryClient, {
      queryKey: historyKey("alpha"),
      queryFn: async ({ pageParam }: { pageParam: number }) => {
        reads.push(pageParam)
        return [pageParam]
      },
      initialPageParam: 0,
      getNextPageParam: (last: ReadonlyArray<number>) => last[0] + 1,
    })
    subscriptions.push(history.subscribe(() => {}))
    await vi.waitFor(() => expect(history.getCurrentResult().data?.pages).toHaveLength(1))
    await history.fetchNextPage()
    await history.fetchNextPage()
    expect(history.getCurrentResult().data?.pages).toHaveLength(3)

    reads.length = 0
    queryInvalidator.refreshHistory("alpha")
    await vi.waitFor(() => expect(history.getCurrentResult().isFetching).toBe(false))
    expect(reads).toEqual([0])
    expect(history.getCurrentResult().data?.pages).toEqual([[0]])
  })

  it("only marks a change while the tab is hidden, and re-reads it when the tab shows again", async () => {
    queryClient.mount()
    try {
      const board = observe(boardKey("alpha"))
      await vi.waitFor(() => expect(board.getCurrentResult().data).toBe(1))

      focusManager.setFocused(false)
      queryInvalidator.refreshList("alpha")
      await settled()
      expect(board.getCurrentResult().data).toBe(1)
      expect(queryClient.getQueryState(boardKey("alpha"))?.isInvalidated).toBe(true)

      focusManager.setFocused(true)
      await vi.waitFor(() => expect(board.getCurrentResult().data).toBe(2))
    } finally {
      queryClient.unmount()
    }
  })

  it("refreshes the changed project, or every project after a global change", async () => {
    const alpha = observe(boardKey("alpha"))
    const beta = observe(boardKey("beta"))
    const merged = observe(mergedBoardKey)
    await vi.waitFor(() => {
      expect(alpha.getCurrentResult().data).toBe(1)
      expect(beta.getCurrentResult().data).toBe(1)
      expect(merged.getCurrentResult().data).toBe(1)
    })

    queryInvalidator.refreshVisible("alpha")

    await vi.waitFor(() => {
      expect(alpha.getCurrentResult().data).toBe(2)
      expect(merged.getCurrentResult().data).toBe(2)
    })
    expect(beta.getCurrentResult().data).toBe(1)

    queryInvalidator.refreshVisible()

    await vi.waitFor(() => {
      expect(alpha.getCurrentResult().data).toBe(3)
      expect(beta.getCurrentResult().data).toBe(2)
      expect(merged.getCurrentResult().data).toBe(3)
    })
  })
})

describe("mutations", () => {
  // Without a live stream, no event brings the write back.
  it.each(["success", "failure"] as const)("re-reads the screen after %s while the stream is down", async (outcome) => {
    const project = "openplan"
    const board = observe(boardKey(project))
    const other = observe(boardKey("other"))
    const merged = observe(mergedBoardKey)
    await vi.waitFor(() => {
      expect(board.getCurrentResult().data).toBe(1)
      expect(other.getCurrentResult().data).toBe(1)
      expect(merged.getCurrentResult().data).toBe(1)
    })
    const mutation = mountMutation(project).mutation

    await act(async () => {
      const result = mutation().mutateAsync(outcome === "failure" ? Effect.fail(new Error("refused")) : Effect.void)
      if (outcome === "failure") await expect(result).rejects.toThrow("refused")
      else await result
    })

    expect(board.getCurrentResult().data).toBe(2)
    expect(merged.getCurrentResult().data).toBe(2)
    expect(other.getCurrentResult().data).toBe(1)
  })

  // The daemon sends the write back as the events of the change, and they re-read what changed.
  it("re-reads nothing after a write while the stream is live", async () => {
    connectionStore.set("live")
    const board = observe(boardKey("openplan"))
    await vi.waitFor(() => expect(board.getCurrentResult().data).toBe(1))
    const mutation = mountMutation("openplan").mutation

    await act(async () => mutation().mutateAsync(Effect.void))
    await settled()

    expect(board.getCurrentResult().data).toBe(1)
  })

  // A refusal sends no event, and it can mean that the screen is behind the daemon.
  it("re-reads the screen after a refusal while the stream is live", async () => {
    connectionStore.set("live")
    const board = observe(boardKey("openplan"))
    await vi.waitFor(() => expect(board.getCurrentResult().data).toBe(1))
    const mutation = mountMutation("openplan").mutation

    await act(async () => {
      await expect(mutation().mutateAsync(Effect.fail(new Error("refused")))).rejects.toThrow("refused")
    })

    expect(board.getCurrentResult().data).toBe(2)
  })

  it("shows, clears, and dismisses a refusal", async () => {
    const { container, mutation } = mountMutation("openplan", true)

    await act(async () => {
      await expect(mutation().mutateAsync(Effect.fail(new Error("first refusal")))).rejects.toThrow("first refusal")
    })
    await vi.waitFor(() => expect(container.textContent).toContain("first refusal"))

    await act(async () => mutation().mutateAsync(Effect.void))
    await vi.waitFor(() => expect(container.textContent).not.toContain("first refusal"))

    await act(async () => {
      await expect(mutation().mutateAsync(Effect.fail(new Error("second refusal")))).rejects.toThrow("second refusal")
    })
    await vi.waitFor(() => expect(container.textContent).toContain("second refusal"))

    const dismiss = container.querySelector<HTMLElement>('[aria-label="Dismiss"]')
    expect(dismiss).not.toBeNull()
    act(() => dismiss?.click())
    expect(container.textContent).not.toContain("second refusal")
  })
})

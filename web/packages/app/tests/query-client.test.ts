// @vitest-environment happy-dom

import { QueryClientProvider, QueryObserver } from "@tanstack/react-query"
import { Effect } from "effect"
import { act, createElement } from "react"
import { createRoot, type Root } from "react-dom/client"
import { afterEach, describe, expect, it, vi } from "vitest"

import { MutationError } from "../src/components/mutation-error"
import {
  boardKey,
  flowKey,
  mergedBoardKey,
  queryClient,
  queryInvalidator,
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
  const Harness = () => {
    mutation = useProjectMutation(project)
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
        createElement(Harness),
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
  queryClient.clear()
})

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

  it("re-reads the flow after any project changes, because one flow spans them all", async () => {
    const flow = observe(flowKey("project=alpha"))
    await vi.waitFor(() => expect(flow.getCurrentResult().data).toBe(1))

    queryInvalidator.refreshList("beta")
    await vi.waitFor(() => expect(flow.getCurrentResult().data).toBe(2))

    queryInvalidator.refreshVisible()
    await vi.waitFor(() => expect(flow.getCurrentResult().data).toBe(3))
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
  it.each(["success", "failure"] as const)("refreshes visible data after %s", async (outcome) => {
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

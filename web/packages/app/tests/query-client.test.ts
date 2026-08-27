// @vitest-environment happy-dom

import { QueryClientProvider, QueryObserver } from "@tanstack/react-query"
import { Effect } from "effect"
import { act, createElement } from "react"
import { createRoot, type Root } from "react-dom/client"
import { afterEach, describe, expect, it, vi } from "vitest"

import {
  boardKey,
  mergedBoardKey,
  queryClient,
  queryInvalidator,
  tagsKey,
  taskKey,
  tasksKey,
  useProjectMutation,
} from "../src/lib/query-client"

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

let root: Root | undefined

function mountMutation(project: string) {
  let mutation!: ReturnType<typeof useProjectMutation>
  const Harness = () => {
    mutation = useProjectMutation(project)
    return null
  }
  const mounted = createRoot(document.createElement("div"))
  root = mounted
  act(() => mounted.render(createElement(QueryClientProvider, { client: queryClient }, createElement(Harness))))
  return () => mutation
}

afterEach(() => {
  if (root !== undefined) {
    const mounted = root
    act(() => mounted.unmount())
    root = undefined
  }
  queryClient.clear()
})

describe("query keys", () => {
  it("separates projects and branches", () => {
    expect(boardKey("alpha")).not.toEqual(boardKey("beta"))
    expect(tasksKey("alpha")).not.toEqual(tasksKey("beta"))
    expect(tagsKey("alpha")).not.toEqual(tagsKey("alpha", "feature"))
    expect(taskKey("alpha", "APP-1")).not.toEqual(taskKey("beta", "APP-1"))
    expect(taskKey("alpha", "APP-1")).not.toEqual(taskKey("alpha", "APP-1", "feature"))
  })
})

describe("invalidation", () => {
  it("reloads an invalidated task when its next observer subscribes", async () => {
    const project = "deferred"
    const id = "DEF-1"
    let version = 0
    const observer = new QueryObserver(queryClient, {
      queryKey: taskKey(project, id),
      queryFn: async () => ++version,
    })

    const first = observer.subscribe(() => {})
    await vi.waitFor(() => expect(observer.getCurrentResult().data).toBe(1))
    first()

    queryInvalidator.refreshTask(project, id)
    await vi.waitFor(() => expect(queryClient.getQueryState(taskKey(project, id))?.isInvalidated).toBe(true))
    expect(version).toBe(1)

    const second = observer.subscribe(() => {})
    await vi.waitFor(() => expect(observer.getCurrentResult().data).toBe(2))

    queryInvalidator.refreshTask(project, id)
    await vi.waitFor(() => expect(observer.getCurrentResult().data).toBe(3))
    second()
  })

  it("invalidates one project and the merged board", async () => {
    queryClient.setQueryData(boardKey("alpha"), "alpha")
    queryClient.setQueryData(boardKey("beta"), "beta")
    queryClient.setQueryData(mergedBoardKey, "merged")

    queryInvalidator.refreshVisible("alpha")

    await vi.waitFor(() => {
      expect(queryClient.getQueryState(boardKey("alpha"))?.isInvalidated).toBe(true)
      expect(queryClient.getQueryState(mergedBoardKey)?.isInvalidated).toBe(true)
    })
    expect(queryClient.getQueryState(boardKey("beta"))?.isInvalidated).toBe(false)
  })
})

describe("mutations", () => {
  it.each(["success", "failure"] as const)("invalidates project data after %s", async (outcome) => {
    const project = "open-plan"
    queryClient.setQueryData(boardKey(project), "project")
    queryClient.setQueryData(mergedBoardKey, "merged")
    const mutation = mountMutation(project)

    await act(async () => {
      const result = mutation().mutateAsync(outcome === "failure" ? Effect.fail(new Error("refused")) : Effect.void)
      if (outcome === "failure") await expect(result).rejects.toThrow("refused")
      else await result
    })

    expect(queryClient.getQueryState(boardKey(project))?.isInvalidated).toBe(true)
    expect(queryClient.getQueryState(mergedBoardKey)?.isInvalidated).toBe(true)
  })
})

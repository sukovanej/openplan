// @vitest-environment happy-dom

import { QueryClientProvider } from "@tanstack/react-query"
import { act, createElement } from "react"
import { createRoot, type Root } from "react-dom/client"
import { MemoryRouter } from "react-router-dom"
import { afterEach, describe, expect, it, vi } from "vitest"

import { TaskHistory } from "../src/components/task-history"
import { TASK_HISTORY_PAGE } from "../src/lib/history"
import { queryClient, queryInvalidator } from "../src/lib/query-client"

const served = vi.hoisted(() => ({
  // Newest first, as the daemon answers: r24 down to r0.
  revisions: Array.from({ length: 25 }, (_, at) => `r${24 - at}`),
  pages: [] as Array<{ before?: string; limit: number }>,
}))

vi.mock("../src/lib/api", async () => {
  const { Effect } = await import("effect")
  return {
    getTaskHistory: (_project: string, _id: string, page: { before?: string; limit: number }) =>
      Effect.sync(() => {
        served.pages.push(page)
        const from = page.before === undefined ? 0 : served.revisions.indexOf(page.before) + 1
        return served.revisions.slice(from, from + page.limit).map((id) => ({
          revision: {
            id,
            parents: [],
            author: "Milan",
            agent: id === "r24" ? "claude_code" : undefined,
            at: "2026-01-02T00:00:00Z",
            message: `Change ${id}\n\nThe details.`,
          },
          changes: [{ path: "tasks/00001-first.md", kind: id === "r0" ? "added" : "modified", task: "OPP-1" }],
        }))
      }),
  }
})

let mounted: { root: Root; container: HTMLElement } | undefined

afterEach(async () => {
  const held = mounted
  mounted = undefined
  if (held !== undefined) {
    await act(async () => held.root.unmount())
    held.container.remove()
  }
  served.revisions = Array.from({ length: 25 }, (_, at) => `r${24 - at}`)
  served.pages = []
  queryClient.clear()
})

const tick = () =>
  act(async () => {
    await new Promise((resume) => setTimeout(resume, 0))
  })

const entries = (root: HTMLElement) => Array.from(root.querySelectorAll<HTMLAnchorElement>("li > a"))

async function until(check: () => boolean): Promise<void> {
  for (let attempt = 0; attempt < 20; attempt++) {
    if (check()) return
    await tick()
  }
  throw new Error("the history never settled")
}

async function show(selected: string | undefined): Promise<HTMLElement> {
  ;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true
  const container = document.createElement("div")
  document.body.append(container)
  const root = createRoot(container)
  mounted = { root, container }
  await act(async () => {
    root.render(
      createElement(
        QueryClientProvider,
        { client: queryClient },
        createElement(MemoryRouter, null, createElement(TaskHistory, { project: "openplan", id: "OPP-1", selected })),
      ),
    )
  })
  await until(() => entries(container).length > 0)
  return container
}

function older(root: HTMLElement): HTMLButtonElement | undefined {
  return Array.from(root.querySelectorAll<HTMLButtonElement>("button")).find(
    (button) => button.textContent === "Show older revisions",
  )
}

describe("the history of a task", () => {
  it("lists the newest page first, each revision linking to the task as it left it", async () => {
    const root = await show(undefined)

    expect(served.pages).toEqual([{ before: undefined, limit: TASK_HISTORY_PAGE }])
    expect(entries(root)).toHaveLength(TASK_HISTORY_PAGE)
    const newest = entries(root)[0]
    expect(newest.getAttribute("href")).toBe("/openplan/task/OPP-1?revision=r24")
    expect(newest.textContent).toContain("Change r24")
    expect(newest.textContent).not.toContain("The details.")
    expect(newest.textContent).toContain("claude_code")
  })

  it("pages on from the oldest revision it holds, and stops at the first one", async () => {
    const root = await show(undefined)

    await act(async () => older(root)?.click())
    await until(() => entries(root).length === 25)

    expect(served.pages.at(-1)).toEqual({ before: "r5", limit: TASK_HISTORY_PAGE })
    expect(entries(root).at(-1)?.textContent).toContain("added")
    expect(older(root)).toBeUndefined()
  })

  it("marks the revision on screen", async () => {
    const root = await show("r23")

    const current = entries(root).filter((entry) => entry.getAttribute("aria-current") === "page")
    expect(current.map((entry) => entry.getAttribute("href"))).toEqual(["/openplan/task/OPP-1?revision=r23"])
  })

  // A write to the task is a new revision, and the daemon names the task it changed.
  it("reads the history again when the task changes", async () => {
    const root = await show(undefined)
    served.revisions = ["r25", ...served.revisions]

    queryInvalidator.refreshTask("openplan", "OPP-1")
    await until(() => entries(root)[0]?.getAttribute("href") === "/openplan/task/OPP-1?revision=r25")
  })
})

// @vitest-environment happy-dom

import { QueryClientProvider } from "@tanstack/react-query"
import { act, createElement, type ReactNode } from "react"
import { createRoot, type Root } from "react-dom/client"
import { MemoryRouter } from "react-router-dom"
import { afterEach, describe, expect, it, vi } from "vitest"

import { DocRefChip, RefReader, TaskRefChip } from "@openplan/task-ui"

import { queryClient, queryInvalidator } from "../src/lib/query-client"
import { refReader } from "../src/lib/ref-reader"

const served = vi.hoisted(() => ({
  tasks: new Map<string, string>(),
  docs: new Set<string>(),
  reads: [] as string[],
}))

vi.mock("../src/lib/api", async () => {
  const { Data, Effect } = await import("effect")
  class TaskNotFound extends Data.TaggedError("TaskNotFound")<{ readonly id: string }> {}
  return {
    TaskNotFound,
    getTask: (project: string, id: string) =>
      Effect.suspend(() => {
        served.reads.push(id)
        const status = served.tasks.get(id)
        return status === undefined
          ? Effect.fail(new TaskNotFound({ id }))
          : Effect.succeed({ project, id, title: id, metadata: { status, created: "2026-01-01T00:00:00Z" } })
      }),
    getDoc: (project: string, name: string) =>
      Effect.suspend(() =>
        served.docs.has(name)
          ? Effect.succeed({ project, name, title: name, body: "" })
          : Effect.fail(new TaskNotFound({ id: name })),
      ),
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
  served.tasks.clear()
  served.docs.clear()
  served.reads = []
  queryClient.clear()
})

const tick = () =>
  act(async () => {
    await new Promise((resume) => setTimeout(resume, 0))
  })

async function settle() {
  do await tick()
  while (queryClient.isFetching() > 0)
}

const chips = (root: HTMLElement) =>
  Array.from(root.querySelectorAll("a")).map((chip) => ({
    text: chip.textContent,
    status: chip.querySelector("[aria-label]")?.getAttribute("aria-label"),
    dangling: chip.classList.contains("border-dashed"),
  }))

async function show(...chips: ReadonlyArray<ReactNode>): Promise<HTMLElement> {
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
        createElement(MemoryRouter, null, createElement(RefReader, { value: refReader }, ...chips)),
      ),
    )
  })
  await settle()
  return container
}

const task = (id: string, key = id) =>
  createElement(TaskRefChip, { key, to: `/openplan/task/${id}`, project: "openplan", id })

describe("the reader of the app", () => {
  it("reads a task once for every chip that shows it", async () => {
    served.tasks.set("OPP-2", "in_progress")
    const root = await show(task("OPP-2", "a"), task("OPP-2", "b"))

    const chip = { text: "OPP-2", status: "In progress", dangling: false }
    expect(chips(root)).toEqual([chip, chip])
    expect(served.reads).toEqual(["OPP-2"])
  })

  it("renders a task the daemon does not hold dashed", async () => {
    expect(chips(await show(task("OPP-9")))).toMatchObject([{ text: "OPP-9", dangling: true }])
  })

  it("follows a change to the task", async () => {
    served.tasks.set("OPP-2", "in_progress")
    const root = await show(task("OPP-2"))
    served.tasks.set("OPP-2", "done")

    queryInvalidator.refreshTask("openplan", "OPP-2")
    await settle()

    expect(chips(root)[0].status).toBe("Done")
  })

  it("renders a doc the daemon does not hold dashed", async () => {
    served.docs.add("storage")
    const doc = (name: string) =>
      createElement(DocRefChip, { key: name, to: `/openplan/doc/${name}`, project: "openplan", name })
    const root = await show(doc("storage"), doc("gone"))

    expect(chips(root).map(({ text, dangling }) => ({ text, dangling }))).toEqual([
      { text: "storage", dangling: false },
      { text: "gone", dangling: true },
    ])
  })
})

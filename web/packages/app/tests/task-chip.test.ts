// @vitest-environment happy-dom

import { QueryClientProvider } from "@tanstack/react-query"
import { act, createElement } from "react"
import { createRoot, type Root } from "react-dom/client"
import { MemoryRouter } from "react-router-dom"
import { afterEach, describe, expect, it, vi } from "vitest"

import { TaskChip } from "../src/components/task-chip"
import { queryClient, queryInvalidator } from "../src/lib/query-client"

const served = vi.hoisted(() => ({
  tasks: new Map<string, { title: string; status: string }>(),
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
        const task = served.tasks.get(id)
        return task === undefined
          ? Effect.fail(new TaskNotFound({ id }))
          : Effect.succeed({
              project,
              id,
              title: task.title,
              metadata: { status: task.status, created: "2026-01-01T00:00:00Z" },
            })
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
  served.tasks.clear()
  served.reads = []
  queryClient.clear()
})

const tick = () =>
  act(async () => {
    await new Promise((resume) => setTimeout(resume, 0))
  })

const chips = (root: HTMLElement) =>
  Array.from(root.querySelectorAll("a")).map((chip) => ({
    text: chip.textContent,
    to: chip.getAttribute("href"),
    dangling: chip.classList.contains("border-dashed"),
  }))

async function show(...ids: ReadonlyArray<string>): Promise<HTMLElement> {
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
        createElement(
          MemoryRouter,
          null,
          ids.map((id, at) => createElement(TaskChip, { key: at, project: "openplan", id, sign: "+" })),
        ),
      ),
    )
  })
  await tick()
  return container
}

describe("a task chip", () => {
  it("reads its own task, once for every chip that shows it", async () => {
    served.tasks.set("OPP-2", { title: "Ship login", status: "in_progress" })
    const root = await show("OPP-2", "OPP-2")

    const chip = { text: "+OPP-2Ship login", to: "/openplan/task/OPP-2", dangling: false }
    expect(chips(root)).toEqual([chip, chip])
    expect(served.reads).toEqual(["OPP-2"])
  })

  it("is dashed for a task that does not exist", async () => {
    expect(chips(await show("OPP-9"))).toEqual([{ text: "+OPP-9", to: "/openplan/task/OPP-9", dangling: true }])
  })

  it("follows a change to its task", async () => {
    served.tasks.set("OPP-2", { title: "Ship login", status: "in_progress" })
    const root = await show("OPP-2")
    served.tasks.set("OPP-2", { title: "Ship the login page", status: "done" })

    queryInvalidator.refreshTask("openplan", "OPP-2")
    await tick()

    expect(chips(root)[0].text).toBe("+OPP-2Ship the login page")
  })
})

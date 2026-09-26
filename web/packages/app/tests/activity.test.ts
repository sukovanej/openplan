// @vitest-environment happy-dom

import { QueryClientProvider } from "@tanstack/react-query"
import { act, createElement } from "react"
import { createRoot, type Root } from "react-dom/client"
import { MemoryRouter } from "react-router-dom"
import { afterEach, describe, expect, it, vi } from "vitest"

import type { HistoryEntry } from "@openplan/api-client"

import { queryClient } from "../src/lib/query-client"
import { Activity } from "../src/routes/activity"

const revision = (id: string, agent?: string): HistoryEntry["revision"] => ({
  id,
  parents: ["0000000000000000000000000000000000000000"],
  author: "Milan",
  agent,
  at: "2026-01-02T00:00:00Z",
  message: `Set it to done in ${id}`,
})

const served = vi.hoisted(() => ({ entries: [] as Array<unknown> }))

vi.mock("../src/lib/api", async () => {
  const { Effect } = await import("effect")
  return {
    getProjectHistory: () => Effect.sync(() => served.entries),
    getBoard: () =>
      Effect.sync(() => ({
        groups: [
          {
            status: "done",
            rows: [
              {
                task: {
                  project: "openplan",
                  id: "OPP-1",
                  title: "Ship it",
                  metadata: { status: "done", created: "2026-01-01T00:00:00Z" },
                },
                depth: 0,
                has_children: false,
              },
            ],
          },
        ],
      })),
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
  served.entries = []
  queryClient.clear()
})

const revisions = (root: HTMLElement) =>
  Array.from(root.querySelectorAll<HTMLLIElement>("ol[aria-label='Revisions'] > li"))

const tick = () =>
  act(async () => {
    await new Promise((resume) => setTimeout(resume, 0))
  })

async function show(): Promise<HTMLElement> {
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
        createElement(MemoryRouter, null, createElement(Activity, { project: "openplan" })),
      ),
    )
  })
  for (let attempt = 0; attempt < 20 && revisions(container).length === 0; attempt++) await tick()
  return container
}

const lines = (revision: Element) =>
  Array.from(revision.querySelectorAll("ul[aria-label='Changes'] > li")).map((line) => line.textContent)

describe("the activity", () => {
  it("gives each revision when and who once, and a line for each of its changes", async () => {
    served.entries = [
      {
        revision: revision("c0ffee1234567890", "claude-code"),
        changes: [],
        summary: [],
        tasks: [
          {
            task: "OPP-1",
            kind: "modified",
            title: "Ship it",
            fields: [{ field: "status", from: "todo", to: "done" }],
          },
          { task: "OPP-2", kind: "removed", title: "Doomed" },
        ],
        tags: [{ tag: "server", kind: "modified", renamed_from: "backend" }],
      },
      {
        revision: revision("beef000011112222"),
        changes: [{ path: "config.toml", kind: "modified" }],
        summary: [],
        tasks: [],
        tags: [],
      },
    ]
    const root = await show()

    const [first, second] = revisions(root)
    expect(first.textContent?.match(/Milan/g)).toHaveLength(1)
    expect(first.textContent).toContain("claude-code")
    expect(first.querySelector("time")?.compareDocumentPosition(first.querySelector("ul")!)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    )
    expect(lines(first)).toEqual(["TodoDoneOPP-1Ship it", "DeletedOPP-2Doomed", "Renamed from backendserver"])
    expect(lines(second)).toEqual(["Editedconfig.toml"])
    expect(root.querySelector("table")).toBeNull()
  })

  it("names no revision by its hash, and shows no message", async () => {
    served.entries = [
      {
        revision: revision("c0ffee1234567890"),
        changes: [],
        summary: [],
        tasks: [{ task: "OPP-1", kind: "added", title: "Ship it" }],
        tags: [],
      },
    ]
    const root = await show()

    expect(root.textContent).toContain("Created")
    expect(root.textContent).not.toContain("c0ffee1")
    expect(root.textContent).not.toContain("Set it to done")
  })

  it("links a task that the board holds to the task as it is now", async () => {
    served.entries = [
      {
        revision: revision("c0ffee1234567890"),
        changes: [],
        summary: [],
        tasks: [{ task: "OPP-1", kind: "added", title: "Ship it" }],
        tags: [],
      },
    ]
    const root = await show()
    await tick()

    expect(revisions(root)[0].querySelector("a")?.getAttribute("href")).toBe("/openplan/task/OPP-1")
  })
})

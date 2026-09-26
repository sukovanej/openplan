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

const served = vi.hoisted(() => ({ entries: [] as Array<unknown>, diffs: vi.fn() }))

vi.mock("../src/lib/api", async () => {
  const { Effect } = await import("effect")
  return {
    getProjectHistory: () => Effect.sync(() => served.entries),
    getRevisionDiff: (...target: Array<unknown>) =>
      Effect.sync(() => {
        served.diffs(...target)
        return {
          kind: "text",
          diff: "--- a/config.toml\n+++ b/config.toml\n@@ -1,1 +1,1 @@\n-old = 1\n+new = 1\n",
          truncated: false,
        }
      }),
    listAllDocs: () =>
      Effect.sync(() => [
        {
          project: "openplan",
          name: "the-design",
          title: "The Design",
          metadata: { created: "2026-01-01T00:00:00Z" },
          updated: "2026-01-02T00:00:00Z",
          conflicts: 0,
        },
      ]),
    listTags: () => Effect.sync(() => [{ name: "server", display: "Server", color: "blue" }]),
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
  served.diffs.mockClear()
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
            fields: [
              { field: "status", from: "todo", to: "done" },
              { field: "tags", from: [], to: ["server"] },
            ],
          },
          { task: "OPP-2", kind: "removed", title: "Doomed" },
        ],
        tags: [{ tag: "server", kind: "modified", renamed_from: "backend" }],
        docs: [],
      },
      {
        revision: revision("beef000011112222"),
        changes: [{ path: "config.toml", kind: "modified" }],
        summary: [],
        tasks: [],
        tags: [],
        docs: [],
      },
    ]
    const root = await show()
    await tick()

    const [first, second] = revisions(root)
    expect(first.textContent?.match(/Milan/g)).toHaveLength(1)
    expect(first.textContent).toContain("claude-code")
    expect(first.querySelector("time")?.compareDocumentPosition(first.querySelector("ul")!)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    )
    expect(lines(first)).toEqual(["TodoDone+ServerOPP-1Ship it", "DeletedOPP-2Doomed", "RenamedbackendServer"])
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
        docs: [],
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
        docs: [],
      },
    ]
    const root = await show()
    await tick()

    expect(revisions(root)[0].querySelector("a")?.getAttribute("href")).toBe("/openplan/task/OPP-1")
  })
})

describe("the doc changes in the activity", () => {
  const docs = (): HistoryEntry => ({
    revision: revision("d0c5000011112222"),
    changes: [
      { path: "docs/architecture.md", kind: "removed", doc: "architecture" },
      { path: "docs/the-design.md", kind: "added", doc: "the-design" },
      { path: "docs/storage.md", kind: "removed", doc: "storage" },
    ],
    summary: [],
    tasks: [],
    tags: [],
    docs: [
      { doc: "the-design", kind: "modified", renamed_from: "architecture" },
      { doc: "storage", kind: "removed" },
    ],
  })

  it("gives each doc one line with its title, what happened, and a link to it or to the revision before it went", async () => {
    served.entries = [docs()]
    const root = await show()
    await tick()

    const [revision] = revisions(root)
    expect(lines(revision)).toEqual(["Renamed from architectureThe Design", "Deletedstorage"])
    const [renamed, deleted] = Array.from(revision.querySelectorAll("ul[aria-label='Changes'] > li"))
    expect(renamed.querySelector("a")?.getAttribute("href")).toBe("/openplan/doc/the-design")
    expect(deleted.querySelector("a")?.getAttribute("href")).toBe(
      "/openplan/doc/storage?revision=0000000000000000000000000000000000000000",
    )
    expect(root.textContent).not.toContain("docs/")
  })

  it("diffs a renamed doc from its old file to its new one", async () => {
    served.entries = [docs()]
    const root = await show()
    const line = revisions(root)[0].querySelector("[aria-haspopup=dialog]")!

    await act(async () => void line.dispatchEvent(new Event("pointerover", { bubbles: true })))
    for (let attempt = 0; attempt < 20 && served.diffs.mock.calls.length === 0; attempt++) await tick()

    expect(served.diffs).toHaveBeenCalledExactlyOnceWith("openplan", "d0c5000011112222", {
      path: "docs/the-design.md",
      from: "docs/architecture.md",
    })
  })
})

describe("the diff of a change", () => {
  const config = (count: number): HistoryEntry => ({
    revision: revision("beef000011112222"),
    changes: Array.from({ length: count }, (_, at) => ({ path: `assets/${at}.txt`, kind: "modified" as const })),
    summary: [],
    tasks: [],
    tags: [],
    docs: [],
  })

  it("reads the diff of a line the moment the pointer enters it", async () => {
    served.entries = [config(1)]
    const root = await show()
    const line = revisions(root)[0].querySelector("[aria-haspopup=dialog]")!

    await act(async () => void line.dispatchEvent(new Event("pointerover", { bubbles: true })))
    for (let attempt = 0; attempt < 20 && !root.querySelector("[role=dialog] mark, [role=dialog] .grid"); attempt++) {
      await tick()
    }

    expect(served.diffs).toHaveBeenCalledExactlyOnceWith("openplan", "beef000011112222", { path: "assets/0.txt" })
    const card = root.querySelector("[role=dialog]")!
    expect(card.getAttribute("aria-label")).toBe("Diff of assets/0.txt")
    expect(card.textContent).toContain("-old = 1")
    expect(card.textContent).toContain("+new = 1")
  })

  it("gives the line that counts the rest no diff", async () => {
    served.entries = [config(13)]
    const root = await show()
    const changes = Array.from(revisions(root)[0].querySelectorAll("ul[aria-label='Changes'] > li"))

    expect(changes.at(-1)?.textContent).toBe("and 1 more")
    expect(changes.at(-1)?.querySelector("[aria-haspopup]")).toBeNull()
    expect(changes[0].querySelector("[aria-haspopup]")).not.toBeNull()
  })
})

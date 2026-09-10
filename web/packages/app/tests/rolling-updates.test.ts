// @vitest-environment happy-dom

import { expect, it } from "@effect/vitest"
import { QueryClientProvider } from "@tanstack/react-query"
import { act, createElement } from "react"
import { createRoot, type Root } from "react-dom/client"
import { MemoryRouter } from "react-router-dom"
import { afterEach, describe, vi } from "vitest"

import type { MatrixCell } from "@openplan/api-client"
import { ROLLING_UPDATES_LABEL } from "@openplan/task-ui"

import { RollingUpdates } from "../src/components/rolling-updates"
import { connectionStore } from "../src/lib/connection"
import { queryClient, queryInvalidator } from "../src/lib/query-client"
import { conflicted, pendingCount, type ProjectUpdates, syncState } from "../src/lib/rolling-updates"

const cell = (id: string): MatrixCell => ({
  branch: "openplan/rolling-updates",
  task: {
    id,
    title: id,
    metadata: { status: "todo", created: "2026-01-01T00:00:00Z", parent: null, rank: null, dependencies: [], tags: [] },
  },
  blob_oid: id,
  dirty: false,
  kind: "modified",
})

const updates = (project: string, pending: number, conflict = false): ProjectUpdates => ({
  project,
  pending: Array.from({ length: pending }, (_, at) => cell(`${project}-${at}`)),
  conflict: conflict ? { files: [".plan/tasks/00001-t.md"], worktree: "/tmp/updates" } : undefined,
})

it("counts what every project has waiting", () => {
  expect(pendingCount([updates("a", 2), updates("b", 3)])).toBe(5)
})

it("names the projects a conflict holds", () => {
  expect(conflicted([updates("a", 2), updates("b", 1, true)]).map((one) => one.project)).toEqual(["b"])
})

it("is idle when no project has anything waiting", () => {
  expect(syncState([updates("a", 0)], true, false)).toBe("idle")
})

it("is pending when a project has something waiting", () => {
  expect(syncState([updates("a", 0), updates("b", 1)], true, false)).toBe("pending")
})

// A conflict is what stops the count from ever being published, so it outranks the count.
it("is blocked when a conflict holds a project, whatever else is waiting", () => {
  expect(syncState([updates("a", 4), updates("b", 1, true)], true, false)).toBe("blocked")
})

it("is syncing while a publish runs", () => {
  expect(syncState([updates("a", 1, true)], true, true)).toBe("syncing")
})

// Nothing can publish while the daemon is down, and what the control holds is already stale.
it("is offline whatever the daemon last reported", () => {
  expect(syncState([updates("a", 3, true)], false, true)).toBe("offline")
})

const PROJECT = "openplan"

const served = vi.hoisted(() => ({
  diff: ["@@ -1 +1 @@", "-the old body", "+the new body"].join("\n"),
}))

vi.mock("../src/lib/api", async () => {
  const { Effect } = await import("effect")
  return {
    listProjects: Effect.succeed([
      {
        name: "openplan",
        root: "/repo",
        git_common_dir: "/repo/.git",
        abbreviation: "OPP",
        status: { state: "ok" },
        rolling_updates_branch: "openplan/rolling-updates",
      },
    ]),
    getRollingUpdates: () =>
      Effect.sync(() => ({
        pending: [cell("OPP-1"), cell("OPP-2")],
        conflict: null,
      })),
    getRollingUpdateDiff: () => Effect.sync(() => ({ diff: served.diff })),
    publishRollingUpdates: () => Effect.succeed({ remote: "origin", branch: "b", commit: "c", pull_request: null }),
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
  queryClient.clear()
})

// The effect runtime resolves a read on a timer rather than on the microtask queue, and each read
// starts the one below it, so the mount is stepped until the control it draws is there.
const tick = () =>
  act(async () => {
    await new Promise((resume) => setTimeout(resume, 0))
  })

async function settle(container: HTMLElement, label: string): Promise<HTMLButtonElement> {
  for (let attempt = 0; attempt < 20; attempt++) {
    const found = container.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`)
    if (found !== null) return found
    await tick()
  }
  throw new Error(`no ${label} button appeared`)
}

async function openTheReview(): Promise<HTMLElement> {
  ;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true
  connectionStore.set("live")
  const container = document.createElement("div")
  document.body.append(container)
  const root = createRoot(container)
  mounted = { root, container }
  await act(async () => {
    root.render(
      createElement(
        QueryClientProvider,
        { client: queryClient },
        createElement(MemoryRouter, null, createElement(RollingUpdates)),
      ),
    )
  })
  await click(await settle(container, ROLLING_UPDATES_LABEL))
  return container
}

async function click(target: HTMLElement): Promise<void> {
  await act(async () => target.click())
  await tick()
}

const expanded = (root: HTMLElement) =>
  Array.from(root.querySelectorAll<HTMLButtonElement>('button[aria-label^="Diff of"]')).map(
    (chevron) => chevron.getAttribute("aria-expanded") === "true",
  )

describe("the review popover", () => {
  it("expands the row whose chevron a person clicks", async () => {
    const root = await openTheReview()

    await click(await settle(root, "Diff of OPP-1"))

    expect(expanded(root)).toEqual([true, false])
    expect(root.textContent).toContain("the new body")
  })

  // Two diffs at once would push the rest of the list, and the button that publishes it, out of view.
  it("closes the open row when another row opens", async () => {
    const root = await openTheReview()

    await click(await settle(root, "Diff of OPP-1"))
    await click(await settle(root, "Diff of OPP-2"))

    expect(expanded(root)).toEqual([false, true])
  })

  it("closes the open row when its own chevron is clicked again", async () => {
    const root = await openTheReview()

    await click(await settle(root, "Diff of OPP-1"))
    await click(await settle(root, "Diff of OPP-1"))

    expect(expanded(root)).toEqual([false, false])
  })

  // The daemon commits, rebases, and publishes on its own timers, and each one can rewrite the diff
  // a person is reading.
  it("re-reads the open diff when the rolling updates change", async () => {
    const root = await openTheReview()
    await click(await settle(root, "Diff of OPP-1"))
    served.diff = ["@@ -1 +1 @@", "-the old body", "+a later body"].join("\n")

    queryInvalidator.refreshRollingUpdates(PROJECT)
    await tick()
    await tick()

    expect(root.textContent).toContain("a later body")
  })
})

// @vitest-environment happy-dom

import { QueryClientProvider } from "@tanstack/react-query"
import { act, createElement } from "react"
import { createRoot, type Root } from "react-dom/client"
import { MemoryRouter } from "react-router-dom"
import { afterEach, describe, expect, it, vi } from "vitest"

import type { ProjectView, SyncView, TaskListItem } from "@openplan/api-client"

import { SYNC_LABEL, SyncStatus } from "../src/components/sync-status"
import { connectionStore } from "../src/lib/connection"
import { queryClient, queryInvalidator } from "../src/lib/query-client"
import {
  failed,
  type ProjectSync,
  syncable,
  syncResultText,
  syncState,
  tasksInConflict,
  waitingCount,
} from "../src/lib/sync"

const view = (over: Partial<SyncView> = {}): SyncView => ({ remote: "origin", ahead: 0, behind: 0, ...over })
const sync = (project: string, over: Partial<SyncView> = {}): ProjectSync => ({ project, view: view(over) })

describe("the sync state", () => {
  it("counts what every project has to send and to receive", () => {
    expect(waitingCount([sync("a", { ahead: 2 }), sync("b", { ahead: 1, behind: 3 })])).toBe(6)
  })

  it("names the projects whose last sync failed", () => {
    expect(failed([sync("a"), sync("b", { error: "no route" })]).map((one) => one.project)).toEqual(["b"])
  })

  it("is idle when nothing waits", () => {
    expect(syncState([sync("a")], true, false)).toBe("idle")
  })

  it("is waiting while a revision waits to be sent or received", () => {
    expect(syncState([sync("a"), sync("b", { behind: 1 })], true, false)).toBe("waiting")
  })

  // A failure is what keeps the count from going down, so it outranks the count.
  it("has failed when one project's last sync failed, whatever else waits", () => {
    expect(syncState([sync("a", { ahead: 4 }), sync("b", { error: "no route" })], true, false)).toBe("failed")
  })

  it("is syncing while a sync runs", () => {
    expect(syncState([sync("a", { error: "no route" })], true, true)).toBe("syncing")
  })

  // Nothing can sync while the daemon is down, and what the control holds is already stale.
  it("is offline whatever the daemon last reported", () => {
    expect(syncState([sync("a", { ahead: 3, error: "no route" })], false, true)).toBe("offline")
  })
})

const task = (id: string, conflicts: number): TaskListItem => ({
  project: "openplan",
  id,
  title: `Task ${id}`,
  metadata: { status: "todo", created: "2026-01-01T00:00:00Z", parent: null, rank: null, dependencies: [], tags: [] },
  updated: "2026-01-02T00:00:00Z",
  comment_count: 0,
  conflicts,
  problems: [],
})

describe("the tasks a sync left in conflict", () => {
  it("are the tasks that hold at least one conflict, in list order", () => {
    const ids = tasksInConflict([task("OPP-1", 0), task("OPP-2", 2), task("OPP-3", 1)]).map((one) => one.id)
    expect(ids).toEqual(["OPP-2", "OPP-3"])
  })
})

describe("which projects sync", () => {
  const project = (over: Partial<ProjectView>): ProjectView => ({
    name: "openplan",
    root: "/repo",
    backend: "git",
    abbreviation: "OPP",
    status: { state: "ok" },
    sync: view(),
    ...over,
  })

  it("takes a git project that has a remote", () => {
    expect(syncable(project({}))).toBe(true)
  })

  it("leaves out a local project, a repository without a remote, and a project that is not served", () => {
    expect(syncable(project({ backend: "local", sync: undefined }))).toBe(false)
    expect(syncable(project({ sync: undefined }))).toBe(false)
    expect(syncable(project({ status: { state: "error", reason: "gone" } }))).toBe(false)
  })
})

describe("what a sync did", () => {
  it("says that nothing moved", () => {
    expect(syncResultText({ received: 0, sent: 0, merged: false, status: view() })).toBe("Nothing to receive or send.")
  })

  it("says what came in, what went out, and whether the two merged", () => {
    expect(syncResultText({ received: 2, sent: 1, merged: true, status: view() })).toBe(
      "Received 2 revisions. Merged them with the local changes. Sent 1 revision.",
    )
  })
})

const served = vi.hoisted(() => ({
  view: { remote: "origin", ahead: 1, behind: 0 } as {
    remote: string
    ahead: number
    behind: number
    last_success?: string
    last_attempt?: string
    error?: string
  },
  reads: 0,
  synced: [] as Array<string>,
  tasks: [] as Array<TaskListItem>,
}))

vi.mock("../src/lib/api", async () => {
  const { Effect } = await import("effect")
  return {
    listProjects: Effect.sync(() => [
      {
        name: "openplan",
        root: "/repo",
        git_common_dir: "/repo/.git",
        backend: "git",
        abbreviation: "OPP",
        status: { state: "ok" },
        sync: { remote: "origin", ahead: 1, behind: 0 },
      },
      { name: "notes", root: "/notes", backend: "local", abbreviation: "NTS", status: { state: "ok" } },
    ]),
    listTasks: () => Effect.sync(() => served.tasks),
    getSync: () =>
      Effect.sync(() => {
        served.reads += 1
        return served.view
      }),
    runSync: (project: string) =>
      Effect.sync(() => {
        served.synced.push(project)
        served.view = { remote: "origin", ahead: 0, behind: 0, last_success: "2026-01-02T00:00:00Z" }
        return { received: 2, sent: 1, merged: false, status: served.view }
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
  served.view = { remote: "origin", ahead: 1, behind: 0 }
  served.reads = 0
  served.synced = []
  served.tasks = []
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

async function click(target: HTMLElement): Promise<void> {
  await act(async () => target.click())
  await tick()
}

function labelled(root: HTMLElement, text: string): HTMLButtonElement {
  const found = Array.from(root.querySelectorAll<HTMLButtonElement>("button")).find((one) => one.textContent === text)
  if (found === undefined) throw new Error(`no ${text} button appeared`)
  return found
}

async function openTheControl(): Promise<HTMLElement> {
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
        createElement(MemoryRouter, null, createElement(SyncStatus)),
      ),
    )
  })
  await click(await settle(container, SYNC_LABEL))
  return container
}

describe("the sync control", () => {
  // The project list carries the state, so the control reads nothing more until a sync says so.
  it("shows the git project from the project list, and leaves the local one out", async () => {
    const root = await openTheControl()

    expect(root.textContent).toContain("origin")
    expect(root.textContent).toContain("Never")
    expect(root.querySelector('section[aria-label="notes"]')).toBeNull()
    expect(served.reads).toBe(0)
  })

  it("re-reads the state when the daemon says a sync ran, and shows why it failed", async () => {
    const root = await openTheControl()
    served.view = { remote: "origin", ahead: 1, behind: 0, error: "could not reach origin" }

    queryInvalidator.refreshSync("openplan")
    await tick()
    await tick()

    expect(served.reads).toBe(1)
    expect(root.querySelector('[role="alert"]')?.textContent).toContain("could not reach origin")
  })

  it("links each task of the project that holds a conflict", async () => {
    served.tasks = [task("OPP-1", 0), task("OPP-2", 2)]
    const root = await openTheControl()
    await tick()

    const note = root.querySelector('[role="note"]')
    expect(note?.textContent).toContain("1 task holds conflicts from a sync.")
    const links = Array.from(note?.querySelectorAll("a") ?? [])
    expect(links.map((link) => link.getAttribute("href"))).toEqual(["/openplan/task/OPP-2"])
    expect(links[0].textContent).toContain("2 conflicts")
  })

  it("says nothing about conflicts when no task holds one", async () => {
    served.tasks = [task("OPP-1", 0)]
    const root = await openTheControl()
    await tick()

    expect(root.querySelector('[role="note"]')).toBeNull()
  })

  it("syncs now, and says what the sync moved", async () => {
    const root = await openTheControl()

    await click(labelled(root, "Sync now"))
    await tick()

    expect(served.synced).toEqual(["openplan"])
    expect(root.textContent).toContain("Received 2 revisions. Sent 1 revision.")
  })
})

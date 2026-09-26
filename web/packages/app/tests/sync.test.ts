// @vitest-environment happy-dom

import { QueryClientProvider } from "@tanstack/react-query"
import { act, createElement } from "react"
import { createRoot, type Root } from "react-dom/client"
import { MemoryRouter } from "react-router-dom"
import { afterEach, describe, expect, it, vi } from "vitest"

import type { ProjectView, SyncView } from "@openplan/api-client"

import { SYNC_LABEL, SyncStatus } from "../src/components/sync-status"
import { connectionStore } from "../src/lib/connection"
import { queryClient, queryInvalidator } from "../src/lib/query-client"
import { failed, type ProjectSync, syncable, syncState, waitingCount } from "../src/lib/sync"

const view = (over: Partial<SyncView> = {}): SyncView => ({
  remote: "origin",
  ahead: 0,
  behind: 0,
  syncing: false,
  ...over,
})
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

  it("is syncing while the daemon runs a sync of its own", () => {
    expect(syncState([sync("a", { ahead: 2 }), sync("b", { syncing: true })], true, false)).toBe("syncing")
  })

  // Nothing can sync while the daemon is down, and what the control holds is already stale.
  it("is offline whatever the daemon last reported", () => {
    expect(syncState([sync("a", { ahead: 3, error: "no route" })], false, true)).toBe("offline")
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

const served = vi.hoisted(() => ({
  view: { remote: "origin", ahead: 1, behind: 0, syncing: false } as {
    remote: string
    ahead: number
    behind: number
    syncing: boolean
    last_success?: string
    last_attempt?: string
    error?: string
  },
  reads: 0,
  synced: [] as Array<string>,
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
        sync: { remote: "origin", ahead: 1, behind: 0, syncing: false },
      },
      { name: "notes", root: "/notes", backend: "local", abbreviation: "NTS", status: { state: "ok" } },
    ]),
    getSync: () =>
      Effect.sync(() => {
        served.reads += 1
        return served.view
      }),
    runSync: (project: string) =>
      Effect.sync(() => {
        served.synced.push(project)
        served.view = { remote: "origin", ahead: 0, behind: 0, syncing: false, last_success: "2026-01-02T00:00:00Z" }
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
  served.view = { remote: "origin", ahead: 1, behind: 0, syncing: false }
  served.reads = 0
  served.synced = []
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

    expect(root.querySelector('li[aria-label="openplan"]')?.textContent).toContain("Never synced")
    expect(root.querySelector('li[aria-label="notes"]')).toBeNull()
    expect(served.reads).toBe(0)
  })

  it("re-reads the state when the daemon says a sync ran, and marks a failed sync", async () => {
    const root = await openTheControl()
    served.view = { remote: "origin", ahead: 1, behind: 0, syncing: false, error: "could not reach origin" }

    queryInvalidator.refreshSync("openplan")
    await tick()
    await tick()

    expect(served.reads).toBe(1)
    expect(root.querySelector('[aria-label="The last sync failed"]')).not.toBeNull()
  })

  it("spins the sync icon while the daemon runs a sync", async () => {
    const root = await openTheControl()
    served.view = { remote: "origin", ahead: 1, behind: 0, syncing: true }

    queryInvalidator.refreshSync("openplan")
    await tick()
    await tick()

    const button = labelled(root, "Sync now")
    expect(button.disabled).toBe(true)
    expect(button.querySelector("svg")?.getAttribute("class")).toContain("animate-spin")
  })

  it("syncs now, and shows when the sync succeeded", async () => {
    const root = await openTheControl()

    await click(labelled(root, "Sync now"))
    await tick()

    expect(served.synced).toEqual(["openplan"])
    const row = root.querySelector('li[aria-label="openplan"]')
    expect(row?.textContent).toContain("Synced")
    expect(row?.textContent).not.toContain("Never")
  })
})

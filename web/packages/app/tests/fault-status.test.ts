// @vitest-environment happy-dom

import { QueryClientProvider } from "@tanstack/react-query"
import { act, createElement } from "react"
import { createRoot, type Root } from "react-dom/client"
import { MemoryRouter } from "react-router-dom"
import { afterEach, describe, expect, it, vi } from "vitest"

import type { Fault } from "@openplan/api-client"

import { FAULTS_LABEL, FaultStatus } from "../src/components/fault-status"
import { queryClient, queryInvalidator } from "../src/lib/query-client"

const served = vi.hoisted(() => ({ faults: [] as Array<Fault> }))

vi.mock("../src/lib/api", async () => {
  const { Effect } = await import("effect")
  return { listFaults: Effect.sync(() => served.faults) }
})

const unsigned: Fault = {
  project: "openplan",
  kind: "no_identity",
  message: 'git `user.name` is not set; set it with `git config --global user.name "Your Name"`',
}
const offline: Fault = { project: "notes", kind: "sync_failed", message: "the last sync failed: no route" }

let mounted: { root: Root; container: HTMLElement } | undefined

afterEach(async () => {
  const held = mounted
  mounted = undefined
  if (held !== undefined) {
    await act(async () => held.root.unmount())
    held.container.remove()
  }
  served.faults = []
  queryClient.clear()
})

// The effect runtime resolves a read on a timer rather than on the microtask queue.
const tick = () =>
  act(async () => {
    await new Promise((resume) => setTimeout(resume, 0))
  })

async function mount(): Promise<HTMLElement> {
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
        createElement(MemoryRouter, null, createElement(FaultStatus)),
      ),
    )
  })
  await tick()
  await tick()
  return container
}

const control = (root: HTMLElement) => root.querySelector<HTMLButtonElement>(`button[aria-label="${FAULTS_LABEL}"]`)

describe("the fault control", () => {
  it("is absent while no fault lasts", async () => {
    const root = await mount()
    expect(control(root)).toBeNull()
  })

  it("counts the faults and lists each with its project and its message", async () => {
    served.faults = [unsigned, offline]
    const root = await mount()

    const button = control(root)
    expect(button?.textContent).toContain("2")
    await act(async () => button?.click())

    expect(root.querySelector('li[aria-label="openplan"]')?.textContent).toContain("git config --global user.name")
    expect(root.querySelector('li[aria-label="notes"]')?.textContent).toContain("the last sync failed")
  })

  it("appears when the daemon says a fault started and goes when it ends", async () => {
    const root = await mount()
    expect(control(root)).toBeNull()

    served.faults = [unsigned]
    queryInvalidator.refreshFaults()
    await tick()
    await tick()
    expect(control(root)).not.toBeNull()

    served.faults = []
    queryInvalidator.refreshFaults()
    await tick()
    await tick()
    expect(control(root)).toBeNull()
  })
})

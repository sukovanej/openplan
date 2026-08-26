import { Effect } from "effect"
import { describe, expect, it } from "vitest"

import { TaskRejected } from "../src/lib/api"
import {
  boardQuery,
  mutationError,
  Query,
  runMutation,
  storeInvalidator,
  tagsQuery,
  taskQuery,
  tasksQuery,
} from "../src/lib/store"

// A load is dispatched through the runtime, so it settles a turn after the refresh that asked for it.
const settle = () => new Promise((resolve) => setTimeout(resolve, 0))

// A query that counts its own loads, standing in for a mounted read of `project`. Subscribing is
// what puts it among the mounted queries, which is the set an invalidation walks.
function counting(project: string | undefined) {
  let loads = 0
  const query = new Query(
    project,
    Effect.sync(() => ++loads),
  )
  const unsubscribe = query.subscribe(() => {})
  return { loads: () => loads, unsubscribe }
}

// Two projects hold different task sets, so each read is its own query. Without this a change in one
// project would land on the other's data.
describe("per-project queries", () => {
  it("gives each project its own board and task-list query, and keeps each stable", () => {
    expect(boardQuery("alpha")).toBe(boardQuery("alpha"))
    expect(boardQuery("alpha")).not.toBe(boardQuery("beta"))
    expect(tasksQuery("alpha")).not.toBe(tasksQuery("beta"))
  })

  // Each project serves its own `.plan/tags/`, so one project's registry never names another's tags.
  it("gives each project its own tag registry", () => {
    expect(tagsQuery("alpha")).toBe(tagsQuery("alpha"))
    expect(tagsQuery("alpha")).not.toBe(tagsQuery("beta"))
  })

  // Two stores can commit the same abbreviation, so the same key names a different task in each.
  it("keys a task query on its project as well as its id and branch", () => {
    expect(taskQuery("alpha", "APP-1")).toBe(taskQuery("alpha", "APP-1"))
    expect(taskQuery("alpha", "APP-1")).not.toBe(taskQuery("beta", "APP-1"))
    expect(taskQuery("alpha", "APP-1")).not.toBe(taskQuery("alpha", "APP-1", "feature"))
    expect(taskQuery("alpha", "APP-1", "feature")).not.toBe(taskQuery("beta", "APP-1", "feature"))
  })

  // A view that mounts, unmounts, and mounts again — which is every view under StrictMode — comes
  // back holding the query it already had, and a write reaches a task's reads through this map.
  // Dropping the query when its last listener left left that remounted view unreachable, so the
  // second write of a session showed nothing until a reload.
  it("keeps a task query in the map after its last listener leaves", () => {
    const query = taskQuery("gamma", "GAM-1")
    query.subscribe(() => {})()
    expect(taskQuery("gamma", "GAM-1")).toBe(query)
  })
})

describe("invalidation", () => {
  it("refreshes one project's reads and the merged ones, and leaves every other project alone", async () => {
    const alpha = counting("alpha")
    const beta = counting("beta")
    const merged = counting(undefined)
    await settle()
    const before = { alpha: alpha.loads(), beta: beta.loads(), merged: merged.loads() }

    storeInvalidator.refreshVisible("alpha")
    await settle()
    expect(alpha.loads()).toBe(before.alpha + 1)
    expect(merged.loads()).toBe(before.merged + 1)
    expect(beta.loads()).toBe(before.beta)

    alpha.unsubscribe()
    beta.unsubscribe()
    merged.unsubscribe()
  })

  // A projects_changed or a resync names no project, so nothing on screen can be trusted.
  it("refreshes every project when the change names none", async () => {
    const alpha = counting("alpha")
    const beta = counting("beta")
    await settle()
    const before = { alpha: alpha.loads(), beta: beta.loads() }

    storeInvalidator.refreshVisible()
    await settle()
    expect(alpha.loads()).toBe(before.alpha + 1)
    expect(beta.loads()).toBe(before.beta + 1)

    alpha.unsubscribe()
    beta.unsubscribe()
  })

  // An unmounted query is not a read on screen, so nothing is spent refreshing it.
  it("leaves an unmounted query alone", async () => {
    const gone = counting("alpha")
    await settle()
    gone.unsubscribe()
    const before = gone.loads()

    storeInvalidator.refreshVisible("alpha")
    await settle()
    expect(gone.loads()).toBe(before)
  })
})

describe("mutationError", () => {
  it("holds the reason a write was refused until it is dismissed", async () => {
    const refusal = new TaskRejected({ status: 400, message: "cannot reparent epic under child" })
    await runMutation("open-plan", Effect.fail(refusal))
    expect(mutationError.getSnapshot()).toBe(refusal)

    mutationError.clear()
    expect(mutationError.getSnapshot()).toBeUndefined()
  })

  it("clears once a later write succeeds", async () => {
    await runMutation("open-plan", Effect.fail(new TaskRejected({ status: 400, message: "nope" })))
    expect(mutationError.getSnapshot()).toBeDefined()

    await runMutation("open-plan", Effect.succeed("ok"))
    expect(mutationError.getSnapshot()).toBeUndefined()
  })

  it("resolves rather than rejecting, so a caller never needs its own catch", async () => {
    const boom = new Error("boom")
    await expect(runMutation("open-plan", Effect.fail(boom))).resolves.toBe(boom)
    mutationError.clear()
  })

  // A refusal is the whole answer for most writes, but one that can be followed by a different
  // attempt — a forced tag delete, which only a conflict earns — has to read which refusal it was.
  it("hands the refusal back, and nothing at all when the write landed", async () => {
    const refusal = new TaskRejected({ status: 409, message: "still referenced" })
    await expect(runMutation("open-plan", Effect.fail(refusal))).resolves.toBe(refusal)
    await expect(runMutation("open-plan", Effect.succeed("ok"))).resolves.toBeUndefined()
  })

  it("notifies subscribers on change and stops after unsubscribe", async () => {
    let notifications = 0
    const unsubscribe = mutationError.subscribe(() => {
      notifications++
    })

    await runMutation("open-plan", Effect.fail(new Error("first")))
    expect(notifications).toBe(1)

    mutationError.clear()
    expect(notifications).toBe(2)
    // Already clear — no change, so no notification.
    mutationError.clear()
    expect(notifications).toBe(2)

    unsubscribe()
    await runMutation("open-plan", Effect.fail(new Error("second")))
    expect(notifications).toBe(2)
    mutationError.clear()
  })
})

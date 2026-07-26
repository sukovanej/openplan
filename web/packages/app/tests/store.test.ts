import { Effect } from "effect"
import { describe, expect, it } from "vitest"

import { TaskRejected } from "../src/lib/api"
import { mutationError, runMutation } from "../src/lib/store"

describe("mutationError", () => {
  it("holds the reason a write was refused until it is dismissed", async () => {
    const refusal = new TaskRejected({ status: 400, message: "cannot reparent epic under child" })
    await runMutation(Effect.fail(refusal))
    expect(mutationError.getSnapshot()).toBe(refusal)

    mutationError.clear()
    expect(mutationError.getSnapshot()).toBeUndefined()
  })

  it("clears once a later write succeeds", async () => {
    await runMutation(Effect.fail(new TaskRejected({ status: 400, message: "nope" })))
    expect(mutationError.getSnapshot()).toBeDefined()

    await runMutation(Effect.succeed("ok"))
    expect(mutationError.getSnapshot()).toBeUndefined()
  })

  it("resolves rather than rejecting, so a caller never needs its own catch", async () => {
    await expect(runMutation(Effect.fail(new Error("boom")))).resolves.toBeUndefined()
    mutationError.clear()
  })

  it("notifies subscribers on change and stops after unsubscribe", async () => {
    let notifications = 0
    const unsubscribe = mutationError.subscribe(() => {
      notifications++
    })

    await runMutation(Effect.fail(new Error("first")))
    expect(notifications).toBe(1)

    mutationError.clear()
    expect(notifications).toBe(2)
    // Already clear — no change, so no notification.
    mutationError.clear()
    expect(notifications).toBe(2)

    unsubscribe()
    await runMutation(Effect.fail(new Error("second")))
    expect(notifications).toBe(2)
    mutationError.clear()
  })
})

import { describe, expect, it } from "vitest"

import { type Draft, type DraftStore, type TextKind, TextDraft } from "../src/lib/text-draft"

const plain: TextKind<string> = {
  same: (a, b) => a === b,
  is: (value): value is string => typeof value === "string",
  refuse: () => undefined,
}

function memoryStore(initial?: Draft<string>): DraftStore<string> & { value: Draft<string> | undefined } {
  const store = {
    value: initial,
    read: () => store.value,
    write: (draft: Draft<string> | undefined) => {
      store.value = draft
    },
  }
  return store
}

interface Call {
  readonly base: string
  readonly text: string
  readonly answer: (body: string) => void
  readonly fail: (error: Error) => void
}

// A write that waits for the test to answer it, so a test can type while a save is on its way.
function pendingWrites() {
  const calls: Call[] = []
  const write = (base: string, text: string) =>
    new Promise<string>((answer, fail) => calls.push({ base, text, answer, fail }))
  return { calls, write }
}

const settle = () => new Promise((resolve) => setTimeout(resolve))

describe("TextDraft", () => {
  it("sends the text with the body it was typed over", async () => {
    const writes = pendingWrites()
    const draft = new TextDraft("A\n", writes.write, memoryStore(), plain)
    draft.change("B\n")
    expect(draft.getSnapshot().state.kind).toBe("unsaved")
    const saved = draft.save()
    expect(draft.getSnapshot().state.kind).toBe("saving")
    writes.calls[0].answer("B\n")
    await saved
    expect(writes.calls.map(({ base, text }) => [base, text])).toEqual([["A\n", "B\n"]])
    expect(draft.getSnapshot()).toEqual({ shown: "B\n", state: { kind: "saved" } })
  })

  it("sends nothing when the text is the body", async () => {
    const writes = pendingWrites()
    const draft = new TextDraft("A\n", writes.write, memoryStore(), plain)
    draft.change("B\n")
    draft.change("A\n")
    await draft.save()
    expect(writes.calls).toEqual([])
  })

  it("shows the merged body a save answers with", async () => {
    const writes = pendingWrites()
    const draft = new TextDraft("A\n", writes.write, memoryStore(), plain)
    draft.change("A\nmine\n")
    const saved = draft.save()
    writes.calls[0].answer("theirs\nA\nmine\n")
    await saved
    expect(draft.getSnapshot().shown).toBe("theirs\nA\nmine\n")
  })

  it("sends what was typed during a save from the text that save wrote", async () => {
    const writes = pendingWrites()
    const draft = new TextDraft("A\n", writes.write, memoryStore(), plain)
    draft.change("B\n")
    const first = draft.save()
    draft.change("C\n")
    void draft.save()
    writes.calls[0].answer("B\n")
    await settle()
    expect(writes.calls[1]).toMatchObject({ base: "B\n", text: "C\n" })
    writes.calls[1].answer("C\n")
    await first
    expect(draft.getSnapshot().state.kind).toBe("saved")
  })

  it("takes a new body in while nothing is unsaved, and keeps the reader's text otherwise", () => {
    const draft = new TextDraft("A\n", pendingWrites().write, memoryStore(), plain)
    draft.received("B\n")
    expect(draft.getSnapshot().shown).toBe("B\n")
    draft.change("mine\n")
    draft.received("C\n")
    expect(draft.getSnapshot().shown).toBe("B\n")
  })

  it("keeps an unsaved text in the store and forgets it once saved", async () => {
    const writes = pendingWrites()
    const store = memoryStore()
    const draft = new TextDraft("A\n", writes.write, store, plain)
    draft.change("B\n")
    expect(store.value).toEqual({ base: "A\n", text: "B\n" })
    const saved = draft.save()
    writes.calls[0].answer("B\n")
    await saved
    expect(store.value).toBeUndefined()
  })

  it("restores a stored text and saves it over the base it was typed on", async () => {
    const writes = pendingWrites()
    const draft = new TextDraft("A\nelse\n", writes.write, memoryStore({ base: "A\n", text: "B\n" }), plain)
    expect(draft.restored).toBe(true)
    expect(draft.getSnapshot()).toEqual({ shown: "B\n", state: { kind: "unsaved" } })
    void draft.save()
    expect(writes.calls[0]).toMatchObject({ base: "A\n", text: "B\n" })
  })

  it("keeps the text and says why when a save fails", async () => {
    const writes = pendingWrites()
    const draft = new TextDraft("A\n", writes.write, memoryStore(), plain)
    draft.change("B\n")
    const saved = draft.save()
    writes.calls[0].fail(new Error("the daemon is down"))
    await saved
    expect(draft.getSnapshot().state).toEqual({ kind: "failed", message: "the daemon is down" })
    const again = draft.save()
    expect(writes.calls[1]).toMatchObject({ base: "A\n", text: "B\n" })
    writes.calls[1].answer("B\n")
    await again
    expect(draft.getSnapshot().state.kind).toBe("saved")
  })

  it("refuses to send a text the owner refuses", async () => {
    const writes = pendingWrites()
    const draft = new TextDraft("A", writes.write, memoryStore(), {
      ...plain,
      refuse: (text) => (text.trim() === "" ? "A task needs a title." : undefined),
    })
    draft.change(" ")
    await draft.save()
    expect(writes.calls).toEqual([])
    expect(draft.getSnapshot().state).toEqual({ kind: "failed", message: "A task needs a title." })
  })
})

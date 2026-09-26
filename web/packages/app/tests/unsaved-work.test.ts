// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest"

import { holdsUnsavedWork } from "../src/lib/unsaved-work"

afterEach(() => {
  document.body.innerHTML = ""
})

describe("unsaved work", () => {
  it("is nothing on a page without dialogs or typed text", () => {
    document.body.innerHTML = `<input type="text" /><input type="checkbox" checked /><textarea></textarea>`
    expect(holdsUnsavedWork(document)).toBe(false)
  })

  it("is an open dialog", () => {
    document.body.innerHTML = `<div role="dialog">Keyboard shortcuts</div>`
    expect(holdsUnsavedWork(document)).toBe(true)
  })

  it("is text typed in a field", () => {
    document.body.innerHTML = `<input type="text" />`
    document.querySelector("input")!.value = "backend"
    expect(holdsUnsavedWork(document)).toBe(true)
  })

  it("is text typed in a text area", () => {
    document.body.innerHTML = `<textarea></textarea>`
    document.querySelector("textarea")!.value = "Keep the OAuth version."
    expect(holdsUnsavedWork(document)).toBe(true)
  })
})

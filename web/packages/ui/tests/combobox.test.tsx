import { act } from "react"
import { describe, expect, it, vi } from "vitest"

import { type ComboOption, Combobox } from "../src/combobox"
import { render } from "./render"

const setValue = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!

function options(query: string, chosen: (name: string) => void): ReadonlyArray<ComboOption> {
  const create = { key: " create", content: `Create ${query}`, onSelect: () => chosen(`create ${query}`) }
  const existing = ["Local setup", "Storage"]
    .filter((name) => name.toLowerCase().includes(query.toLowerCase()))
    .map((name) => ({ key: name, content: name, onSelect: () => chosen(name) }))
  return query === "" ? existing : [create, ...existing]
}

function typeAndEnter(container: HTMLElement, text: string) {
  const input = container.querySelector("input")!
  act(() => {
    setValue.call(input, text)
    input.dispatchEvent(new Event("input", { bubbles: true }))
  })
  act(() => {
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }))
  })
}

describe("Combobox", () => {
  it("takes the first option for the typed text when Enter comes before the debounce", () => {
    const chosen = vi.fn()
    const container = render(
      <Combobox placeholder="Find" buildOptions={(query) => options(query, chosen)} debounceMs={10_000} />,
    )

    typeAndEnter(container, "Indexes")

    expect(chosen).toHaveBeenCalledExactlyOnceWith("create Indexes")
  })

  it("takes the active option once the list shows the typed text", async () => {
    const chosen = vi.fn()
    const container = render(
      <Combobox placeholder="Find" buildOptions={(query) => options(query, chosen)} debounceMs={0} />,
    )
    const input = container.querySelector("input")!
    act(() => {
      setValue.call(input, "stor")
      input.dispatchEvent(new Event("input", { bubbles: true }))
    })
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 1))
    })
    act(() => {
      input.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true, cancelable: true }))
    })
    act(() => {
      input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }))
    })

    expect(chosen).toHaveBeenCalledExactlyOnceWith("Storage")
  })
})

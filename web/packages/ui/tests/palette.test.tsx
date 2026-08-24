import { act } from "react"
import { createRoot } from "react-dom/client"
import { describe, expect, it, vi } from "vitest"

import { Palette, type PaletteItem, type PaletteProvider } from "../src/palette"
import { render } from "./render"

const DEBOUNCE = 0

function provider(
  items: (query: string) => Promise<ReadonlyArray<PaletteItem>>,
  over: Partial<PaletteProvider> = {},
): PaletteProvider {
  return {
    id: "test",
    placeholder: "Search tasks",
    idleLabel: "Type to search",
    emptyLabel: "No matches",
    items,
    ...over,
  }
}

const named = (names: ReadonlyArray<string>, onSelect: (name: string) => void = () => {}): Array<PaletteItem> =>
  names.map((name) => ({ key: name, content: name, onSelect: () => onSelect(name) }))

const inputOf = (container: HTMLElement) => container.querySelector("input")!
const optionsOf = (container: HTMLElement) => [...container.querySelectorAll('[role="option"]')]
const activeOf = (container: HTMLElement) => container.querySelector('[role="option"][aria-selected="true"]')

// The provider resolves in a microtask and the debounce in a timer, so a keystroke lands on screen
// only after both have been let through.
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 1))
  })
}

// React tracks the last value it wrote to the node, so assigning `input.value` directly leaves it
// believing nothing changed. The prototype setter is what its tracker watches.
const setValue = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!

async function type(container: HTMLElement, text: string) {
  const input = inputOf(container)
  act(() => {
    setValue.call(input, text)
    input.dispatchEvent(new Event("input", { bubbles: true }))
  })
  await settle()
}

const press = (container: HTMLElement, key: string) =>
  act(() => {
    inputOf(container).dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }))
  })

async function open(
  items: (query: string) => Promise<ReadonlyArray<PaletteItem>>,
  onClose = () => {},
  over: Partial<PaletteProvider> = {},
): Promise<HTMLElement> {
  const container = render(<Palette open provider={provider(items, over)} onClose={onClose} debounceMs={DEBOUNCE} />)
  await settle()
  return container
}

describe("Palette", () => {
  it("renders nothing until it is open", () => {
    const container = render(
      <Palette open={false} provider={provider(async () => [])} onClose={() => {}} debounceMs={DEBOUNCE} />,
    )
    expect(container.textContent).toBe("")
  })

  it("takes the focus when it opens", async () => {
    const container = await open(async () => [])
    expect(document.activeElement).toBe(inputOf(container))
  })

  it("says what an empty list means before and after the reader types", async () => {
    const container = await open(async (query) => named(query === "a" ? ["Alpha"] : []))
    expect(container.textContent).toContain("Type to search")

    await type(container, "zz")
    expect(container.textContent).toContain("No matches")
  })

  it("asks the provider for the typed query and renders what it answers", async () => {
    const asked: Array<string> = []
    const container = await open(async (query) => {
      asked.push(query)
      return named(query === "" ? [] : ["Alpha", "Beta"])
    })

    await type(container, "al")
    expect(asked).toEqual(["", "al"])
    expect(optionsOf(container).map((option) => option.textContent)).toEqual(["Alpha", "Beta"])
  })

  it("walks the results with the arrow keys and wraps at both ends", async () => {
    const container = await open(async () => named(["Alpha", "Beta"]))
    expect(activeOf(container)?.textContent).toBe("Alpha")

    press(container, "ArrowDown")
    expect(activeOf(container)?.textContent).toBe("Beta")

    press(container, "ArrowDown")
    expect(activeOf(container)?.textContent).toBe("Alpha")

    press(container, "ArrowUp")
    expect(activeOf(container)?.textContent).toBe("Beta")
  })

  it("keeps walking when scrollIntoView answers with a promise, as Chrome's does", async () => {
    const original = Element.prototype.scrollIntoView
    Element.prototype.scrollIntoView = () => Promise.resolve() as unknown as void
    try {
      const container = await open(async () => named(["Alpha", "Beta", "Gamma"]))

      press(container, "ArrowDown")
      press(container, "ArrowDown")
      expect(activeOf(container)?.textContent).toBe("Gamma")
    } finally {
      Element.prototype.scrollIntoView = original
    }
  })

  it("selects the current result on Enter and closes", async () => {
    const selected: Array<string> = []
    const onClose = vi.fn()
    const container = await open(async () => named(["Alpha", "Beta"], (name) => selected.push(name)), onClose)

    press(container, "ArrowDown")
    press(container, "Enter")
    expect(selected).toEqual(["Beta"])
    expect(onClose).toHaveBeenCalledOnce()
  })

  it("selects nothing on Enter over an empty list", async () => {
    const onClose = vi.fn()
    const container = await open(async () => [], onClose)

    press(container, "Enter")
    expect(onClose).not.toHaveBeenCalled()
  })

  it("closes on Escape and on the backdrop, but not on the panel itself", async () => {
    const onClose = vi.fn()
    const container = await open(async () => [], onClose)

    press(container, "Escape")
    expect(onClose).toHaveBeenCalledOnce()

    act(() => {
      container.querySelector<HTMLElement>('[role="dialog"]')!.click()
    })
    expect(onClose).toHaveBeenCalledOnce()

    act(() => {
      container.querySelector<HTMLElement>('[role="presentation"]')!.click()
    })
    expect(onClose).toHaveBeenCalledTimes(2)
  })

  it("keeps the newest answer when an earlier one resolves after it", async () => {
    const pending: Array<(items: ReadonlyArray<PaletteItem>) => void> = []
    const container = await open((query) =>
      query === "" ? Promise.resolve([]) : new Promise((resolve) => pending.push(resolve)),
    )

    await type(container, "a")
    await type(container, "ab")
    act(() => {
      pending[1](named(["Newest"]))
      pending[0](named(["Stale"]))
    })
    await settle()

    expect(optionsOf(container).map((option) => option.textContent)).toEqual(["Newest"])
  })

  it("comes back empty, with nothing asked for the query the reader dismissed", async () => {
    const asked: Array<string> = []
    const items = async (query: string) => {
      asked.push(query)
      return named(query === "" ? [] : ["Alpha"])
    }
    function Harness({ open }: { open: boolean }) {
      return <Palette open={open} provider={provider(items)} onClose={() => {}} debounceMs={DEBOUNCE} />
    }
    const container = document.createElement("div")
    document.body.append(container)
    const root = createRoot(container)
    act(() => root.render(<Harness open />))
    await settle()

    await type(container, "al")
    expect(optionsOf(container).map((option) => option.textContent)).toEqual(["Alpha"])

    act(() => root.render(<Harness open={false} />))
    expect(container.textContent).toBe("")

    act(() => root.render(<Harness open />))
    expect(inputOf(container).value).toBe("")
    expect(optionsOf(container)).toEqual([])
    await settle()
    expect(asked).toEqual(["", "al", ""])

    act(() => root.unmount())
    container.remove()
  })

  it("reports a provider that fails instead of reading as no matches", async () => {
    const container = await open(async (query) => {
      if (query !== "") throw new Error("down")
      return []
    })

    await type(container, "a")
    expect(container.textContent).toContain("Could not run the search")
  })
})

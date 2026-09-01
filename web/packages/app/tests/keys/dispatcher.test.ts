// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { taskPath, taskRouteOf } from "@open-planner/task-ui"

import { copyTargetRow, hoveredRow } from "../../src/lib/copy-target"
import { bindings } from "../../src/lib/keys/bindings"
import { Dispatcher } from "../../src/lib/keys/dispatcher"
import { fromEvent, normalizeToken } from "../../src/lib/keys/match"
import type { Binding, OverlayName, PaletteTarget, RouteScope, RunContext } from "../../src/lib/keys/types"
import { focusedRow, rowCursor } from "../../src/lib/row-cursor"

const PROJECT = "open-plan"

interface Harness {
  readonly navigations: Array<string>
  readonly copied: Array<string | undefined>
  readonly overlay: { open: number; close: number; toggle: number }
  readonly closed: Array<OverlayName>
  readonly opened: Array<PaletteTarget>
  readonly detail: {
    showFlow: number
    editParent: number
    addSubtask: number
    editTags: number
    goToParent: number
    escape: number
  }
  setScope: (scope: RouteScope) => void
  setPath: (pathname: string) => void
  setOverlay: (name: OverlayName | null) => void
  detach: () => void
}

function mount(over: ReadonlyArray<Binding> = bindings): Harness {
  let scope: RouteScope = "list"
  let pathname = "/"
  let activeOverlay: OverlayName | null = null
  const navigations: Array<string> = []
  const copied: Array<string | undefined> = []
  const overlay = { open: 0, close: 0, toggle: 0 }
  const closed: Array<OverlayName> = []
  const opened: Array<PaletteTarget> = []
  const detail = { showFlow: 0, editParent: 0, addSubtask: 0, editTags: 0, goToParent: 0, escape: 0 }
  const context = (): RunContext => ({
    navigate: (to) => navigations.push(to),
    overlay: (name) => ({
      open: () => void overlay.open++,
      close: () => {
        overlay.close++
        closed.push(name)
      },
      toggle: () => void overlay.toggle++,
    }),
    palette: {
      open: (target) => void opened.push(target),
    },
    cursor: {
      moveBy: (delta) => {
        const hovered = hoveredRow.place(rowCursor.getSnapshot().rows)
        hoveredRow.clear()
        rowCursor.moveBy(delta, hovered)
      },
      focusedRow: () => {
        const cursor = rowCursor.getSnapshot()
        return focusedRow(cursor) ?? hoveredRow.among(cursor.rows)
      },
    },
    copy: {
      taskId: () => {
        const cursor = rowCursor.getSnapshot()
        const row = copyTargetRow(hoveredRow.among(cursor.rows), focusedRow(cursor), pathname)
        copied.push(row === undefined ? undefined : taskRouteOf(row)?.id)
      },
    },
    detail: {
      showFlow: () => void detail.showFlow++,
      editParent: () => void detail.editParent++,
      addSubtask: () => void detail.addSubtask++,
      editTags: () => void detail.editTags++,
      goToParent: () => void detail.goToParent++,
      escape: () => void detail.escape++,
    },
  })
  const dispatcher = new Dispatcher({
    bindings: over,
    routeScope: () => scope,
    activeOverlay: () => activeOverlay,
    context,
    chordTimeoutMs: 1000,
  })
  const detach = dispatcher.attach(window)
  return {
    navigations,
    copied,
    overlay,
    closed,
    opened,
    detail,
    setScope: (next) => void (scope = next),
    setPath: (next) => void (pathname = next),
    setOverlay: (next) => void (activeOverlay = next),
    detach,
  }
}

// A row is named by its task's path, which is what the cursor holds and what opening it navigates
// to.
const path = (id: string) => taskPath(PROJECT, id)
const paths = (...ids: ReadonlyArray<string>) => ids.map(path)

function press(key: string, target: EventTarget = window, init: KeyboardEventInit = {}) {
  target.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...init }))
}

beforeEach(() => {
  rowCursor.setRows([])
  hoveredRow.clear()
  document.body.innerHTML = ""
})

afterEach(() => {
  vi.useRealTimers()
})

describe("chord buffering", () => {
  it("fires the g-l chord exactly once when completed in time", () => {
    const h = mount()
    press("g")
    expect(h.navigations).toEqual([])
    press("l")
    expect(h.navigations).toEqual(["/"])

    press("g")
    press("l")
    expect(h.navigations).toEqual(["/", "/"])
    h.detach()
  })

  it("resets a partial chord after the timeout so a late second key does nothing", () => {
    vi.useFakeTimers()
    const h = mount()
    press("g")
    vi.advanceTimersByTime(1000)
    press("l")
    expect(h.navigations).toEqual([])
    h.detach()
  })

  it("a stray key mid-chord does not fire and does not strand the buffer", () => {
    const h = mount()
    press("g")
    press("x")
    expect(h.navigations).toEqual([])
    press("g")
    press("l")
    expect(h.navigations).toEqual(["/"])
    h.detach()
  })
})

describe("input scoping", () => {
  it("ignores single-key bindings while focus is in an editable element", () => {
    const h = mount()
    rowCursor.setRows(paths("a", "b", "c"))
    const input = document.createElement("input")
    document.body.append(input)
    input.focus()

    press("j", input)
    expect(rowCursor.getSnapshot().index).toBe(-1)

    press("j", document.body)
    expect(rowCursor.getSnapshot().index).toBe(0)
    h.detach()
  })

  it("yields Enter to a focused button or link instead of hijacking its native activation", () => {
    const h = mount()
    rowCursor.setRows(paths("a", "b", "c"))
    rowCursor.moveBy(1)
    const button = document.createElement("button")
    document.body.append(button)
    button.focus()

    press("Enter", button)
    expect(h.navigations).toEqual([])

    press("Enter", document.body)
    expect(h.navigations).toEqual([path("a")])
    h.detach()
  })

  it("still delivers a command-modified key from an editable element", () => {
    const h = mount()
    const input = document.createElement("input")
    document.body.append(input)
    input.focus()

    press("k", input, { metaKey: true })
    expect(h.opened).toEqual(["home"])
    h.detach()
  })
})

describe("cursor clamping", () => {
  it("k at the first row and j at the last row are no-ops", () => {
    const h = mount()
    rowCursor.setRows(paths("a", "b", "c"))

    press("k")
    expect(rowCursor.getSnapshot().index).toBe(0)
    press("k")
    expect(rowCursor.getSnapshot().index).toBe(0)

    press("j")
    press("j")
    expect(rowCursor.getSnapshot().index).toBe(2)
    press("j")
    expect(rowCursor.getSnapshot().index).toBe(2)
    h.detach()
  })
})

describe("row cursor is live on both the list and detail routes", () => {
  it("moves and opens from j/k/Enter while on a task detail page", () => {
    const h = mount()
    h.setScope("detail")
    rowCursor.setRows(paths("a", "b", "c"))

    press("j")
    press("j")
    expect(rowCursor.getSnapshot().index).toBe(1)

    press("Enter")
    expect(h.navigations).toEqual([path("b")])
    h.detach()
  })
})

describe("scope resolution", () => {
  it("routes Escape to the detail-view handler only on the detail route", () => {
    const h = mount()
    press("Escape")
    expect(h.detail.escape).toBe(0)

    h.setScope("detail")
    press("Escape")
    expect(h.detail.escape).toBe(1)
    h.detach()
  })

  it("triggers parent, subtask, and tag edits only on the detail route", () => {
    const h = mount()
    press("p")
    press("s")
    press("t")
    expect(h.detail).toEqual({ showFlow: 0, editParent: 0, addSubtask: 0, editTags: 0, goToParent: 0, escape: 0 })

    h.setScope("detail")
    press("p")
    press("s")
    press("t")
    expect(h.detail).toEqual({ showFlow: 0, editParent: 1, addSubtask: 1, editTags: 1, goToParent: 0, escape: 0 })
    h.detach()
  })

  it("distinguishes the g-p chord (go to parent) from a bare p (edit parent)", () => {
    const h = mount()
    h.setScope("detail")

    press("g")
    press("p")
    expect(h.detail).toMatchObject({ goToParent: 1, editParent: 0 })

    press("p")
    expect(h.detail).toMatchObject({ goToParent: 1, editParent: 1 })
    h.detach()
  })

  it("suppresses route and global bindings while the help overlay is open", () => {
    const h = mount()
    rowCursor.setRows(paths("a", "b", "c"))
    h.setOverlay("help")

    press("j")
    expect(rowCursor.getSnapshot().index).toBe(-1)
    press("g")
    press("l")
    expect(h.navigations).toEqual([])

    press("Escape")
    expect(h.overlay.close).toBe(1)
    press("?")
    expect(h.overlay.close).toBe(2)
    h.detach()
  })

  it("copies an id from either route, and not while the overlay is open", () => {
    const h = mount()
    rowCursor.setRows(paths("12", "13"))
    rowCursor.moveBy(1)
    press(".", window, { metaKey: true })
    expect(h.copied).toEqual(["12"])

    h.setScope("detail")
    h.setPath(path("28"))
    rowCursor.setRows([])
    press(".", window, { metaKey: true })
    expect(h.copied).toEqual(["12", "28"])

    h.setOverlay("help")
    press(".", window, { metaKey: true })
    expect(h.copied).toEqual(["12", "28"])
    h.detach()
  })

  it("copies the hovered row ahead of the keyboard selection, from Ctrl as well as Cmd", () => {
    const h = mount()
    rowCursor.setRows(paths("12", "13"))
    rowCursor.moveBy(1)
    hoveredRow.enter(path("13"), 1)

    press(".", window, { metaKey: true })
    press(".", window, { ctrlKey: true })
    expect(h.copied).toEqual(["13", "13"])
    h.detach()
  })

  it("copies the row j moved to, not the one the pointer was left resting on", () => {
    const h = mount()
    rowCursor.setRows(paths("12", "13", "14"))
    hoveredRow.enter(path("13"), 1)

    press("j")
    press(".", window, { metaKey: true })
    expect(h.copied).toEqual(["14"])
    h.detach()
  })

  it("j resumes from the hovered row, and from the first row when nothing is hovered", () => {
    const h = mount()
    rowCursor.setRows(paths("12", "13", "14"))
    hoveredRow.enter(path("13"), 1)
    press("j")
    expect(focusedRow(rowCursor.getSnapshot())).toBe(path("14"))

    rowCursor.clear()
    press("j")
    expect(focusedRow(rowCursor.getSnapshot())).toBe(path("12"))
    h.detach()
  })

  it("Enter opens the hovered row, which reads as the current one", () => {
    const h = mount()
    rowCursor.setRows(paths("12", "13"))
    hoveredRow.enter(path("13"), 1)

    press("Enter")
    expect(h.navigations).toEqual([path("13")])
    h.detach()
  })

  it("copies while a picker input has focus, where bare keys are left to the field", () => {
    const h = mount()
    rowCursor.setRows(paths("12"))
    rowCursor.moveBy(1)
    const input = document.createElement("input")
    document.body.append(input)
    input.focus()

    press(".", input, { metaKey: true })
    expect(h.copied).toEqual(["12"])
    h.detach()
  })

  it("resolves to no target when nothing is hovered, selected, or open", () => {
    const h = mount()
    press(".", window, { metaKey: true })
    expect(h.copied).toEqual([undefined])
    h.detach()
  })

  it("matches a Cmd+. event against the authored mod+. token", () => {
    const event = new KeyboardEvent("keydown", { key: ".", metaKey: true })
    expect(fromEvent(event)).toBe(normalizeToken("mod+."))
    expect(fromEvent(new KeyboardEvent("keydown", { key: ".", ctrlKey: true }))).toBe("mod+.")
  })

  it("? toggles the overlay from a normal route", () => {
    const h = mount()
    press("?")
    expect(h.overlay.toggle).toBe(1)
    h.detach()
  })

  it("opens the palette on home from Cmd+K and on search from /", () => {
    const h = mount()
    press("k", window, { metaKey: true })
    press("/")
    expect(h.opened).toEqual(["home", "search"])
    h.detach()
  })

  it("leaves / to a text field rather than opening the palette over it", () => {
    const h = mount()
    const input = document.createElement("input")
    document.body.append(input)
    input.focus()

    press("/", input)
    expect(h.opened).toEqual([])
    h.detach()
  })

  it("Escape closes the palette, and the help overlay's keys stay out of its scope", () => {
    const h = mount()
    h.setOverlay("palette")

    press("?")
    expect(h.overlay.close).toBe(0)

    press("Escape")
    expect(h.closed).toEqual(["palette"])
    h.detach()
  })
})

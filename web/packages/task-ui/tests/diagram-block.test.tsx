import { act } from "react"
import { describe, expect, it, vi } from "vitest"

import { type DiagramOutcome, DiagramDrawer } from "../src/diagram-drawer"
import { TaskBody } from "../src/task-body"
import { render } from "./render"

const draw = vi.fn(async (source: string): Promise<DiagramOutcome> => {
  await new Promise((resolve) => setTimeout(resolve, 1))
  if (source.includes("-->\n")) return { error: "line 2, column 7: expected a node id", line: 2 }
  if (source.trim() === "flowchart TD") return { drawing: { svg: "", width: 0, height: 0 } }
  const [, from, to] = /(\w) --> (\w)/.exec(source)!
  const label = `${from} to ${to}`
  return {
    drawing: {
      svg: `<svg class="op-diagram" viewBox="0 0 120 40" width="120" height="40"><text>${label}</text></svg>`,
      width: 120,
      height: 40,
    },
  }
})

async function settle(): Promise<void> {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20))
  })
}

function body(markdown: string): HTMLElement {
  return render(
    <DiagramDrawer value={draw}>
      <TaskBody project="openplan" abbreviation="OPP" markdown={markdown} />
    </DiagramDrawer>,
  )
}

async function drawn(markdown: string): Promise<HTMLElement> {
  const root = body(markdown)
  await settle()
  return root
}

describe("a mermaid fence", () => {
  it("says it is drawing until the drawing arrives", async () => {
    const root = body("```mermaid\nflowchart LR\n  w --> v\n```")
    expect(root.querySelector("pre")).toBeNull()
    const placeholder = root.querySelector("[data-diagram='drawing'][role='status']")!
    expect(placeholder.textContent).toContain("Drawing")
    await settle()
    expect(root.querySelector("[data-diagram='drawing']")).toBeNull()
    expect(root.querySelector("figure[data-diagram='drawn'] svg.op-diagram")).not.toBeNull()
  })

  it("puts the SVG inline, where the theme CSS reaches it", async () => {
    const root = await drawn("```mermaid\nflowchart LR\n  a --> b\n```")
    const svg = root.querySelector("figure[data-diagram='drawn'] svg")!
    expect(svg.textContent).toBe("a to b")
    expect(root.querySelector("figure img")).toBeNull()
  })

  it("sends the source of the fence to the daemon", async () => {
    draw.mockClear()
    await drawn("```mermaid\nflowchart LR\n  c --> d\n```")
    expect(draw).toHaveBeenCalledWith("flowchart LR\n  c --> d\n")
  })

  it("shows the message and marks the line where the source stops", async () => {
    const root = await drawn("```mermaid\nflowchart LR\n  a -->\n```")
    const block = root.querySelector("[data-diagram='error']")!
    expect(block.querySelector("[role='alert']")!.textContent).toBe("line 2, column 7: expected a node id")
    expect(block.querySelector("pre code")!.textContent).toBe("flowchart LR  a -->")
    expect(block.querySelector("[data-failed]")!.textContent).toBe("  a -->")
    expect(root.querySelector("svg")).toBeNull()
  })

  it("says so when the diagram is empty", async () => {
    const root = await drawn("```mermaid\nflowchart TD\n```")
    expect(root.querySelector("[data-diagram='empty']")!.textContent).toContain("empty")
  })

  it("reads the tag with any case", async () => {
    const root = await drawn("```Mermaid\nflowchart LR\n  e --> f\n```")
    expect(root.querySelector("figure[data-diagram='drawn']")).not.toBeNull()
  })

  it("leaves a d2 fence as code", async () => {
    const root = await drawn("```d2\na -> b\n```")
    expect(root.querySelector("[data-diagram]")).toBeNull()
    expect(root.querySelector("pre code")!.textContent).toBe("a -> b\n")
  })
})

describe("the full view of a diagram", () => {
  async function opened(markdown: string): Promise<HTMLElement> {
    const root = await drawn(markdown)
    act(() => {
      root.querySelector<HTMLElement>("figure[data-diagram='drawn'] button")!.click()
    })
    return document.querySelector<HTMLElement>("[role='dialog']")!
  }

  it("opens on a click and shows the same drawing in a viewport", async () => {
    const dialog = await opened("```mermaid\nflowchart LR\n  s --> t\n```")
    const viewport = dialog.querySelector("[role='group'][aria-label='Diagram']")!
    expect(viewport.querySelector("svg")!.textContent).toBe("s to t")
    expect(viewport.querySelector("[aria-label='Zoom in']")).not.toBeNull()
  })

  it("closes on Escape", async () => {
    const dialog = await opened("```mermaid\nflowchart LR\n  u --> v\n```")
    act(() => {
      dialog.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }))
    })
    expect(document.querySelector("[role='dialog']")).toBeNull()
  })

  it("closes from the button on the drawing", async () => {
    const dialog = await opened("```mermaid\nflowchart LR\n  m --> o\n```")
    act(() => {
      dialog.querySelector<HTMLElement>("[aria-label='Close']")!.click()
    })
    expect(document.querySelector("[role='dialog']")).toBeNull()
  })

  it("keeps the page's single-key bindings off the keys pressed in it", async () => {
    const dialog = await opened("```mermaid\nflowchart LR\n  w --> z\n```")
    expect(dialog.hasAttribute("data-keys-ignore")).toBe(true)
  })
})

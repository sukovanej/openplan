import { act } from "react"
import { describe, expect, it, vi } from "vitest"

import { TaskBody } from "../src/task-body"
import { render } from "./render"

// The real engine answers one message at a time, so the fake counts the calls in flight and the
// tests assert the count never passes one.
let inFlight = 0
let mostInFlight = 0

async function oneMessage<T>(reply: () => T): Promise<T> {
  inFlight += 1
  mostInFlight = Math.max(mostInFlight, inFlight)
  await new Promise((resolve) => setTimeout(resolve, 1))
  inFlight -= 1
  return reply()
}

const compile = vi.fn(({ fs }: { fs: { index: string } }) =>
  oneMessage(() => {
    const source = fs.index
    if (source.includes("->\n")) {
      throw new Error(
        JSON.stringify([{ range: "index,0:0:0-0:4:4", errmsg: "index:1:1: connection missing destination" }]),
      )
    }
    return { diagram: { source }, renderOptions: { pad: 16 } }
  }),
)

vi.mock("@terrastruct/d2", () => ({
  D2: class {
    compile = compile
    render(diagram: { source: string }, options: { themeID: number }): Promise<string> {
      return oneMessage(
        () => `<svg viewBox="0 0 120 40"><text>${diagram.source.trim()} theme ${options.themeID}</text></svg>`,
      )
    }
  },
}))

async function drawn(markdown: string): Promise<HTMLElement> {
  const root = render(<TaskBody project="openplan" abbreviation="OPP" markdown={markdown} />)
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20))
  })
  return root
}

function decoded(img: Element): string {
  return decodeURIComponent(img.getAttribute("src")!.replace("data:image/svg+xml;charset=utf-8,", ""))
}

describe("a d2 fence", () => {
  it("draws the light and the dark theme and lets CSS pick one", async () => {
    const root = await drawn("```d2\na -> b\n```")
    const figure = root.querySelector("figure[data-diagram='drawn']")!
    expect(root.querySelector("pre")).toBeNull()
    const images = [...figure.querySelectorAll("img")]
    expect(images).toHaveLength(2)
    expect(decoded(images[0]!)).toContain("a -> b theme 0")
    expect(images[0]!.className).toContain("dark:hidden")
    expect(decoded(images[1]!)).toContain("a -> b theme 200")
    expect(images[1]!.className).toContain("dark:block")
    expect(images[0]!.getAttribute("width")).toBe("90")
    expect(images[0]!.getAttribute("height")).toBe("30")
  })

  it("keeps the source and names the problem when the compiler refuses it", async () => {
    const root = await drawn("```d2\na ->\n```")
    const block = root.querySelector("[data-diagram='error']")!
    expect(block.querySelector("[role='alert']")!.textContent).toBe("1:1: connection missing destination")
    expect(block.querySelector("pre code")!.textContent).toBe("a ->\n")
  })

  it("compiles one source once", async () => {
    compile.mockClear()
    await drawn("```d2\nx -> y\n```")
    await drawn("```d2\nx -> y\n```")
    expect(compile).toHaveBeenCalledTimes(1)
  })

  it("sends the engine one message at a time across diagrams", async () => {
    mostInFlight = 0
    const root = await drawn("```d2\np -> q\n```\n\n```d2\nq -> r\n```\n\n```d2\nr ->\n```")
    expect(root.querySelectorAll("figure[data-diagram='drawn']")).toHaveLength(2)
    expect(root.querySelectorAll("[data-diagram='error']")).toHaveLength(1)
    expect(mostInFlight).toBe(1)
  })

  it("reads the tag with any case", async () => {
    const root = await drawn("```D2\nc -> d\n```")
    expect(root.querySelector("figure[data-diagram='drawn']")).not.toBeNull()
  })
})

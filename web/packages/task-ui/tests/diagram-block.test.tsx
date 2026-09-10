import { act } from "react"
import { afterEach, describe, expect, it, vi } from "vitest"

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

type CompileRequest = { fs: { index: string }; options: { sketch?: boolean; layout?: string } }

const compile = vi.fn(({ fs }: CompileRequest) =>
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

const renderSvg = vi.fn((diagram: { source: string }, options: { themeID: number }) =>
  oneMessage(() => `<svg viewBox="0 0 120 40"><text>${diagram.source.trim()} theme ${options.themeID}</text></svg>`),
)

vi.mock("@terrastruct/d2", () => ({
  D2: class {
    compile = compile
    render = renderSvg
  },
}))

afterEach(() => {
  document.documentElement.classList.remove("dark")
})

async function settle(): Promise<void> {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20))
  })
}

async function drawn(markdown: string): Promise<HTMLElement> {
  const root = render(<TaskBody project="openplan" abbreviation="OPP" markdown={markdown} />)
  await settle()
  return root
}

function decoded(img: Element): string {
  return decodeURIComponent(img.getAttribute("src")!.replace("data:image/svg+xml;charset=utf-8,", ""))
}

describe("a d2 fence", () => {
  it("says it is drawing until the picture arrives", async () => {
    const root = render(<TaskBody project="openplan" abbreviation="OPP" markdown={"```d2\nw -> v\n```"} />)
    expect(root.querySelector("pre")).toBeNull()
    const placeholder = root.querySelector("[data-diagram='drawing'][role='status']")!
    expect(placeholder.textContent).toContain("Drawing")
    expect(placeholder.querySelectorAll("[data-slot='skeleton']").length).toBeGreaterThan(1)
    await settle()
    expect(root.querySelector("[data-diagram='drawing']")).toBeNull()
    expect(root.querySelector("figure[data-diagram='drawn'] img")).not.toBeNull()
  })

  it("draws the current theme only", async () => {
    renderSvg.mockClear()
    const root = await drawn("```d2\na -> b\n```")
    const images = [...root.querySelectorAll("figure[data-diagram='drawn'] img")]
    expect(images).toHaveLength(1)
    expect(decoded(images[0]!)).toContain("a -> b theme 3")
    expect(images[0]!.getAttribute("width")).toBe("90")
    expect(images[0]!.getAttribute("height")).toBe("30")
    expect(renderSvg).toHaveBeenCalledTimes(1)
  })

  it("draws the dark theme when the page is dark", async () => {
    document.documentElement.classList.add("dark")
    const root = await drawn("```d2\ne -> f\n```")
    expect(decoded(root.querySelector("figure[data-diagram='drawn'] img")!)).toContain("e -> f theme 200")
  })

  it("keeps the picture up while a theme flip draws the other one", async () => {
    compile.mockClear()
    const root = await drawn("```d2\ng -> h\n```")
    act(() => {
      document.documentElement.classList.add("dark")
    })
    await act(async () => {
      await Promise.resolve()
    })
    expect(root.querySelector("[data-diagram='drawing']")).toBeNull()
    expect(decoded(root.querySelector("figure[data-diagram='drawn'] img")!)).toContain("theme 3")
    await settle()
    expect(decoded(root.querySelector("figure[data-diagram='drawn'] img")!)).toContain("g -> h theme 200")
    expect(compile).toHaveBeenCalledTimes(1)
  })

  it("asks for the sketch look and the elk layout", async () => {
    await drawn("```d2\nm -> n\n```")
    const request = compile.mock.calls.at(-1)![0]
    expect(request.options.sketch).toBe(true)
    expect(request.options.layout).toBe("elk")
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

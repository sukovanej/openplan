import { describe, expect, it } from "vitest"

import { joinBody, splitBody } from "../src/lib/task-content"

const unchanged = (body: string) => {
  const content = splitBody(body)
  return joinBody(content, content.title, content.description)
}

describe("splitBody and joinBody", () => {
  it("give back an untouched body byte for byte", () => {
    for (const body of ["# Title\n\nText.\n", "\n# Title\nText", "# Title", "# Title\n", "No title.\n", ""]) {
      expect(unchanged(body)).toBe(body)
    }
  })

  it("take the title from the first line that holds text", () => {
    expect(splitBody("\n# Plan  \n\nBody\n")).toEqual({
      head: "\n",
      title: "Plan  ",
      titleEnd: "\n",
      description: "\nBody\n",
    })
  })

  it("change the title and keep the description", () => {
    const content = splitBody("# Old\n\nText.\n")
    expect(joinBody(content, "New", content.description)).toBe("# New\n\nText.\n")
  })

  it("put a blank line between the title and a first description", () => {
    expect(joinBody(splitBody("# Title"), "Title", "Text.\n")).toBe("# Title\n\nText.\n")
  })

  it("add a title line to a body that has none once the title changes", () => {
    const content = splitBody("Text.\n")
    expect(joinBody(content, undefined, content.description)).toBe("Text.\n")
    expect(joinBody(content, "Named", content.description)).toBe("# Named\n\nText.\n")
  })
})

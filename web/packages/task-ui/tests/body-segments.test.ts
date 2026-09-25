import { describe, expect, it } from "vitest"

import { type BodySegment, bodySegments } from "../src/body-segments"

const BLOCK = [
  "<<<<<<< Ann (a1b2c3d)\n",
  "Use OAuth only.\n",
  "=======\n",
  "Use OAuth and email login.\n",
  ">>>>>>> Ben (e4f5a6b)\n",
].join("")

const texts = (segments: ReadonlyArray<BodySegment>) =>
  segments.map((segment) => (segment.kind === "text" ? segment.text : segment.block))

// Every segment's text, in order, is the body: a resolve names a block by its exact text.
const joined = (segments: ReadonlyArray<BodySegment>) => texts(segments).join("")

describe("bodySegments", () => {
  it("reads a body with no markers as one text segment", () => {
    expect(bodySegments("# Title\n\nSome text.\n")).toEqual([{ kind: "text", text: "# Title\n\nSome text.\n" }])
  })

  it("reads nothing from an empty body", () => {
    expect(bodySegments("")).toEqual([])
  })

  it("splits a block from the text around it, with both versions and their labels", () => {
    const body = `# Title\n\nIntro.\n${BLOCK}\nOutro.\n`
    const segments = bodySegments(body)
    expect(segments).toEqual([
      { kind: "text", text: "# Title\n\nIntro.\n" },
      {
        kind: "conflict",
        block: BLOCK,
        other: { label: "Ann (a1b2c3d)", text: "Use OAuth only.\n" },
        published: { label: "Ben (e4f5a6b)", text: "Use OAuth and email login.\n" },
      },
      { kind: "text", text: "\nOutro.\n" },
    ])
    expect(joined(segments)).toBe(body)
  })

  it("keeps a block that ends the body without a final newline exactly as written", () => {
    const body = BLOCK.slice(0, -1)
    const [segment] = bodySegments(body)
    expect(segment).toMatchObject({ kind: "conflict", block: body })
  })

  it("skips the base section between the first version and the split", () => {
    const body = "<<<<<<< Ann\nmine\n||||||| base\nwas\n=======\ntheirs\n>>>>>>> Ben\n"
    expect(bodySegments(body)).toEqual([
      {
        kind: "conflict",
        block: body,
        other: { label: "Ann", text: "mine\n" },
        published: { label: "Ben", text: "theirs\n" },
      },
    ])
  })

  it("reads two blocks in a row as two blocks", () => {
    const segments = bodySegments(`${BLOCK}${BLOCK}`)
    expect(segments.map((segment) => segment.kind)).toEqual(["conflict", "conflict"])
  })

  it("keeps a title line that sits inside a block in each version", () => {
    const body = "<<<<<<< Ann\n# Login\n=======\n# Sign in\n>>>>>>> Ben\n\nText.\n"
    const [block] = bodySegments(body)
    expect(block).toMatchObject({ other: { text: "# Login\n" }, published: { text: "# Sign in\n" } })
  })

  it.each([
    ["backtick", "```"],
    ["tilde", "~~~"],
  ])("reads marker lines inside a %s fence as text", (_, fence) => {
    const body = `Example:\n\n${fence}text\n${BLOCK}${fence}\n`
    expect(bodySegments(body)).toEqual([{ kind: "text", text: body }])
  })

  it("keeps a whole fenced code block inside a version, markers in it included", () => {
    const code = "```\n=======\n>>>>>>> not the end\n```\n"
    const body = `<<<<<<< Ann\n${code}=======\nplain\n>>>>>>> Ben\n`
    expect(bodySegments(body)).toEqual([
      {
        kind: "conflict",
        block: body,
        other: { label: "Ann", text: code },
        published: { label: "Ben", text: "plain\n" },
      },
    ])
  })

  it("closes a fence only with a bare run of the same character at least as long", () => {
    const body = "````\n```\n~~~~\n```` still open\n<<<<<<< a\nx\n=======\ny\n>>>>>>> b\n````\n"
    expect(bodySegments(body)).toEqual([{ kind: "text", text: body }])
  })

  it("reads a fence indented by four spaces as text, so it opens nothing", () => {
    const body = `    \`\`\`\n${BLOCK}`
    expect(bodySegments(body).map((segment) => segment.kind)).toEqual(["text", "conflict"])
  })

  it.each([
    ["no split", "<<<<<<< Ann\nmine\n>>>>>>> Ben\n"],
    ["no end", "<<<<<<< Ann\nmine\n=======\ntheirs\n"],
    ["no start", "mine\n=======\ntheirs\n>>>>>>> Ben\n"],
  ])("reads a block with %s as text", (_, body) => {
    expect(bodySegments(body)).toEqual([{ kind: "text", text: body }])
  })

  it("reads a second start inside an open block as text of the first version", () => {
    const body = "<<<<<<< Ann\nmine\n<<<<<<< Cid\nmore\n=======\ntheirs\n>>>>>>> Ben\n"
    expect(bodySegments(body)).toEqual([
      {
        kind: "conflict",
        block: body,
        other: { label: "Ann", text: "mine\n<<<<<<< Cid\nmore\n" },
        published: { label: "Ben", text: "theirs\n" },
      },
    ])
  })

  it("takes a marker only when its sign stands alone or a space follows it", () => {
    const body = "<<<<<<<< Ann\nmine\n=======\ntheirs\n>>>>>>> Ben\n"
    expect(bodySegments(body)).toEqual([{ kind: "text", text: body }])
  })

  it("reads a marker line that ends in a carriage return", () => {
    const body = "<<<<<<< Ann\r\nmine\r\n=======\r\ntheirs\r\n>>>>>>> Ben\r\n"
    expect(bodySegments(body)).toEqual([
      {
        kind: "conflict",
        block: body,
        other: { label: "Ann", text: "mine\r\n" },
        published: { label: "Ben", text: "theirs\r\n" },
      },
    ])
  })
})

// The daemon's reader (`op_task::conflict::blocks`) decides what a block is, and a resolve names the
// block by its exact text, so this reads the markers by the same rules, line for line.
const START = "<<<<<<<"
const BASE = "|||||||"
const SPLIT = "======="
const END = ">>>>>>>"

export interface ConflictVersion {
  readonly label: string
  readonly text: string
}

// `other` comes first, as git writes it; `published` is the version the remote already published,
// which is in force until someone picks. `block` is the whole block as the body holds it, markers
// included.
export interface ConflictBlock {
  readonly block: string
  readonly other: ConflictVersion
  readonly published: ConflictVersion
}

export type BodySegment =
  | { readonly kind: "text"; readonly text: string }
  | ({ readonly kind: "conflict" } & ConflictBlock)

type MarkerKind = "start" | "base" | "split" | "end"

interface Marker {
  readonly kind: MarkerKind
  readonly label: string
}

const LABELLED: ReadonlyArray<readonly [string, MarkerKind]> = [
  [START, "start"],
  [BASE, "base"],
  [END, "end"],
]

function marker(line: string): Marker | undefined {
  if (line === SPLIT) return { kind: "split", label: "" }
  for (const [sign, kind] of LABELLED) {
    if (!line.startsWith(sign)) continue
    const rest = line.slice(sign.length)
    if (rest === "" || rest.startsWith(" ")) return { kind, label: rest.trim() }
  }
  return undefined
}

// The fence left open after `line`: a run of three or more of one fence character opens one, and a
// bare run at least as long of the same character closes it.
function fenceAfter(open: string | undefined, line: string): string | undefined {
  const trimmed = line.trimStart()
  if (line.length - trimmed.length > 3) return open
  const sign = /^[`~]*/.exec(trimmed)?.[0] ?? ""
  if (sign.length < 3 || [...sign].some((char) => char !== sign[0])) return open
  if (open === undefined) return sign
  return sign.startsWith(open) && trimmed.slice(sign.length).trim() === "" ? undefined : open
}

const linesOf = (text: string): ReadonlyArray<string> => text.match(/[^\n]*\n|[^\n]+$/g) ?? []

interface OpenBlock {
  readonly start: number
  part: "other" | "base" | "published"
  readonly otherLabel: string
  other: string
  published: string
}

function append(open: OpenBlock | undefined, line: string) {
  if (open?.part === "other") open.other += line
  if (open?.part === "published") open.published += line
}

// A block lacking any of its three markers is text, and so is every marker line inside a fenced
// code block, so a task can show a conflict as an example.
export function bodySegments(body: string): ReadonlyArray<BodySegment> {
  const segments: Array<BodySegment> = []
  let fence: string | undefined
  let open: OpenBlock | undefined
  let textStart = 0
  let offset = 0
  for (const line of linesOf(body)) {
    const start = offset
    offset += line.length
    const bare = line.replace(/[\r\n]+$/, "")
    if (fence !== undefined) {
      fence = fenceAfter(fence, bare)
      append(open, line)
      continue
    }
    const found = marker(bare)
    if (open === undefined && found?.kind === "start") {
      open = { start, part: "other", otherLabel: found.label, other: "", published: "" }
    } else if (open !== undefined && found?.kind === "base" && open.part === "other") {
      open.part = "base"
    } else if (open !== undefined && found?.kind === "split" && open.part !== "published") {
      open.part = "published"
    } else if (open !== undefined && found?.kind === "end" && open.part === "published") {
      if (open.start > textStart) segments.push({ kind: "text", text: body.slice(textStart, open.start) })
      segments.push({
        kind: "conflict",
        block: body.slice(open.start, offset),
        other: { label: open.otherLabel, text: open.other },
        published: { label: found.label, text: open.published },
      })
      textStart = offset
      open = undefined
    } else {
      fence = fenceAfter(fence, bare)
      append(open, line)
    }
  }
  if (textStart < body.length) segments.push({ kind: "text", text: body.slice(textStart) })
  return segments
}

export type DiffLineKind = "context" | "added" | "deleted"

export interface DiffSpan {
  readonly text: string
  readonly changed: boolean
}

export interface DiffLine {
  readonly kind: DiffLineKind
  readonly before: number | null
  readonly after: number | null
  readonly spans: ReadonlyArray<DiffSpan>
}

export interface DiffHunk {
  readonly ranges: string
  readonly lines: ReadonlyArray<DiffLine>
}

interface Row {
  kind: DiffLineKind
  before: number | null
  after: number | null
  text: string
}

const HUNK = /^@@ (-(\d+)(?:,\d+)? \+(\d+)(?:,\d+)?) @@/

// Everything before the first `@@` is the file header, which the caller renders itself, and
// everything after a line git owns but a reader does not — `\ No newline at end of file` — belongs
// to neither side.
export function parseDiff(diff: string): DiffHunk[] {
  const hunks: Array<{ ranges: string; rows: Row[] }> = []
  let open: { ranges: string; rows: Row[] } | undefined
  let before = 0
  let after = 0
  for (const line of diff.split("\n")) {
    const header = HUNK.exec(line)
    if (header !== null) {
      open = { ranges: header[1], rows: [] }
      hunks.push(open)
      before = Number(header[2])
      after = Number(header[3])
      continue
    }
    if (open === undefined || line === "" || line.startsWith("\\")) continue
    const text = line.slice(1)
    switch (line[0]) {
      case " ":
        open.rows.push({ kind: "context", before: before++, after: after++, text })
        break
      case "-":
        open.rows.push({ kind: "deleted", before: before++, after: null, text })
        break
      case "+":
        open.rows.push({ kind: "added", before: null, after: after++, text })
        break
      default:
        open = undefined
    }
  }
  return hunks.map((hunk) => ({ ranges: hunk.ranges, lines: emphasize(hunk.rows) }))
}

// A task file is markdown prose with one paragraph on one line, so a line-level mark alone hides
// the word that changed. Each removed line pairs with the added line at the same offset in the run
// that follows it; a run with no partner keeps its whole line marked by kind alone.
function emphasize(rows: ReadonlyArray<Row>): DiffLine[] {
  const lines: DiffLine[] = rows.map((row) => ({ ...row, spans: plain(row.text) }))
  let at = 0
  while (at < rows.length) {
    const removed = run(rows, at, "deleted")
    const added = run(rows, removed, "added")
    for (let step = 0; step < removed - at && step < added - removed; step++) {
      const [was, now] = compare(rows[at + step].text, rows[removed + step].text)
      lines[at + step] = { ...lines[at + step], spans: was }
      lines[removed + step] = { ...lines[removed + step], spans: now }
    }
    at = Math.max(added, at + 1)
  }
  return lines
}

function run(rows: ReadonlyArray<Row>, from: number, kind: DiffLineKind): number {
  let end = from
  while (end < rows.length && rows[end].kind === kind) end++
  return end
}

function compare(was: string, now: string): [DiffSpan[], DiffSpan[]] {
  const a = words(was)
  const b = words(now)
  const kept = shared(a, b)
  // Two lines with no word in common are a rewrite, and marking every word of both says nothing the
  // line's own colour does not.
  if (!kept.a.includes(true)) return [plain(was), plain(now)]
  return [spans(a, kept.a), spans(b, kept.b)]
}

// Each word carries the space that follows it, so a run of changed words renders as one span rather
// than as words separated by unmarked gaps.
function words(text: string): string[] {
  return text.match(/\s*\S+\s*/g) ?? []
}

// Which words the two lines share, by longest common subsequence. The trailing space a word carries
// is for rendering, so the comparison drops it.
function shared(a: ReadonlyArray<string>, b: ReadonlyArray<string>): { a: boolean[]; b: boolean[] } {
  const left = a.map((word) => word.trim())
  const right = b.map((word) => word.trim())
  const table = Array.from({ length: left.length + 1 }, () => new Array<number>(right.length + 1).fill(0))
  for (let i = left.length - 1; i >= 0; i--) {
    for (let j = right.length - 1; j >= 0; j--) {
      table[i][j] = left[i] === right[j] ? table[i + 1][j + 1] + 1 : Math.max(table[i + 1][j], table[i][j + 1])
    }
  }
  const inA = new Array<boolean>(left.length).fill(false)
  const inB = new Array<boolean>(right.length).fill(false)
  let i = 0
  let j = 0
  while (i < left.length && j < right.length) {
    if (left[i] === right[j]) {
      inA[i] = true
      inB[j] = true
      i++
      j++
    } else if (table[i + 1][j] >= table[i][j + 1]) {
      i++
    } else {
      j++
    }
  }
  return { a: inA, b: inB }
}

function spans(words: ReadonlyArray<string>, kept: ReadonlyArray<boolean>): DiffSpan[] {
  const out: DiffSpan[] = []
  for (let at = 0; at < words.length; at++) {
    const changed = !kept[at]
    const last = out[out.length - 1]
    if (last !== undefined && last.changed === changed) {
      out[out.length - 1] = { text: last.text + words[at], changed }
    } else {
      out.push({ text: words[at], changed })
    }
  }
  return out
}

function plain(text: string): DiffSpan[] {
  return text === "" ? [] : [{ text, changed: false }]
}

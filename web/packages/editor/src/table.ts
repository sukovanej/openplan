export type Align = "left" | "center" | "right" | null

// Offsets count from the start of the table source. An empty cell has `from === to`, one space in.
export interface Cell {
  readonly text: string
  readonly from: number
  readonly to: number
}

export interface Row {
  readonly cells: ReadonlyArray<Cell>
  // After the closing pipe, or after the last character when the row has none.
  readonly end: number
  readonly closed: boolean
}

export interface Table {
  readonly rows: ReadonlyArray<Row>
  readonly align: ReadonlyArray<Align>
  // A table in a list item or a quote repeats the item's indent or the quote marks on each line
  // after the first.
  readonly indent: string
}

export interface Grid {
  readonly rows: ReadonlyArray<ReadonlyArray<string>>
  readonly align: ReadonlyArray<Align>
}

export interface Spot {
  readonly row: number
  readonly col: number
}

export interface TextChange {
  readonly from: number
  readonly to: number
  readonly insert: string
}

// GFM splits a row at each pipe that no backslash escapes, inside code spans too.
const unescapeCell = (raw: string) => raw.replace(/\\\|/g, "|")
export const escapeCell = (text: string) => text.replace(/\r?\n/g, " ").trim().replace(/\|/g, "\\|")

function splitRow(line: string, offset: number): Row {
  const pipes: number[] = []
  for (let at = 0; at < line.length; at++) {
    if (line[at] === "\\") at++
    else if (line[at] === "|") pipes.push(at)
  }
  const first = line.search(/\S/)
  const last = line.trimEnd().length - 1
  const open = pipes.length > 0 && pipes[0] === first
  const closed = pipes.length > (open ? 1 : 0) && pipes[pipes.length - 1] === last
  const inner = pipes.slice(open ? 1 : 0, closed ? -1 : undefined)
  const edges = [open ? pipes[0] : -1, ...inner, closed ? last : line.length]
  const cells: Cell[] = []
  for (let index = 0; index + 1 < edges.length; index++) {
    const start = edges[index] + 1
    const span = line.slice(start, edges[index + 1])
    const content = span.trim()
    const from = offset + start + (content === "" ? Math.min(1, span.length) : span.length - span.trimStart().length)
    cells.push({ text: unescapeCell(content), from, to: from + content.length })
  }
  return { cells, end: offset + (closed ? last + 1 : line.trimEnd().length), closed }
}

function alignOf(delimiter: string): Align {
  const left = delimiter.startsWith(":")
  const right = delimiter.endsWith(":")
  if (left && right) return "center"
  if (left) return "left"
  if (right) return "right"
  return null
}

export function parseTable(source: string): Table {
  const lines: { text: string; from: number }[] = []
  let offset = 0
  for (const text of source.split("\n")) {
    lines.push({ text, from: offset })
    offset += text.length + 1
  }
  const [header, delimiter, ...body] = lines
  const indent = /^[\s>]*/.exec(delimiter?.text ?? "")?.[0] ?? ""
  const rowOf = (line: { text: string; from: number }) => {
    const skip = line.text.startsWith(indent) ? indent.length : 0
    return splitRow(line.text.slice(skip), line.from + skip)
  }
  return {
    rows: [splitRow(header.text, header.from), ...body.map(rowOf)],
    align: rowOf(delimiter ?? { text: "", from: 0 }).cells.map((cell) => alignOf(cell.text)),
    indent,
  }
}

export const columnsOf = (table: Table): number => table.rows[0].cells.length

// Only the one cell's text changes, so an edit leaves the rest of the source as it was written.
export function cellChange(table: Table, { row, col }: Spot, text: string): TextChange {
  const target = table.rows[row]
  const insert = escapeCell(text)
  const cell = target.cells[col]
  if (cell !== undefined) return { from: cell.from, to: cell.to, insert }
  const gap = " |".repeat(col - target.cells.length)
  const tail = target.closed ? `${gap} ${insert} |` : `${gap} | ${insert}`
  return { from: target.end, to: target.end, insert: tail }
}

export function gridOf(table: Table): Grid {
  const columns = columnsOf(table)
  return {
    rows: table.rows.map((row) => Array.from({ length: columns }, (_, col) => row.cells[col]?.text ?? "")),
    align: Array.from({ length: columns }, (_, col) => table.align[col] ?? null),
  }
}

const blankRow = (grid: Grid) => grid.align.map(() => "")

export const insertRow = (grid: Grid, at: number): Grid => ({
  ...grid,
  rows: [...grid.rows.slice(0, at), blankRow(grid), ...grid.rows.slice(at)],
})

export const deleteRow = (grid: Grid, at: number): Grid => ({
  ...grid,
  rows: grid.rows.filter((_, index) => index !== at),
})

export const insertColumn = (grid: Grid, at: number): Grid => ({
  rows: grid.rows.map((row) => [...row.slice(0, at), "", ...row.slice(at)]),
  align: [...grid.align.slice(0, at), null, ...grid.align.slice(at)],
})

export const deleteColumn = (grid: Grid, at: number): Grid => ({
  rows: grid.rows.map((row) => row.filter((_, index) => index !== at)),
  align: grid.align.filter((_, index) => index !== at),
})

export const alignColumn = (grid: Grid, at: number, align: Align): Grid => ({
  ...grid,
  align: grid.align.map((current, index) => (index === at ? align : current)),
})

function pad(text: string, width: number, align: Align): string {
  const room = width - text.length
  if (align === "right") return text.padStart(width)
  if (align === "center") return " ".repeat(Math.floor(room / 2)) + text + " ".repeat(Math.ceil(room / 2))
  return text.padEnd(width)
}

function delimiterOf(width: number, align: Align): string {
  if (align === "center") return `:${"-".repeat(width - 2)}:`
  if (align === "left") return `:${"-".repeat(width - 1)}`
  if (align === "right") return `${"-".repeat(width - 1)}:`
  return "-".repeat(width)
}

export function formatTable(grid: Grid, indent: string): string {
  const rows = grid.rows.map((row) => row.map(escapeCell))
  const widths = grid.align.map((_, col) => Math.max(3, ...rows.map((row) => row[col].length)))
  const line = (cells: ReadonlyArray<string>) => `| ${cells.join(" | ")} |`
  const [header, ...body] = rows.map((row) => line(row.map((text, col) => pad(text, widths[col], grid.align[col]))))
  const delimiter = line(grid.align.map((align, col) => delimiterOf(widths[col], align)))
  return [header, delimiter, ...body].join(`\n${indent}`)
}

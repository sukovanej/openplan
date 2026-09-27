import { describe, expect, it } from "vitest"

import {
  alignColumn,
  cellChange,
  deleteColumn,
  deleteRow,
  formatTable,
  gridOf,
  insertColumn,
  insertRow,
  parseTable,
  type TextChange,
} from "../src/table"

const apply = (source: string, change: TextChange) =>
  source.slice(0, change.from) + change.insert + source.slice(change.to)

const edit = (source: string, row: number, col: number, text: string) =>
  apply(source, cellChange(parseTable(source), { row, col }, text))

describe("a markdown table", () => {
  it("reads the cells, the rows, and the alignment", () => {
    const table = parseTable("| a | b | c |\n|:--|:-:|--:|\n| 1 | 2 | 3 |")
    expect(gridOf(table)).toEqual({
      rows: [
        ["a", "b", "c"],
        ["1", "2", "3"],
      ],
      align: ["left", "center", "right"],
    })
  })

  it("reads rows without the outer pipes", () => {
    expect(gridOf(parseTable("a | b\n--- | ---\n1 | 2")).rows).toEqual([
      ["a", "b"],
      ["1", "2"],
    ])
  })

  it("keeps an escaped pipe inside its cell", () => {
    expect(gridOf(parseTable("| a | b |\n|---|---|\n| `x \\| y` | 2 |")).rows[1]).toEqual(["`x | y`", "2"])
  })

  it("fills the cells a short row leaves out", () => {
    expect(gridOf(parseTable("| a | b | c |\n|---|---|---|\n| 1 |")).rows[1]).toEqual(["1", "", ""])
  })

  it("changes only the text of the edited cell", () => {
    const source = "| name   | size |\n| ------ | ---- |\n| apple  | 3    |"
    expect(edit(source, 1, 0, "pear")).toBe("| name   | size |\n| ------ | ---- |\n| pear  | 3    |")
  })

  it("writes into an empty cell", () => {
    expect(edit("| a | b |\n|---|---|\n|  | 2 |", 1, 0, "x")).toBe("| a | b |\n|---|---|\n| x | 2 |")
  })

  it("adds the cells a short row needs", () => {
    expect(edit("| a | b | c |\n|---|---|---|\n| 1 |", 1, 2, "x")).toBe("| a | b | c |\n|---|---|---|\n| 1 | | x |")
    expect(edit("a | b | c\n--|--|--\n1", 1, 2, "x")).toBe("a | b | c\n--|--|--\n1 | | x")
  })

  it("escapes a pipe and joins lines in the text of a cell", () => {
    expect(edit("| a |\n|---|\n| 1 |", 1, 0, "x | y\nz ")).toBe("| a |\n|---|\n| x \\| y z |")
  })

  it("writes a reshaped table with aligned columns", () => {
    const grid = gridOf(parseTable("| a | b |\n|---|---|\n| long text | 2 |"))
    const reshaped = alignColumn(insertColumn(insertRow(grid, 2), 1), 2, "right")
    expect(formatTable(reshaped, "")).toBe(
      [
        "| a         |     |   b |",
        "| --------- | --- | --: |",
        "| long text |     |   2 |",
        "|           |     |     |",
      ].join("\n"),
    )
  })

  it("deletes a row and a column", () => {
    const grid = gridOf(parseTable("| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |"))
    expect(deleteColumn(deleteRow(grid, 1), 0)).toEqual({ rows: [["b"], ["4"]], align: [null] })
  })

  it("keeps the indent of a table in a list item", () => {
    const table = parseTable("| a |\n  |---|\n  | 1 |")
    expect(gridOf(table).rows).toEqual([["a"], ["1"]])
    expect(formatTable(gridOf(table), table.indent)).toBe("| a   |\n  | --- |\n  | 1   |")
  })

  it("keeps the quote marks of a quoted table", () => {
    const table = parseTable("| a |\n> |---|\n> | 1 |")
    expect(gridOf(table).rows).toEqual([["a"], ["1"]])
    expect(formatTable(insertRow(gridOf(table), 2), table.indent)).toBe("| a   |\n> | --- |\n> | 1   |\n> |     |")
  })
})

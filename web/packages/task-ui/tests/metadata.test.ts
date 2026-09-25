import { describe, expect, it } from "vitest"

import type { FrontmatterFields, Metadata } from "@openplan/api-client"

import {
  conflictedFields,
  createdOf,
  dependenciesOf,
  fieldConflict,
  fieldFailure,
  fieldValue,
  parentOf,
  problems,
  statusField,
  tagsOf,
} from "../src/metadata"

const fields = (over: Partial<FrontmatterFields> = {}): Metadata => ({
  status: "todo",
  created: "2026-07-31T14:08:22Z",
  parent: null,
  rank: null,
  dependencies: [],
  tags: [],
  ...over,
})

const statusConflict = {
  kind: "conflict",
  value: "done",
  sides: [
    { label: "Ann (a1b2c3d)", value: "in_progress" },
    { label: "Ben (e4f5a6b)", value: "done" },
  ],
} as const

describe("a field", () => {
  it("reads a bare value as itself", () => {
    expect(fieldValue("todo")).toBe("todo")
    expect(fieldValue(null)).toBe(null)
    expect(fieldValue(["OPP-1"])).toEqual(["OPP-1"])
    expect(fieldFailure("todo")).toBeUndefined()
    expect(fieldConflict("todo")).toBeUndefined()
  })

  it("reads a failure as no value", () => {
    const failure = { kind: "invalid", message: "not a status" } as const
    expect(fieldValue(failure)).toBeUndefined()
    expect(fieldFailure(failure)).toBe(failure)
    expect(fieldFailure({ kind: "missing" })).toEqual({ kind: "missing" })
    expect(fieldConflict(failure)).toBeUndefined()
  })

  it("reads a conflict as the version in force, and not as a failure", () => {
    expect(fieldValue(statusConflict)).toBe("done")
    expect(fieldFailure(statusConflict)).toBeUndefined()
    expect(fieldConflict(statusConflict)).toBe(statusConflict)
  })

  it("reads a conflict over nothing as nothing, which is still a value", () => {
    const parent = {
      kind: "conflict",
      value: null,
      sides: [
        { label: "Ann", value: "OPP-3" },
        { label: "Ben", value: null },
      ],
    } as const
    expect(fieldValue(parent)).toBe(null)
    expect(parentOf(fields({ parent }))).toBeUndefined()
  })
})

describe("the metadata readers", () => {
  it("read every conflicted field as the version in force", () => {
    const metadata = fields({
      status: statusConflict,
      created: {
        kind: "conflict",
        value: "2026-08-01T00:00:00Z",
        sides: [
          { label: "Ann", value: "2026-07-01T00:00:00Z" },
          { label: "Ben", value: "2026-08-01T00:00:00Z" },
        ],
      },
      parent: {
        kind: "conflict",
        value: "OPP-2",
        sides: [
          { label: "Ann", value: null },
          { label: "Ben", value: "OPP-2" },
        ],
      },
      dependencies: {
        kind: "conflict",
        value: ["OPP-4"],
        sides: [
          { label: "Ann", value: [] },
          { label: "Ben", value: ["OPP-4"] },
        ],
      },
      tags: {
        kind: "conflict",
        value: ["ui"],
        sides: [
          { label: "Ann", value: ["cli"] },
          { label: "Ben", value: ["ui"] },
        ],
      },
    })
    expect(statusField(metadata)).toBe(statusConflict)
    expect(createdOf(metadata)).toBe("2026-08-01T00:00:00Z")
    expect(parentOf(metadata)).toBe("OPP-2")
    expect(dependenciesOf(metadata)).toEqual(["OPP-4"])
    expect(tagsOf(metadata)).toEqual(["ui"])
  })
})

describe("conflictedFields", () => {
  it("names the fields in conflict, in frontmatter order", () => {
    const rank = {
      kind: "conflict",
      value: "m",
      sides: [
        { label: "Ann", value: "a" },
        { label: "Ben", value: "m" },
      ],
    } as const
    expect(conflictedFields(fields({ rank, status: statusConflict }))).toEqual(["status", "rank"])
  })

  it("names nothing for a task without conflicts, or with frontmatter that did not parse", () => {
    expect(conflictedFields(fields({ status: { kind: "missing" } }))).toEqual([])
    expect(conflictedFields({ kind: "error", message: "no frontmatter" })).toEqual([])
  })
})

describe("problems", () => {
  it("lists the failed fields and leaves the conflicts out", () => {
    expect(problems(fields({ status: statusConflict, tags: { kind: "invalid", message: "not a list" } }))).toEqual([
      { field: "tags", message: "not a list" },
    ])
  })
})

import type { Field_Status } from "@openplan/api-client"

import type { ReadRefs } from "../src/ref-reader"

// A store that holds these tasks and docs, and answers at once.
export const holding = (tasks: Readonly<Record<string, Field_Status>>, docs: ReadonlyArray<string> = []): ReadRefs => ({
  useTask: (_project, id) => (id in tasks ? { status: tasks[id] } : "gone"),
  useDoc: (_project, name) => (docs.includes(name) ? "found" : "gone"),
})

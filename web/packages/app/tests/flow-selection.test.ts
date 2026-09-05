import { describe, expect, it } from "vitest"

import {
  describeSelection,
  EVERY_TASK,
  readSelection,
  selectionParams,
  selectsEveryTask,
  taskFlowPath,
} from "../src/lib/flow-selection"

describe("the selection in the URL", () => {
  it("keeps every repeat of a name, because the values of one name are alternatives", () => {
    const selection = readSelection(new URLSearchParams("project=open-plan&status=todo&status=in_progress&tag=api"))
    expect(selection).toEqual({
      projects: ["open-plan"],
      statuses: ["todo", "in_progress"],
      tasks: [],
      tags: ["api"],
    })
  })

  it("survives a round trip through the query string", () => {
    const selection = {
      projects: ["open-plan", "web"],
      statuses: ["todo"],
      tasks: ["OPP-42"],
      tags: ["api", "ui"],
    }
    expect(readSelection(selectionParams(selection))).toEqual(selection)
  })

  it("reads an empty query as every task", () => {
    const selection = readSelection(new URLSearchParams(""))
    expect(selection).toEqual(EVERY_TASK)
    expect(selectsEveryTask(selection)).toBe(true)
    expect(describeSelection(selection)).toBe("Every task that is not finished")
  })

  it("sends a named task with the project that spells its key", () => {
    expect(taskFlowPath("open-plan", "OPP-42")).toBe("/flow?project=open-plan&task=OPP-42")
    expect(selectsEveryTask(readSelection(new URLSearchParams("project=open-plan&task=OPP-42")))).toBe(false)
  })
})

import { taskPath } from "@open-planner/task-ui"

import type { Binding } from "./types"

export const bindings: ReadonlyArray<Binding> = [
  {
    id: "rows.down",
    keys: "j",
    scope: "rows",
    label: "Move down",
    group: "Navigation",
    run: (ctx) => ctx.cursor.moveBy(1),
  },
  {
    id: "rows.up",
    keys: "k",
    scope: "rows",
    label: "Move up",
    group: "Navigation",
    run: (ctx) => ctx.cursor.moveBy(-1),
  },
  {
    id: "rows.open",
    keys: "Enter",
    scope: "rows",
    label: "Open task",
    group: "Navigation",
    run: (ctx) => {
      const id = ctx.cursor.focusedId()
      if (id !== undefined) ctx.navigate(taskPath(id))
    },
  },
  {
    id: "detail.back",
    keys: "Escape",
    scope: "detail",
    label: "Back",
    group: "Navigation",
    run: (ctx) => ctx.detail.escape(),
  },
  {
    id: "go.list",
    keys: ["g", "l"],
    scope: "global",
    label: "Go to list",
    group: "Navigation",
    run: (ctx) => ctx.navigate("/"),
  },
  {
    id: "go.parent",
    keys: ["g", "p"],
    scope: "detail",
    label: "Go to parent",
    group: "Navigation",
    run: (ctx) => ctx.detail.goToParent(),
  },
  {
    id: "detail.parent",
    keys: "p",
    scope: "detail",
    label: "Set / change parent",
    group: "Task",
    run: (ctx) => ctx.detail.editParent(),
  },
  {
    id: "detail.subtask",
    keys: "s",
    scope: "detail",
    label: "Add subtask",
    group: "Task",
    run: (ctx) => ctx.detail.addSubtask(),
  },
  {
    id: "task.copy-id",
    keys: "mod+.",
    scope: "global",
    label: "Copy task id",
    group: "Task",
    run: (ctx) => ctx.copy.taskId(),
  },
  {
    id: "help.toggle",
    keys: "?",
    scope: "global",
    label: "Toggle this help",
    group: "Help",
    run: (ctx) => ctx.overlay.toggle(),
  },
  {
    id: "help.close.escape",
    keys: "Escape",
    scope: "overlay",
    label: "Close help",
    group: "Help",
    run: (ctx) => ctx.overlay.close(),
  },
  {
    id: "help.close.question",
    keys: "?",
    scope: "overlay",
    label: "Close help",
    group: "Help",
    run: (ctx) => ctx.overlay.close(),
  },
]

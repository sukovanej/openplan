import type { Binding } from "./types"

export const bindings: ReadonlyArray<Binding> = [
  {
    id: "list.down",
    keys: "j",
    scope: "list",
    label: "Move down",
    group: "Navigation",
    run: (ctx) => ctx.cursor.moveBy(1),
  },
  {
    id: "list.up",
    keys: "k",
    scope: "list",
    label: "Move up",
    group: "Navigation",
    run: (ctx) => ctx.cursor.moveBy(-1),
  },
  {
    id: "list.open",
    keys: "Enter",
    scope: "list",
    label: "Open task",
    group: "Navigation",
    run: (ctx) => {
      const id = ctx.cursor.focusedId()
      if (id !== undefined) ctx.navigate(`/task/${id}`)
    },
  },
  {
    id: "detail.back",
    keys: "Escape",
    scope: "detail",
    label: "Back to list",
    group: "Navigation",
    run: (ctx) => ctx.navigate("/"),
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

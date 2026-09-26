import {
  autocompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
  startCompletion,
} from "@codemirror/autocomplete"
import { EditorSelection, type Extension } from "@codemirror/state"
import type { EditorView } from "@codemirror/view"

import type { TaskRef } from "@openplan/api-client"

import { isInCode } from "./syntax"

export interface TaskOption {
  readonly task: TaskRef
  readonly indices: ReadonlyArray<number>
}

export interface TaskSearch {
  readonly search: (query: string) => ReadonlyArray<TaskOption>
  readonly picked: (task: TaskRef) => void
}

// `[[` anywhere, or `@` where a word starts: an `@` inside a word is an address.
const TASK_TRIGGER = /(?:\[\[|(?<=^|[\s(])@)[^\]\n@]*$/

const taskReferences =
  (search: TaskSearch) =>
  (context: CompletionContext): CompletionResult | null => {
    const typed = context.matchBefore(TASK_TRIGGER)
    if (typed === null || isInCode(context.state, context.pos, -1)) return null
    const query = typed.text.replace(/^(?:\[\[|@)/, "").trim()
    return {
      from: typed.from,
      filter: false,
      options: search.search(query).map(({ task }) => ({
        label: task.title,
        detail: task.id,
        type: "task",
        apply: (view: EditorView, _completion: Completion, from: number, to: number) => {
          search.picked(task)
          const closing = view.state.sliceDoc(to, to + 2) === "]]" ? 2 : 0
          const insert = `[[${task.id}]] `
          view.dispatch({
            changes: { from, to: to + closing, insert },
            selection: { anchor: from + insert.length },
          })
        },
      })),
    }
  }

type LineEdit = (view: EditorView, from: number, to: number) => void

// Puts `prefix` at the start of the line the command was typed on, in place of the `/command`.
const prefixLine =
  (prefix: string): LineEdit =>
  (view, from, to) => {
    const line = view.state.doc.lineAt(from)
    const changes = view.state.changes([
      { from, to, insert: "" },
      { from: line.from, insert: prefix },
    ])
    view.dispatch({ changes, selection: { anchor: changes.mapPos(to, 1) } })
  }

// Replaces the `/command` with `text`, and puts the caret `caret` characters into it.
const insertBlock =
  (text: string, caret: number): LineEdit =>
  (view, from, to) => {
    view.dispatch({ changes: { from, to, insert: text }, selection: EditorSelection.cursor(from + caret) })
  }

interface SlashCommand {
  readonly label: string
  readonly detail: string
  readonly run: LineEdit
}

const SLASH_COMMANDS: ReadonlyArray<SlashCommand> = [
  { label: "Heading 2", detail: "##", run: prefixLine("## ") },
  { label: "Heading 3", detail: "###", run: prefixLine("### ") },
  { label: "Heading 4", detail: "####", run: prefixLine("#### ") },
  { label: "Bulleted list", detail: "-", run: prefixLine("- ") },
  { label: "Numbered list", detail: "1.", run: prefixLine("1. ") },
  { label: "Checklist", detail: "- [ ]", run: prefixLine("- [ ] ") },
  { label: "Quote", detail: ">", run: prefixLine("> ") },
  { label: "Code block", detail: "```", run: insertBlock("```\n\n```", 4) },
  { label: "Diagram", detail: "mermaid", run: insertBlock("```mermaid\ngraph TD\n  A --> B\n```", 19) },
  { label: "Table", detail: "|", run: insertBlock("| Column | Column |\n| --- | --- |\n|  |  |", 36) },
  { label: "Divider", detail: "---", run: insertBlock("---\n", 4) },
  {
    label: "Task reference",
    detail: "[[",
    run: (view, from, to) => {
      insertBlock("[[", 2)(view, from, to)
      startCompletion(view)
    },
  },
]

function slashCommands(context: CompletionContext): CompletionResult | null {
  const typed = context.matchBefore(/(?<=^|\s)\/\w*$/)
  if (typed === null || isInCode(context.state, context.pos, -1)) return null
  // The words after the slash are what the list filters on; the slash goes with the command.
  return {
    from: typed.from + 1,
    options: SLASH_COMMANDS.map((command) => ({
      label: command.label,
      detail: command.detail,
      type: "command",
      apply: (view: EditorView, _completion: Completion, from: number, to: number) => command.run(view, from - 1, to),
    })),
    validFor: /^\w*$/,
  }
}

export function completions(search: TaskSearch): Extension {
  return autocompletion({
    override: [taskReferences(search), slashCommands],
    icons: false,
    activateOnTyping: true,
  })
}

import { syntaxTree } from "@codemirror/language"
import { type EditorState, type Extension, StateEffect, StateField } from "@codemirror/state"
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view"
import type { SyntaxNode } from "@lezer/common"

export interface TextStart {
  readonly line: number
  readonly text: number
  readonly indent?: Indent
}

// The leading whitespace of a list line, which reaches the column of the text at `column`.
export interface Indent {
  readonly to: number
  readonly column: number
}

interface Hang {
  readonly line: number
  readonly width?: number
  readonly indent?: { readonly to: number; readonly width: number }
}

function firstTextOf(state: EditorState, item: SyntaxNode): number | null {
  const body = item.getChild("Paragraph") ?? item.getChild("Task")
  if (body === null) return null
  const marker = body.getChild("TaskMarker")
  return marker === null ? body.from : marker.to + (state.sliceDoc(marker.to, marker.to + 1) === " " ? 1 : 0)
}

// The text of a list item starts after its marker, a checkbox, and the indent of a continuation line.
// A nested item starts under the text of its parent, and a continuation line under the text of its own item.
export function listTextStarts(state: EditorState, from: number, to: number): TextStart[] {
  const starts: TextStart[] = []
  syntaxTree(state).iterate({
    from,
    to,
    enter: (node) => {
      const item = node.node.parent
      if ((node.name !== "Paragraph" && node.name !== "Task") || item?.name !== "ListItem") return
      const first = firstTextOf(state, item)
      if (first === null) return false
      const outer = item.parent?.parent
      const parentText = outer?.name === "ListItem" ? firstTextOf(state, outer) : null
      const doc = state.doc
      for (let at = doc.lineAt(node.from).number; at <= doc.lineAt(node.to).number; at++) {
        const line = doc.line(at)
        const lead = /^\s*/.exec(line.text)?.[0].length ?? 0
        if (line.from <= node.from) {
          const nested = parentText !== null && line.from < item.from && lead === item.from - line.from
          starts.push({
            line: line.from,
            text: first,
            ...(nested ? { indent: { to: item.from, column: parentText } } : {}),
          })
        } else {
          starts.push({
            line: line.from,
            text: line.from + lead,
            ...(lead > 0 ? { indent: { to: line.from + lead, column: first } } : {}),
          })
        }
      }
      return false
    },
  })
  return starts
}

function measure(view: EditorView): Hang[] {
  const { from, to } = view.viewport
  const offset = (line: number, at: number): number | undefined => {
    const lineLeft = view.coordsAtPos(line, 1)?.left
    const left = view.coordsAtPos(at, 1)?.left
    if (lineLeft === undefined || left === undefined) return undefined
    return Math.round(((left - lineLeft) / view.scaleX) * 100) / 100
  }
  return listTextStarts(view.state, from, to).flatMap(({ line, text, indent }) => {
    const hang = offset(line, text)
    const column = indent === undefined ? undefined : offset(line, indent.column)
    const width = hang !== undefined && hang > 0 ? hang : undefined
    const measured =
      indent !== undefined && column !== undefined && column > 0 ? { to: indent.to, width: column } : undefined
    return width === undefined && measured === undefined ? [] : [{ line, width, indent: measured }]
  })
}

const setHangs = StateEffect.define<DecorationSet>()

const hangs = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update: (value, tr) =>
    tr.effects.reduce((current, effect) => (effect.is(setHangs) ? effect.value : current), value.map(tr.changes)),
  provide: (field) => EditorView.decorations.from(field),
})

const indentMark = Decoration.mark({ class: "cm-list-indent" })

function decorate(measured: readonly Hang[]): DecorationSet {
  return Decoration.set(
    measured.flatMap(({ line, width, indent }) => {
      const style = [
        width === undefined ? "" : `--hang: ${width}px;`,
        indent === undefined ? "" : `--indent: ${indent.width}px;`,
      ].join(" ")
      const lineDecoration = Decoration.line({ class: width === undefined ? "" : "cm-hang", attributes: { style } })
      return indent === undefined
        ? [lineDecoration.range(line)]
        : [lineDecoration.range(line), indentMark.range(line, indent.to)]
    }),
    true,
  )
}

function keyOf(set: DecorationSet): string {
  const parts: string[] = []
  set.between(0, Number.MAX_SAFE_INTEGER, (from, to, decoration) => {
    parts.push(`${from}-${to}:${(decoration.spec as { attributes?: { style: string } }).attributes?.style ?? ""}`)
  })
  return parts.join(",")
}

// A line's text start is known only after layout, and a measure callback may not dispatch.
const measurer = ViewPlugin.fromClass(
  class {
    private destroyed = false

    constructor(view: EditorView) {
      this.schedule(view)
    }

    update(update: ViewUpdate): void {
      // Any transaction can reveal or hide a marker and move where the text starts.
      if (update.transactions.length > 0 || update.viewportChanged || update.geometryChanged) {
        this.schedule(update.view)
      }
    }

    destroy(): void {
      this.destroyed = true
    }

    private schedule(view: EditorView): void {
      view.requestMeasure({
        key: this,
        read: measure,
        write: (measured) => {
          const next = decorate(measured)
          if (keyOf(next) === keyOf(view.state.field(hangs))) return
          queueMicrotask(() => {
            if (!this.destroyed) view.dispatch({ effects: setHangs.of(next) })
          })
        },
      })
    }
  },
)

export const hangingIndent: Extension = [hangs, measurer]

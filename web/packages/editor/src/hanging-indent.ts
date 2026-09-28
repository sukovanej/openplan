import { syntaxTree } from "@codemirror/language"
import { type EditorState, type Extension, RangeSetBuilder, StateEffect, StateField } from "@codemirror/state"
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view"

export interface TextStart {
  readonly line: number
  readonly text: number
}

interface Hang {
  readonly line: number
  readonly width: number
}

// The text of a list item starts after its marker, a checkbox, and the indent of a continuation line.
export function listTextStarts(state: EditorState, from: number, to: number): TextStart[] {
  const starts: TextStart[] = []
  syntaxTree(state).iterate({
    from,
    to,
    enter: (node) => {
      if ((node.name !== "Paragraph" && node.name !== "Task") || node.node.parent?.name !== "ListItem") return
      const marker = node.node.getChild("TaskMarker")
      const first = marker === null ? node.from : marker.to + (state.sliceDoc(marker.to, marker.to + 1) === " " ? 1 : 0)
      const doc = state.doc
      for (let at = doc.lineAt(node.from).number; at <= doc.lineAt(node.to).number; at++) {
        const line = doc.line(at)
        const text = line.from <= node.from ? first : line.from + (/^\s*/.exec(line.text)?.[0].length ?? 0)
        starts.push({ line: line.from, text })
      }
      return false
    },
  })
  return starts
}

function measure(view: EditorView): Hang[] {
  const { from, to } = view.viewport
  return listTextStarts(view.state, from, to).flatMap(({ line, text }) => {
    const lineLeft = view.coordsAtPos(line, 1)?.left
    const textLeft = view.coordsAtPos(text, 1)?.left
    if (lineLeft === undefined || textLeft === undefined) return []
    const width = Math.round(((textLeft - lineLeft) / view.scaleX) * 100) / 100
    return width > 0 ? [{ line, width }] : []
  })
}

const setHangs = StateEffect.define<DecorationSet>()

const hangs = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update: (value, tr) =>
    tr.effects.reduce((current, effect) => (effect.is(setHangs) ? effect.value : current), value.map(tr.changes)),
  provide: (field) => EditorView.decorations.from(field),
})

function decorate(measured: readonly Hang[]): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>()
  for (const { line, width } of measured) {
    builder.add(line, line, Decoration.line({ class: "cm-hang", attributes: { style: `--hang: ${width}px` } }))
  }
  return builder.finish()
}

function keyOf(set: DecorationSet): string {
  const parts: string[] = []
  set.between(0, Number.MAX_SAFE_INTEGER, (from, _to, decoration) => {
    parts.push(`${from}:${(decoration.spec as { attributes: { style: string } }).attributes.style}`)
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

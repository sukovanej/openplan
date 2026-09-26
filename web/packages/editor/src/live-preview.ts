import { ensureSyntaxTree, syntaxTree } from "@codemirror/language"
import { type EditorState, type Extension, type Range, StateEffect, StateField, type Text } from "@codemirror/state"
import { Decoration, type DecorationSet, EditorView, ViewPlugin } from "@codemirror/view"
import type { SyntaxNode } from "@lezer/common"

import {
  bodySegments,
  ensureHighlighter,
  highlightTokens,
  isDiagramTag,
  referenced,
  resolveLang,
  taskRefMatches,
  watchHighlighter,
} from "@openplan/task-ui"

import { isInCode } from "./syntax"
import {
  BulletWidget,
  CheckboxWidget,
  ConflictWidget,
  DiagramWidget,
  ImageWidget,
  MarkdownBlockWidget,
  RuleWidget,
  TaskRefWidget,
} from "./widgets"

const setFocused = StateEffect.define<boolean>()
const refresh = StateEffect.define<null>()

const hidden = Decoration.replace({})
const mark = (className: string, attributes?: Record<string, string>) =>
  Decoration.mark({ class: className, attributes })
const line = (className: string) => Decoration.line({ class: className })

const INLINE_STYLES: Readonly<Record<string, string>> = {
  Emphasis: "cm-em",
  StrongEmphasis: "cm-strong",
  Strikethrough: "cm-strike",
  InlineCode: "cm-inline-code",
}

// Which constructs each kind of mark opens and closes.
const INLINE_MARKS: Readonly<Record<string, ReadonlyArray<string>>> = {
  EmphasisMark: ["Emphasis", "StrongEmphasis"],
  StrikethroughMark: ["Strikethrough"],
  CodeMark: ["InlineCode"],
}

export interface Preview {
  readonly decorations: DecorationSet
  readonly atomic: DecorationSet
}

interface Span {
  readonly from: number
  readonly to: number
}

function conflictSpans(doc: Text): Span[] {
  const spans: Span[] = []
  let offset = 0
  for (const segment of bodySegments(doc.toString())) {
    const length = segment.kind === "text" ? segment.text.length : segment.block.length
    if (segment.kind === "conflict") spans.push({ from: offset, to: offset + length })
    offset += length
  }
  return spans
}

class Builder {
  readonly out: Range<Decoration>[] = []
  readonly atomic: Range<Decoration>[] = []
  private readonly taken: Span[] = []

  constructor(
    private readonly state: EditorState,
    private readonly focused: boolean,
  ) {}

  // The source shows where the reader is: a construct the selection touches, or a line it is on.
  touches(from: number, to: number): boolean {
    return this.focused && this.state.selection.ranges.some((range) => range.from <= to && range.to >= from)
  }

  lineTouched(at: number): boolean {
    const { from, to } = this.state.doc.lineAt(at)
    return this.touches(from, to)
  }

  isTaken(from: number, to: number): boolean {
    return this.taken.some((span) => span.from < to && span.to > from)
  }

  isInside(from: number, to: number): boolean {
    return this.taken.some((span) => span.from <= from && span.to >= to)
  }

  replace(from: number, to: number, decoration: Decoration = hidden): void {
    if (from >= to || this.isTaken(from, to)) return
    this.taken.push({ from, to })
    this.out.push(decoration.range(from, to))
  }

  add(from: number, to: number, decoration: Decoration): void {
    if (from < to) this.out.push(decoration.range(from, to))
  }

  lines(from: number, to: number, className: string): void {
    const doc = this.state.doc
    for (let at = doc.lineAt(from).number; at <= doc.lineAt(to).number; at++) {
      const current = doc.line(at)
      this.out.push(line(className).range(current.from))
    }
  }

  // A mark and the one space after it go together, as `## ` or `> `.
  hideMark(node: SyntaxNode): void {
    const after = this.state.sliceDoc(node.to, node.to + 1) === " " ? 1 : 0
    this.replace(node.from, node.to + after)
  }
}

function fenced(builder: Builder, state: EditorState, node: SyntaxNode): boolean {
  const info = node.getChild("CodeInfo")
  const language = info === null ? "" : state.sliceDoc(info.from, info.to).trim()
  const text = node.getChild("CodeText")
  const open = !builder.touches(node.from, node.to)
  const source = text === null ? "" : state.sliceDoc(text.from, text.to)
  if (open && isDiagramTag(language) && source.trim() !== "") {
    builder.replace(node.from, node.to, Decoration.replace({ widget: new DiagramWidget(source), block: true }))
    return false
  }
  builder.lines(node.from, node.to, "cm-code-line")
  const first = state.doc.lineAt(node.from)
  const last = state.doc.lineAt(node.to)
  builder.out.push(line("cm-code-first").range(first.from))
  builder.out.push(line("cm-code-last").range(last.from))
  if (open) {
    builder.out.push(line("cm-fence-hidden").range(first.from))
    if (last.number > first.number && /^\s*(`{3,}|~{3,})\s*$/.test(last.text)) {
      builder.out.push(line("cm-fence-hidden").range(last.from))
    }
  }
  const lang = resolveLang(language || undefined)
  if (lang !== null && text !== null) {
    const tokens = highlightTokens(source, lang)
    if (tokens === null) void ensureHighlighter(lang)
    for (const token of tokens ?? []) {
      const from = text.from + token.offset
      builder.add(from, from + token.length, Decoration.mark({ attributes: { style: token.style } }))
    }
  }
  return false
}

function link(builder: Builder, state: EditorState, node: SyntaxNode): void {
  const marks = node.getChildren("LinkMark")
  const url = node.getChild("URL")
  if (url === null || marks.length < 2 || builder.isTaken(node.from, node.to)) return
  const textFrom = marks[0].to
  const textTo = marks[1].from
  const href = state.sliceDoc(url.from, url.to)
  if (builder.touches(node.from, node.to)) {
    builder.add(textFrom, textTo, mark("cm-link"))
    return
  }
  builder.replace(node.from, textFrom)
  builder.add(textFrom, textTo, mark("cm-link", { "data-href": href }))
  builder.replace(textTo, node.to)
}

function listMark(builder: Builder, state: EditorState, node: SyntaxNode): void {
  const item = node.parent
  const marker = item?.getChild("Task")?.getChild("TaskMarker")
  if (marker !== undefined && marker !== null) {
    const checked = /x/i.test(state.sliceDoc(marker.from, marker.to))
    const end = state.sliceDoc(marker.to, marker.to + 1) === " " ? marker.to + 1 : marker.to
    if (checked) builder.add(end, state.doc.lineAt(end).to, mark("cm-task-done"))
    if (!builder.touches(node.from, marker.to)) {
      builder.replace(node.from, end, Decoration.replace({ widget: new CheckboxWidget(checked) }))
    }
    return
  }
  if (item?.parent?.name === "BulletList") {
    if (!builder.touches(node.from, node.to + 1)) {
      builder.replace(node.from, node.to, Decoration.replace({ widget: new BulletWidget() }))
    }
    return
  }
  builder.add(node.from, node.to, mark("cm-list-number"))
}

function taskRefs(builder: Builder, state: EditorState, abbreviation: string): void {
  for (const match of taskRefMatches(state.doc.toString())) {
    const from = match.index
    const to = from + match[0].length
    if (referenced(match[1], abbreviation) === null || isInCode(state, from, 1)) continue
    if (builder.touches(from, to)) builder.add(from, to, mark("cm-ref-source"))
    else builder.replace(from, to, Decoration.replace({ widget: new TaskRefWidget(match[1]) }))
  }
}

export function previewOf(state: EditorState, focused: boolean, abbreviation: string): Preview {
  const builder = new Builder(state, focused)
  for (const span of conflictSpans(state.doc)) {
    const to = state.doc.lineAt(Math.max(span.from, span.to - 1)).to
    const text = state.sliceDoc(span.from, span.to)
    builder.replace(span.from, to, Decoration.replace({ widget: new ConflictWidget(text), block: true }))
    builder.atomic.push(Decoration.mark({}).range(span.from, to))
  }
  // Task references go before links: `[[DEM-1]]` also reads as a link to nobody.
  taskRefs(builder, state, abbreviation)
  const tree = ensureSyntaxTree(state, state.doc.length, 200) ?? syntaxTree(state)
  tree.iterate({
    enter: (ref) => {
      const node = ref.node
      if (node.name !== "Document" && builder.isInside(node.from, node.to)) return false
      const name = node.name
      const heading = /^(?:ATX|Setext)Heading(\d)$/.exec(name)
      if (heading !== null) {
        builder.out.push(line(`cm-h${heading[1]}`).range(state.doc.lineAt(node.from).from))
        return true
      }
      if (name in INLINE_STYLES) {
        builder.add(node.from, node.to, mark(INLINE_STYLES[name]))
        return true
      }
      switch (name) {
        case "HeaderMark":
          if (node.parent?.name.startsWith("ATXHeading") === true && !builder.lineTouched(node.from))
            builder.hideMark(node)
          return false
        case "EmphasisMark":
        case "StrikethroughMark":
        case "CodeMark": {
          const parent = node.parent
          if (parent !== null && INLINE_MARKS[name].includes(parent.name)) {
            if (!builder.touches(parent.from, parent.to)) builder.replace(node.from, node.to)
          }
          return false
        }
        case "Link":
          link(builder, state, node)
          return true
        case "Autolink": {
          const url = node.getChild("URL")
          if (url === null) return false
          const href = state.sliceDoc(url.from, url.to)
          if (builder.touches(node.from, node.to)) {
            builder.add(url.from, url.to, mark("cm-link"))
          } else {
            builder.replace(node.from, url.from)
            builder.add(url.from, url.to, mark("cm-link", { "data-href": href }))
            builder.replace(url.to, node.to)
          }
          return false
        }
        case "URL": {
          if (node.parent?.name === "Link" || node.parent?.name === "Image") return false
          const href = state.sliceDoc(node.from, node.to)
          builder.add(
            node.from,
            node.to,
            mark("cm-link", builder.touches(node.from, node.to) ? undefined : { "data-href": href }),
          )
          return false
        }
        case "Image": {
          const url = node.getChild("URL")
          const marks = node.getChildren("LinkMark")
          if (url === null || marks.length < 2 || builder.touches(node.from, node.to)) return false
          const alt = state.sliceDoc(marks[0].to, marks[1].from)
          builder.replace(
            node.from,
            node.to,
            Decoration.replace({ widget: new ImageWidget(state.sliceDoc(url.from, url.to), alt) }),
          )
          return false
        }
        case "Blockquote":
          builder.lines(node.from, node.to, "cm-quote")
          return true
        case "QuoteMark":
          if (!builder.lineTouched(node.from)) builder.hideMark(node)
          return false
        case "ListMark":
          listMark(builder, state, node)
          return false
        case "HorizontalRule":
          if (!builder.lineTouched(node.from)) {
            builder.replace(node.from, node.to, Decoration.replace({ widget: new RuleWidget() }))
          }
          return false
        case "FencedCode":
          return fenced(builder, state, node)
        case "CodeBlock":
          builder.lines(node.from, node.to, "cm-code-line")
          return false
        case "Table":
          if (builder.touches(node.from, node.to)) {
            builder.lines(node.from, node.to, "cm-table-source")
          } else {
            const source = state.sliceDoc(node.from, node.to)
            builder.replace(
              node.from,
              node.to,
              Decoration.replace({ widget: new MarkdownBlockWidget(source), block: true }),
            )
          }
          return false
        default:
          return true
      }
    },
  })
  return { decorations: Decoration.set(builder.out, true), atomic: Decoration.set(builder.atomic, true) }
}

export function livePreview(abbreviation: string): Extension {
  const focused = StateField.define<boolean>({
    create: () => false,
    update: (value, tr) =>
      tr.effects.reduce((current, effect) => (effect.is(setFocused) ? effect.value : current), value),
  })

  const preview = StateField.define<Preview>({
    create: (state) => previewOf(state, false, abbreviation),
    update: (value, tr) => {
      const changed =
        tr.docChanged ||
        tr.selection !== undefined ||
        tr.effects.some((effect) => effect.is(setFocused) || effect.is(refresh)) ||
        syntaxTree(tr.startState) !== syntaxTree(tr.state)
      return changed ? previewOf(tr.state, tr.state.field(focused), abbreviation) : value
    },
    provide: (field) => [
      EditorView.decorations.from(field, (value) => value.decorations),
      EditorView.atomicRanges.of((view) => view.state.field(field).atomic),
    ],
  })

  // A grammar arrives after the first paint, and the blocks of its language colour in then.
  const highlighter = ViewPlugin.define((view) => {
    const stop = watchHighlighter(() => view.dispatch({ effects: refresh.of(null) }))
    return { destroy: stop }
  })

  return [
    focused,
    preview,
    highlighter,
    EditorView.domEventHandlers({
      focus: (_event, view) => view.dispatch({ effects: setFocused.of(true) }),
      blur: (_event, view) => view.dispatch({ effects: setFocused.of(false) }),
    }),
  ]
}

import { syntaxTree } from "@codemirror/language"
import { type EditorState, type Extension, Prec } from "@codemirror/state"
import { type Command, type EditorView, keymap, runScopeHandlers } from "@codemirror/view"
import type { SyntaxNode } from "@lezer/common"
import {
  BetweenHorizontalEnd,
  BetweenHorizontalStart,
  BetweenVerticalEnd,
  BetweenVerticalStart,
  Columns3,
  type LucideIcon,
  Rows3,
  TextAlignCenter,
  TextAlignEnd,
  TextAlignStart,
} from "lucide-react"
import { type KeyboardEvent, type ReactNode, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react"

import { TaskInline } from "@openplan/task-ui"
import { Button, cn, Tooltip } from "@openplan/ui"

import { useEditorScope } from "./portals"
import {
  type Align,
  alignColumn,
  cellChange,
  deleteColumn,
  deleteRow,
  formatTable,
  type Grid,
  gridOf,
  insertColumn,
  insertRow,
  parseTable,
  type Spot,
} from "./table"
import { EditButton, editAt, ReactWidget } from "./widgets"

type Caret = "start" | "end"

interface Active extends Spot {
  readonly caret: Caret
}

const oneLine = (text: string) => text.replace(/\r?\n/g, " ")

export function tableAt(state: EditorState, at: number): SyntaxNode | null {
  for (let node: SyntaxNode | null = syntaxTree(state).resolveInner(at, 1); node !== null; node = node.parent) {
    if (node.name === "Table") return node
  }
  return null
}

// A reference chip stands taller than a line of text. Every cell keeps its height, so a row keeps its
// height when one of its cells turns into a text area.
const CELL_HEIGHT = "min-h-[26px]"

type Edge = "first" | "last"

const entrances = new WeakMap<HTMLElement, (row: Edge) => void>()

// The caret passes over a rendered table, so an arrow key next to one moves into its cells instead.
const enterTable =
  (forward: boolean): Command =>
  (view) => {
    const range = view.state.selection.main
    if (!range.empty) return false
    const doc = view.state.doc
    const line = doc.lineAt(range.head)
    const target = view.moveVertically(range, forward)
    if (forward ? target.head <= line.to || line.to === doc.length : target.head >= line.from || line.from === 0) {
      return false
    }
    const table = tableAt(view.state, forward ? line.to + 1 : doc.lineAt(line.from - 1).from)
    if (table === null) return false
    for (const element of view.contentDOM.querySelectorAll<HTMLElement>(".cm-rendered-block")) {
      const enter = entrances.get(element)
      if (enter !== undefined && view.posAtDOM(element) === table.from) {
        enter(forward ? "first" : "last")
        return true
      }
    }
    return false
  }

export const tableKeys: Extension = Prec.high(
  keymap.of([
    { key: "ArrowDown", run: enterTable(true) },
    { key: "ArrowUp", run: enterTable(false) },
  ]),
)

function CellEditor({
  text,
  caret,
  label,
  onInput,
  onKeyDown,
}: {
  text: string
  caret: Caret
  label: string
  onInput: (text: string) => void
  onKeyDown: (event: KeyboardEvent<HTMLTextAreaElement>) => void
}) {
  const area = useRef<HTMLTextAreaElement>(null)
  const [draft, setDraft] = useState(text)
  const [source, setSource] = useState(text)
  // The source trims a cell, so the space the reader just typed lives only in the draft.
  if (text !== source) {
    setSource(text)
    if (oneLine(draft).trim() !== text) setDraft(text)
  }

  useLayoutEffect(() => {
    const node = area.current
    if (node === null) return
    node.focus()
    const at = caret === "start" ? 0 : node.value.length
    node.setSelectionRange(at, at)
  }, [caret])

  // The hidden copy sizes the grid cell, and the text area fills it: a text area has no size of its own.
  return (
    <div className={cn("grid", CELL_HEIGHT)}>
      <span aria-hidden className="invisible col-start-1 row-start-1 break-words whitespace-pre-wrap">
        {draft}
      </span>
      <textarea
        ref={area}
        rows={1}
        cols={1}
        value={draft}
        aria-label={label}
        className="col-start-1 row-start-1 m-0 w-full resize-none overflow-hidden border-0 bg-transparent p-0 [font:inherit] [color:inherit] [text-align:inherit] outline-none"
        onChange={(event) => {
          const value = oneLine(event.target.value)
          setDraft(value)
          onInput(value)
        }}
        onKeyDown={onKeyDown}
      />
    </div>
  )
}

function Tool({
  label,
  icon: Icon,
  pressed,
  disabled,
  onClick,
}: {
  label: string
  icon: LucideIcon
  pressed?: boolean
  disabled?: boolean
  onClick: () => void
}) {
  return (
    <Tooltip content={label}>
      <Button
        size="icon"
        aria-label={label}
        aria-pressed={pressed}
        disabled={disabled}
        onClick={onClick}
        className={cn(
          "disabled:pointer-events-none disabled:opacity-30",
          pressed === true && "bg-muted text-foreground",
        )}
      >
        <Icon className="size-4" />
      </Button>
    </Tooltip>
  )
}

const ALIGNS: ReadonlyArray<{ align: Align; label: string; icon: LucideIcon }> = [
  { align: "left", label: "Align the column left", icon: TextAlignStart },
  { align: "center", label: "Center the column", icon: TextAlignCenter },
  { align: "right", label: "Align the column right", icon: TextAlignEnd },
]

function TableView({ source, view, element }: { source: string; view: EditorView; element: HTMLElement }) {
  const { project, abbreviation, refs, docRefs } = useEditorScope()
  const grid = useMemo(() => gridOf(parseTable(source)), [source])
  const [active, setActive] = useState<Active | null>(null)
  const box = useRef<HTMLDivElement>(null)
  const columns = grid.align.length
  const last = grid.rows.length - 1

  // An edit above the table moves it, so each change finds it in the document as it is now.
  const locate = () => {
    const node = tableAt(view.state, view.posAtDOM(element))
    if (node === null) return null
    return { from: node.from, to: node.to, table: parseTable(view.state.sliceDoc(node.from, node.to)) }
  }

  const write = (spot: Spot, text: string) => {
    const at = locate()
    if (at === null) return
    const change = cellChange(at.table, spot, text)
    view.dispatch({
      changes: { from: at.from + change.from, to: at.from + change.to, insert: change.insert },
      userEvent: "input.type",
    })
  }

  const reshape = (change: (grid: Grid) => Grid, next: Active) => {
    const at = locate()
    if (at === null) return
    const insert = formatTable(change(gridOf(at.table)), at.table.indent)
    view.dispatch({ changes: { from: at.from, to: at.to, insert }, userEvent: "input" })
    setActive(next)
  }

  // The caret goes to the line next to the table. A table at the edge of the text gets a new line there.
  const leave = (direction: "up" | "down") => {
    const at = locate()
    if (at === null) return
    setActive(null)
    const doc = view.state.doc
    if (direction === "down") {
      const end = doc.lineAt(at.to).to
      view.dispatch(
        end < doc.length
          ? { selection: { anchor: end + 1 }, scrollIntoView: true }
          : { changes: { from: end, insert: "\n" }, selection: { anchor: end + 1 }, scrollIntoView: true },
      )
    } else {
      const start = doc.lineAt(at.from).from
      view.dispatch(
        start > 0
          ? { selection: { anchor: start - 1 }, scrollIntoView: true }
          : { changes: { from: 0, insert: "\n" }, selection: { anchor: 0 }, scrollIntoView: true },
      )
    }
    view.focus()
  }

  useEffect(() => {
    entrances.set(element, (row) => setActive({ row: row === "first" ? 0 : last, col: 0, caret: "end" }))
    return () => {
      entrances.delete(element)
    }
  }, [element, last])

  const goTo = (index: number, caret: Caret) =>
    setActive({ row: Math.floor(index / columns), col: index % columns, caret })

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>, spot: Spot) => {
    if (event.nativeEvent.isComposing) return
    // Save and undo belong to the whole text, so the editor's own keys answer them.
    if ((event.metaKey || event.ctrlKey) && /^[szy]$/i.test(event.key)) {
      if (runScopeHandlers(view, event.nativeEvent, "editor")) event.preventDefault()
      return
    }
    const area = event.currentTarget
    const collapsed = area.selectionStart === area.selectionEnd
    const index = spot.row * columns + spot.col
    const cells = grid.rows.length * columns
    switch (event.key) {
      case "Tab":
        event.preventDefault()
        if (event.shiftKey) {
          if (index > 0) goTo(index - 1, "end")
        } else if (index + 1 < cells) {
          goTo(index + 1, "end")
        } else {
          reshape((current) => insertRow(current, last + 1), { row: last + 1, col: 0, caret: "end" })
        }
        return
      case "Enter":
        event.preventDefault()
        if (event.shiftKey) return
        if (spot.row < last) setActive({ ...spot, row: spot.row + 1, caret: "end" })
        else reshape((current) => insertRow(current, last + 1), { ...spot, row: last + 1, caret: "end" })
        return
      case "ArrowUp":
        event.preventDefault()
        if (spot.row > 0) setActive({ ...spot, row: spot.row - 1, caret: "end" })
        else leave("up")
        return
      case "ArrowDown":
        event.preventDefault()
        if (spot.row < last) setActive({ ...spot, row: spot.row + 1, caret: "end" })
        else leave("down")
        return
      case "ArrowLeft":
        if (!event.shiftKey && collapsed && area.selectionStart === 0 && index > 0) {
          event.preventDefault()
          goTo(index - 1, "end")
        }
        return
      case "ArrowRight":
        if (!event.shiftKey && collapsed && area.selectionEnd === area.value.length && index + 1 < cells) {
          event.preventDefault()
          goTo(index + 1, "start")
        }
        return
      case "Escape":
        event.preventDefault()
        leave("down")
        return
    }
  }

  // Focus moves between the cells of the table, so only focus that leaves it ends the editing.
  const onBlur = () => {
    window.setTimeout(() => {
      if (box.current?.contains(document.activeElement) !== true) setActive(null)
    })
  }

  const cell = (Tag: "th" | "td", row: number, col: number, text: string): ReactNode => {
    const editing = active !== null && active.row === row && active.col === col
    return (
      <Tag
        key={col}
        style={{ textAlign: grid.align[col] ?? undefined }}
        className="cursor-text align-top"
        onMouseDown={
          editing
            ? undefined
            : (event) => {
                if (event.button !== 0 || (event.target as HTMLElement).closest("a") !== null) return
                event.preventDefault()
                setActive({ row, col, caret: "end" })
              }
        }
      >
        {editing ? (
          <CellEditor
            text={text}
            caret={active.caret}
            label={row === 0 ? `Header, column ${col + 1}` : `Row ${row}, column ${col + 1}`}
            onInput={(value) => write({ row, col }, value)}
            onKeyDown={(event) => onKeyDown(event, { row, col })}
          />
        ) : (
          <div className={CELL_HEIGHT}>
            <TaskInline project={project} abbreviation={abbreviation} refs={refs} docRefs={docRefs} markdown={text} />
          </div>
        )}
      </Tag>
    )
  }

  return (
    <div ref={box} className="group relative my-3" onBlur={onBlur}>
      {active !== null && (
        <div
          role="toolbar"
          aria-label="Table"
          className="bg-background absolute right-0 bottom-full z-10 mb-1 flex items-center gap-0.5 rounded-md border p-0.5 shadow-sm"
          onMouseDown={(event) => event.preventDefault()}
        >
          <Tool
            label="Insert a row above"
            icon={BetweenHorizontalStart}
            disabled={active.row === 0}
            onClick={() => reshape((current) => insertRow(current, active.row), { ...active, row: active.row + 1 })}
          />
          <Tool
            label="Insert a row below"
            icon={BetweenHorizontalEnd}
            onClick={() => reshape((current) => insertRow(current, active.row + 1), active)}
          />
          <Tool
            label="Delete the row"
            icon={Rows3}
            disabled={active.row === 0}
            onClick={() =>
              reshape((current) => deleteRow(current, active.row), { ...active, row: Math.min(active.row, last - 1) })
            }
          />
          <div className="bg-border mx-0.5 h-4 w-px" />
          <Tool
            label="Insert a column left"
            icon={BetweenVerticalStart}
            onClick={() => reshape((current) => insertColumn(current, active.col), { ...active, col: active.col + 1 })}
          />
          <Tool
            label="Insert a column right"
            icon={BetweenVerticalEnd}
            onClick={() => reshape((current) => insertColumn(current, active.col + 1), active)}
          />
          <Tool
            label="Delete the column"
            icon={Columns3}
            disabled={columns === 1}
            onClick={() =>
              reshape((current) => deleteColumn(current, active.col), {
                ...active,
                col: Math.min(active.col, columns - 2),
              })
            }
          />
          <div className="bg-border mx-0.5 h-4 w-px" />
          {ALIGNS.map(({ align, label, icon }) => {
            const pressed = grid.align[active.col] === align
            return (
              <Tool
                key={label}
                label={label}
                icon={icon}
                pressed={pressed}
                onClick={() => reshape((current) => alignColumn(current, active.col, pressed ? null : align), active)}
              />
            )
          })}
        </div>
      )}
      <div className="border-border overflow-x-auto rounded-lg border">
        <table className="my-0">
          <thead>
            <tr>{grid.rows[0].map((text, col) => cell("th", 0, col, text))}</tr>
          </thead>
          {last > 0 && (
            <tbody>
              {grid.rows.slice(1).map((row, index) => (
                <tr key={index}>{row.map((text, col) => cell("td", index + 1, col, text))}</tr>
              ))}
            </tbody>
          )}
        </table>
      </div>
      {active === null && <EditButton onEdit={() => editAt(view, element)} />}
    </div>
  )
}

export class TableWidget extends ReactWidget {
  protected readonly block = true

  constructor(private readonly source: string) {
    super()
  }

  eq(other: TableWidget): boolean {
    return other.source === this.source
  }

  // A cell edit changes the source. Keeping the element keeps the cell that has the focus.
  updateDOM(element: HTMLElement, view: EditorView): boolean {
    return this.remount(element, view)
  }

  protected render(view: EditorView, element: HTMLElement): ReactNode {
    return <TableView source={this.source} view={view} element={element} />
  }
}

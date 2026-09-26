import { Fragment, useEffect, useMemo, useRef, type MouseEvent, type ReactNode, type Ref } from "react"
import { useNavigate } from "react-router-dom"

import { cn, Panel, PanelBody, PanelHeader, PanelTitle, Row } from "@openplan/ui"

import { rowCursor, useRowCursor } from "../lib/row-cursor"
import { hoveredRow } from "../lib/row-target"
import { treeGuides, type RowGuides } from "../lib/tree-guides"

// One keyboard-walkable list of rows, which the task board and the docs list are both shapes of. The
// grid owns the cursor and the tree guides; a caller supplies the groups, a path per row, and a
// memoized row that renders a `GridRow`.
export interface RowGroup<T> {
  readonly key: string
  readonly label: string
  readonly header?: ReactNode
  readonly rows: ReadonlyArray<T>
}

// Every field keeps its identity while its row stays as it was, so a memoized row spread with it
// renders again only when the cursor reaches it or leaves it.
export interface RowSlot {
  readonly ref?: Ref<HTMLDivElement>
  readonly path: string
  readonly at: number
  readonly guides: RowGuides
  readonly active: boolean
  readonly last: boolean
}

const rowDomId = (path: string) => `grid-row-${path}`

const NO_GUIDES: RowGuides = { columns: [], opensChildren: false }

export function RowGrid<T>({
  label,
  title,
  action,
  groups,
  lead,
  pathOf,
  depthOf,
  children,
}: {
  label: string
  title: ReactNode
  action?: ReactNode
  groups: ReadonlyArray<RowGroup<T>>
  // Rendered above the first group, inside the scrolling body.
  lead?: ReactNode
  pathOf: (row: T) => string
  depthOf?: (row: T) => number
  children: (row: T, slot: RowSlot) => ReactNode
}) {
  const paths = useMemo(() => groups.flatMap((group) => group.rows.map(pathOf)), [groups, pathOf])
  const { index } = useRowCursor(paths)
  const guides = useMemo(
    () =>
      depthOf === undefined
        ? undefined
        : groups.map((group) => treeGuides(group.rows.map((row) => ({ depth: depthOf(row) })))),
    [groups, depthOf],
  )
  const activeId = index >= 0 && index < paths.length ? rowDomId(paths[index]) : undefined

  const activeRow = useRef<HTMLDivElement>(null)
  useEffect(() => {
    activeRow.current?.scrollIntoView({ block: "nearest" })
  }, [index])

  // Walking with the keyboard makes the grid the thing being driven, so it takes the focus. A link
  // clicked earlier would otherwise keep it, and Enter would follow that link instead of opening the
  // row.
  const grid = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const held = grid.current
    if (index < 0 || held === null || held.contains(document.activeElement)) return
    held.focus({ preventScroll: true })
  }, [index])

  let base = 0
  return (
    <Panel
      ref={grid}
      role="grid"
      aria-label={label}
      aria-activedescendant={activeId}
      tabIndex={0}
      className="text-sm focus:outline-none"
    >
      <PanelHeader className="gap-3">
        <PanelTitle>{title}</PanelTitle>
        {action !== undefined && <div className="ml-auto flex min-w-0 items-center">{action}</div>}
      </PanelHeader>
      {/* The pointer only marks a row while the keyboard cursor is idle, so the two never claim one
          at once. The rows read that from this attribute, and a cursor that starts or stops draws
          none of them again. */}
      <PanelBody onMouseLeave={hoveredRow.clear} data-pointer={index === -1 ? "free" : "held"}>
        {lead}
        {groups.map((group, groupIndex) => {
          const lastGroup = groupIndex === groups.length - 1
          return (
            <div key={group.key} role="rowgroup" aria-label={group.label}>
              {group.header}
              {group.rows.map((row, j) => {
                const at = base++
                return (
                  <Fragment key={paths[at]}>
                    {children(row, {
                      ref: at === index ? activeRow : undefined,
                      path: paths[at],
                      at,
                      guides: guides?.[groupIndex][j] ?? NO_GUIDES,
                      active: at === index,
                      last: lastGroup && j === group.rows.length - 1,
                    })}
                  </Fragment>
                )
              })}
            </div>
          )
        })}
      </PanelBody>
    </Panel>
  )
}

export function GridRow({
  slot,
  className,
  children,
}: {
  slot: Omit<RowSlot, "guides">
  className?: string
  children: ReactNode
}) {
  const { ref, path, at, active, last } = slot
  const navigate = useNavigate()

  // The row opens its page from its own click rather than from a link stretched over it: an overlay
  // that size takes every hover in the row with it, leaving the tooltips underneath unreachable. The
  // links the row does contain answer their own clicks, and a modified click is the browser's.
  const open = (event: MouseEvent<HTMLDivElement>) => {
    rowCursor.focus(at)
    const link = event.target instanceof Element && event.target.closest("a") !== null
    if (link || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
    navigate(path)
  }

  return (
    <Row
      ref={ref}
      id={rowDomId(path)}
      role="row"
      aria-selected={active}
      active={active}
      hoverable="while-free"
      last={last}
      onClick={open}
      // Scrolling a list under a still pointer moves `:hover` to another row without a mousemove,
      // so the row the pointer marks is only in step with the store if entering counts too. It must
      // not count while the keyboard drives, or walking with `j` would hand rows it scrolls past
      // back to the pointer.
      onMouseEnter={() => {
        if (rowCursor.getSnapshot().index === -1) hoveredRow.enter(path, at)
      }}
      // Moving the pointer is what hands the current row back to it — and only over a row, so a
      // nudge across a group header or the scrollbar leaves the keyboard's row where it was.
      onMouseMove={() => {
        hoveredRow.enter(path, at)
        rowCursor.clear()
      }}
      onMouseLeave={() => hoveredRow.leave(path, at)}
      className={cn("flex cursor-pointer items-start py-3", className)}
    >
      {children}
    </Row>
  )
}

// Tree lines run down the middle of the icon column they hang from — half an icon in from the indent
// that column occupies — and turn in at the row's own middle, where the icon they point at sits.
// Guides stretch to the row's content box, so each segment overhangs it by the row's padding to meet
// the segment in the row above or below: 12px up, and 12px plus the separator down.
const GUIDE_ROW_TOP = "-top-3"
const GUIDE_ROW_BOTTOM = "-bottom-[13px]"
const GUIDE_TO_MIDDLE = "h-[calc(50%+0.75rem)]"

function Guide({ className }: { className: string }) {
  return <span aria-hidden className={cn("border-muted-foreground/30 absolute left-[0.625rem]", className)} />
}

export function TreeGuides({ columns }: { columns: ReadonlyArray<boolean> }) {
  return (
    <div aria-hidden className="flex shrink-0 self-stretch pl-4">
      {columns.map((continues, column) =>
        column === columns.length - 1 ? (
          <div key={column} className="relative w-6">
            <Guide className={cn(GUIDE_ROW_TOP, GUIDE_TO_MIDDLE, "right-0 border-b border-l")} />
            {continues && <Guide className={cn(GUIDE_ROW_BOTTOM, "top-1/2 border-l")} />}
          </div>
        ) : (
          <div key={column} className="relative w-6">
            {continues && <Guide className={cn(GUIDE_ROW_TOP, GUIDE_ROW_BOTTOM, "border-l")} />}
          </div>
        ),
      )}
    </div>
  )
}

// The line that leaves a row's icon for its first child.
export function ChildGuide() {
  return <Guide className={cn(GUIDE_ROW_BOTTOM, "top-[calc(50%+0.625rem)] border-l")} />
}

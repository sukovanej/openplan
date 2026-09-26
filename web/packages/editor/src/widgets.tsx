import { type EditorView, WidgetType } from "@codemirror/view"
import { Pencil } from "lucide-react"
import type { ReactNode } from "react"

import {
  BodyConflict,
  bodySegments,
  type ConflictBlock,
  DiagramBlock,
  referencedTask,
  TaskBody,
  TaskRefChip,
  taskPath,
} from "@openplan/task-ui"
import { Button } from "@openplan/ui"

import { type Portals, portals, useEditorScope } from "./portals"

const owners = new WeakMap<HTMLElement, Portals>()

abstract class ReactWidget extends WidgetType {
  protected abstract readonly block: boolean
  protected abstract render(view: EditorView, element: HTMLElement): ReactNode

  toDOM(view: EditorView): HTMLElement {
    const element = document.createElement(this.block ? "div" : "span")
    element.className = this.block ? "cm-rendered-block" : "cm-rendered-inline"
    const host = view.state.facet(portals)
    if (host !== undefined) {
      owners.set(element, host)
      host.mount(element, this.render(view, element))
    }
    return element
  }

  destroy(element: HTMLElement): void {
    owners.get(element)?.unmount(element)
  }
}

function TaskRefView({ reference }: { reference: string }) {
  const { project, abbreviation, refs } = useEditorScope()
  const task = referencedTask(reference, abbreviation)
  if (task === null) return <span>[[{reference}]]</span>
  return (
    <TaskRefChip
      to={taskPath(project, task.id, task.section)}
      id={task.id}
      task={refs.find((ref) => ref.id === task.id)}
    />
  )
}

export class TaskRefWidget extends ReactWidget {
  protected readonly block = false

  constructor(private readonly reference: string) {
    super()
  }

  eq(other: TaskRefWidget): boolean {
    return other.reference === this.reference
  }

  protected render(): ReactNode {
    return <TaskRefView reference={this.reference} />
  }
}

// The caret lands where the rendered block starts, which shows the source for editing.
function editAt(view: EditorView, element: HTMLElement) {
  view.dispatch({ selection: { anchor: view.posAtDOM(element) } })
  view.focus()
}

function EditButton({ onEdit }: { onEdit: () => void }) {
  return (
    <Button
      size="icon"
      aria-label="Edit the markdown"
      onMouseDown={(event) => event.preventDefault()}
      onClick={onEdit}
      className="bg-background absolute top-1 right-1 border opacity-0 group-hover:opacity-100"
    >
      <Pencil className="size-3.5" />
    </Button>
  )
}

function MarkdownBlockView({ source, onEdit }: { source: string; onEdit: () => void }) {
  const { project, abbreviation, refs } = useEditorScope()
  return (
    <div className="cursor-text" onMouseDown={onEdit}>
      <TaskBody project={project} abbreviation={abbreviation} refs={refs} markdown={source} />
    </div>
  )
}

export class MarkdownBlockWidget extends ReactWidget {
  protected readonly block = true

  constructor(private readonly source: string) {
    super()
  }

  eq(other: MarkdownBlockWidget): boolean {
    return other.source === this.source
  }

  protected render(view: EditorView, element: HTMLElement): ReactNode {
    return <MarkdownBlockView source={this.source} onEdit={() => editAt(view, element)} />
  }
}

export class DiagramWidget extends ReactWidget {
  protected readonly block = true

  constructor(private readonly source: string) {
    super()
  }

  eq(other: DiagramWidget): boolean {
    return other.source === this.source
  }

  protected render(view: EditorView, element: HTMLElement): ReactNode {
    return (
      <div className="group relative">
        <DiagramBlock source={this.source} />
        <EditButton onEdit={() => editAt(view, element)} />
      </div>
    )
  }
}

// The daemon refuses an edit inside a conflict block, so the block is only ever replaced whole.
export class ConflictWidget extends ReactWidget {
  protected readonly block = true

  constructor(private readonly text: string) {
    super()
  }

  eq(other: ConflictWidget): boolean {
    return other.text === this.text
  }

  protected render(view: EditorView, element: HTMLElement): ReactNode {
    const [segment] = bodySegments(this.text)
    if (segment?.kind !== "conflict") return null
    const resolve = (text: string) => {
      const from = view.posAtDOM(element)
      const to = from + this.text.length
      if (view.state.sliceDoc(from, to) !== this.text) return
      view.dispatch({ changes: { from, to, insert: text.endsWith("\n") ? text : `${text}\n` } })
    }
    return <ConflictView conflict={segment} onResolve={resolve} />
  }
}

function ConflictView({ conflict, onResolve }: { conflict: ConflictBlock; onResolve: (text: string) => void }) {
  const { project, abbreviation, refs } = useEditorScope()
  return (
    <div data-keys-ignore>
      <BodyConflict
        project={project}
        abbreviation={abbreviation}
        refs={refs}
        conflict={conflict}
        onResolve={onResolve}
      />
    </div>
  )
}

export class CheckboxWidget extends WidgetType {
  constructor(private readonly checked: boolean) {
    super()
  }

  eq(other: CheckboxWidget): boolean {
    return other.checked === this.checked
  }

  toDOM(view: EditorView): HTMLElement {
    const box = document.createElement("input")
    box.type = "checkbox"
    box.checked = this.checked
    box.className = "cm-task-box"
    box.setAttribute("aria-label", this.checked ? "Mark as not done" : "Mark as done")
    box.addEventListener("mousedown", (event) => event.preventDefault())
    box.addEventListener("click", (event) => {
      event.preventDefault()
      const start = view.posAtDOM(box)
      const line = view.state.doc.lineAt(start)
      const at = line.from + line.text.indexOf("[", start - line.from) + 1
      view.dispatch({ changes: { from: at, to: at + 1, insert: this.checked ? " " : "x" } })
    })
    return box
  }
}

export class BulletWidget extends WidgetType {
  eq(): boolean {
    return true
  }

  toDOM(): HTMLElement {
    const bullet = document.createElement("span")
    bullet.className = "cm-bullet"
    bullet.textContent = "•"
    return bullet
  }
}

export class RuleWidget extends WidgetType {
  eq(): boolean {
    return true
  }

  toDOM(): HTMLElement {
    const rule = document.createElement("span")
    rule.className = "cm-rule"
    return rule
  }
}

export class ImageWidget extends WidgetType {
  constructor(
    private readonly url: string,
    private readonly alt: string,
  ) {
    super()
  }

  eq(other: ImageWidget): boolean {
    return other.url === this.url && other.alt === this.alt
  }

  toDOM(): HTMLElement {
    const image = document.createElement("img")
    image.src = this.url
    image.alt = this.alt
    image.className = "cm-image"
    return image
  }
}

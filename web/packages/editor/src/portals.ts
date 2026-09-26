import { Facet } from "@codemirror/state"
import { createContext, type ReactNode, useContext } from "react"

import type { DocRef, TaskRef } from "@openplan/api-client"

export interface EditorScope {
  readonly project: string
  readonly abbreviation: string
  readonly refs: ReadonlyArray<TaskRef>
  readonly docRefs: ReadonlyArray<DocRef>
}

export const EditorScopeContext = createContext<EditorScope>({ project: "", abbreviation: "", refs: [], docRefs: [] })

export const useEditorScope = (): EditorScope => useContext(EditorScopeContext)

export interface Portal {
  readonly key: number
  readonly element: HTMLElement
  readonly node: ReactNode
}

// CodeMirror builds widget DOM outside React. Each widget hands its element and content here, and the
// editor component renders them as portals, so they keep the router, the query client, and the scope.
export class Portals {
  private readonly entries = new Map<HTMLElement, Portal>()
  private readonly listeners = new Set<() => void>()
  private snapshot: ReadonlyArray<Portal> = []
  private next = 0

  mount(element: HTMLElement, node: ReactNode): void {
    this.entries.set(element, { key: this.next++, element, node })
    this.emit()
  }

  unmount(element: HTMLElement): void {
    if (this.entries.delete(element)) this.emit()
  }

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  readonly getSnapshot = (): ReadonlyArray<Portal> => this.snapshot

  private emit(): void {
    this.snapshot = [...this.entries.values()]
    for (const listener of this.listeners) listener()
  }
}

export const portals = Facet.define<Portals, Portals | undefined>({ combine: (values) => values[0] })

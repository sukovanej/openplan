import { errorText } from "./format"

export type SaveState =
  | { readonly kind: "saved" }
  | { readonly kind: "unsaved" }
  | { readonly kind: "saving" }
  | { readonly kind: "failed"; readonly message: string }

export interface Draft<T> {
  readonly base: T
  readonly text: T
}

export interface DraftStore<T> {
  readonly read: () => Draft<T> | undefined
  readonly write: (draft: Draft<T> | undefined) => void
}

export interface DraftView<T> {
  readonly shown: T
  readonly state: SaveState
}

export interface TextKind<T> {
  readonly same: (a: T, b: T) => boolean
  readonly is: (value: unknown) => value is T
  // Why the text cannot be sent as it is, if it cannot.
  readonly refuse: (text: T) => string | undefined
}

// Writes `text` over `base` and answers with the text the store holds after it.
export type WriteText<T> = (base: T, text: T) => Promise<T>

// Storage can be full, blocked, or absent. A draft is a second chance, so its loss costs nothing more.
export function localDraftStore<T>(key: string, kind: TextKind<T>): DraftStore<T> {
  return {
    read: () => {
      try {
        const value: unknown = JSON.parse(localStorage.getItem(key) ?? "null")
        if (typeof value !== "object" || value === null) return undefined
        const { base, text } = value as Partial<Draft<unknown>>
        return kind.is(base) && kind.is(text) ? { base, text } : undefined
      } catch {
        return undefined
      }
    },
    write: (draft) => {
      try {
        if (draft === undefined) localStorage.removeItem(key)
        else localStorage.setItem(key, JSON.stringify(draft))
      } catch {
        // The text is still in the editor.
      }
    },
  }
}

// `base` is the text the reader's text descends from. A save sends both, so the store can merge the
// edit with what another writer did since. A save that answers with a merged text puts it in place,
// and so does a new text from the store while nothing is unsaved. `shown` changes only then: what
// the reader types is already on the screen.
export class TextDraft<T> {
  readonly restored: boolean
  private base: T
  private text: T
  private view: DraftView<T>
  private saving = false
  private again = false
  private readonly listeners = new Set<() => void>()

  constructor(
    stored: T,
    private readonly write: WriteText<T>,
    private readonly store: DraftStore<T>,
    private readonly kind: TextKind<T>,
  ) {
    const draft = store.read()
    const restored = draft !== undefined && !kind.same(draft.text, stored) ? draft : undefined
    this.restored = restored !== undefined
    this.base = restored?.base ?? stored
    this.text = restored?.text ?? stored
    this.view = { shown: this.text, state: { kind: this.restored ? "unsaved" : "saved" } }
  }

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  readonly getSnapshot = (): DraftView<T> => this.view

  received(stored: T): void {
    if (!this.saving && !this.unsaved && !this.kind.same(stored, this.base)) this.adopt(stored)
  }

  change(text: T): void {
    this.text = text
    this.keepDraft()
    if (!this.saving) this.show({ state: { kind: this.unsaved ? "unsaved" : "saved" } })
  }

  // One save at a time: a save asked for while one runs goes out after it, from the base it wrote.
  async save(): Promise<void> {
    if (this.saving) {
      this.again = true
      return
    }
    this.saving = true
    try {
      do {
        this.again = false
        await this.saveOnce()
      } while (this.again)
    } finally {
      this.saving = false
    }
  }

  private get unsaved(): boolean {
    return !this.kind.same(this.text, this.base)
  }

  private async saveOnce(): Promise<void> {
    const sent = this.text
    if (!this.unsaved) return
    const refusal = this.kind.refuse(sent)
    if (refusal !== undefined) {
      this.show({ state: { kind: "failed", message: refusal } })
      return
    }
    this.show({ state: { kind: "saving" } })
    try {
      const written = await this.write(this.base, sent)
      this.base = sent
      if (this.text === sent) this.adopt(written)
      else this.keepDraft()
      this.show({ state: { kind: this.unsaved ? "unsaved" : "saved" } })
    } catch (error) {
      this.show({ state: { kind: "failed", message: errorText(error) } })
    }
  }

  private adopt(stored: T): void {
    this.base = stored
    this.text = stored
    this.store.write(undefined)
    this.show({ shown: stored })
  }

  private keepDraft(): void {
    this.store.write(this.unsaved ? { base: this.base, text: this.text } : undefined)
  }

  private show(next: Partial<DraftView<T>>): void {
    const view = { ...this.view, ...next }
    if (view.shown === this.view.shown && sameState(view.state, this.view.state)) return
    this.view = view
    for (const listener of this.listeners) listener()
  }
}

const sameState = (a: SaveState, b: SaveState) =>
  a.kind === b.kind && (a.kind !== "failed" || (b.kind === "failed" && a.message === b.message))

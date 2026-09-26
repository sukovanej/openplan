import { errorText } from "./format"

export type SaveState =
  | { readonly kind: "saved" }
  | { readonly kind: "unsaved" }
  | { readonly kind: "saving" }
  | { readonly kind: "failed"; readonly message: string }

export interface Draft {
  readonly base: string
  readonly text: string
}

export interface DraftStore {
  readonly read: () => Draft | undefined
  readonly write: (draft: Draft | undefined) => void
}

export interface DraftView {
  readonly shown: string
  readonly state: SaveState
}

// Writes `text` over `base` and answers with the body the store holds after it.
export type WriteText = (base: string, text: string) => Promise<string>

// Storage can be full, blocked, or absent. A draft is a second chance, so its loss costs nothing more.
export function localDraftStore(key: string): DraftStore {
  return {
    read: () => {
      try {
        const value: unknown = JSON.parse(localStorage.getItem(key) ?? "null")
        if (typeof value !== "object" || value === null) return undefined
        const { base, text } = value as Partial<Draft>
        return typeof base === "string" && typeof text === "string" ? { base, text } : undefined
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
export class BodyDraft {
  readonly restored: boolean
  private base: string
  private text: string
  private view: DraftView
  private saving = false
  private again = false
  private readonly listeners = new Set<() => void>()

  constructor(
    body: string,
    private readonly write: WriteText,
    private readonly store: DraftStore,
    private readonly refuse: (text: string) => string | undefined = () => undefined,
  ) {
    const draft = store.read()
    this.restored = draft !== undefined && draft.text !== body
    this.base = this.restored && draft !== undefined ? draft.base : body
    this.text = this.restored && draft !== undefined ? draft.text : body
    this.view = { shown: this.text, state: { kind: this.restored ? "unsaved" : "saved" } }
  }

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  readonly getSnapshot = (): DraftView => this.view

  received(body: string): void {
    if (!this.saving && !this.unsaved && body !== this.base) this.adopt(body)
  }

  change(text: string): void {
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
    return this.text !== this.base
  }

  private async saveOnce(): Promise<void> {
    const sent = this.text
    if (!this.unsaved) return
    const refusal = this.refuse(sent)
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

  private adopt(body: string): void {
    this.base = body
    this.text = body
    this.store.write(undefined)
    this.show({ shown: body })
  }

  private keepDraft(): void {
    this.store.write(this.unsaved ? { base: this.base, text: this.text } : undefined)
  }

  private show(next: Partial<DraftView>): void {
    const view = { ...this.view, ...next }
    if (view.shown === this.view.shown && sameState(view.state, this.view.state)) return
    this.view = view
    for (const listener of this.listeners) listener()
  }
}

const sameState = (a: SaveState, b: SaveState) =>
  a.kind === b.kind && (a.kind !== "failed" || (b.kind === "failed" && a.message === b.message))

import { Loader2, Search } from "lucide-react"
import { type KeyboardEvent, type ReactNode, useEffect, useId, useRef, useState } from "react"

import { Row } from "./row"

export interface PaletteItem {
  readonly key: string
  readonly content: ReactNode
  readonly onSelect: () => void
}

// What a consumer plugs into the palette: what to ask for, what the query finds, and what selecting
// one of the results does. The palette owns everything else — the overlay, the input, the debounce,
// the selection, and the keys.
export interface PaletteProvider {
  readonly id: string
  readonly placeholder: string
  // What an empty list means, which is not the same thing before and after the reader types.
  readonly idleLabel: string
  readonly emptyLabel: string
  readonly items: (query: string) => Promise<ReadonlyArray<PaletteItem>>
}

type Results = { readonly _tag: "items"; readonly items: ReadonlyArray<PaletteItem> } | { readonly _tag: "failure" }

const NOTHING: Results = { _tag: "items", items: [] }

const CONTROL_ALIASES: Record<string, string> = { n: "ArrowDown", p: "ArrowUp" }

const keyOf = (event: KeyboardEvent) => (event.ctrlKey ? (CONTROL_ALIASES[event.key] ?? event.key) : event.key)

// Closing unmounts the box below, and `key` remounts it when the consumer changes. Each visit
// therefore starts empty, with nothing of the last one left to paint and no request left to ask for
// a query the reader has already dismissed.
export function Palette({
  open,
  provider,
  onClose,
  debounceMs = 140,
}: {
  open: boolean
  provider: PaletteProvider
  onClose: () => void
  debounceMs?: number
}) {
  return open ? <OpenPalette key={provider.id} provider={provider} onClose={onClose} debounceMs={debounceMs} /> : null
}

function OpenPalette({
  provider,
  onClose,
  debounceMs,
}: {
  provider: PaletteProvider
  onClose: () => void
  debounceMs: number
}) {
  const [text, setText] = useState("")
  const [query, setQuery] = useState("")
  const [results, setResults] = useState<Results>(NOTHING)
  const [active, setActive] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)
  const activeRef = useRef<HTMLLIElement>(null)
  const listId = useId()

  useEffect(() => inputRef.current?.focus(), [])

  useEffect(() => {
    const id = setTimeout(() => setQuery(text), debounceMs)
    return () => clearTimeout(id)
  }, [text, debounceMs])

  // A run token discards a response that resolves after a newer keystroke asked for another one.
  const token = useRef(0)
  useEffect(() => {
    const mine = ++token.current
    void provider.items(query).then(
      (items) => {
        if (token.current === mine) setResults({ _tag: "items", items })
      },
      () => {
        if (token.current === mine) setResults({ _tag: "failure" })
      },
    )
  }, [provider, query])

  useEffect(() => setActive(0), [results])
  // Braces, not a concise body: Chrome's `scrollIntoView` answers with a promise, and React reads
  // whatever an effect returns as its clean-up function.
  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: "nearest" })
  }, [active])

  const items = results._tag === "items" ? results.items : []
  const choose = (index: number) => {
    const item = items[index]
    if (item === undefined) return
    item.onSelect()
    onClose()
  }

  const step = (delta: number) => {
    if (items.length > 0) setActive((at) => (at + delta + items.length) % items.length)
  }

  // The input holds the focus, so these keys never reach the global dispatcher; the palette's scope
  // there covers only the keys pressed while something else in the overlay has the focus.
  const onKeyDown = (event: KeyboardEvent) => {
    switch (keyOf(event)) {
      case "ArrowDown":
        event.preventDefault()
        step(1)
        break
      case "ArrowUp":
        event.preventDefault()
        step(-1)
        break
      case "Enter":
        event.preventDefault()
        choose(active)
        break
      case "Escape":
        event.preventDefault()
        onClose()
        break
    }
  }

  return (
    <div
      role="presentation"
      onClick={onClose}
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/20 p-4 pt-[12vh] backdrop-blur-[1px]"
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={provider.placeholder}
        onClick={(event) => event.stopPropagation()}
        className="bg-background w-full max-w-xl overflow-hidden rounded-xl border shadow-lg"
      >
        <div className="flex h-11 items-center gap-2 border-b px-3">
          <Search className="text-muted-foreground size-4 shrink-0" aria-hidden="true" />
          <input
            ref={inputRef}
            value={text}
            onChange={(event) => setText(event.target.value)}
            onKeyDown={onKeyDown}
            placeholder={provider.placeholder}
            autoComplete="off"
            spellCheck={false}
            role="combobox"
            aria-expanded
            aria-controls={listId}
            aria-activedescendant={items.length > 0 ? `${listId}-${active}` : undefined}
            className="placeholder:text-muted-foreground min-w-0 flex-1 bg-transparent text-sm outline-none"
          />
          {text !== query && <Loader2 className="text-muted-foreground size-3.5 shrink-0 animate-spin" />}
        </div>
        <ul id={listId} role="listbox" aria-label={provider.placeholder} className="max-h-80 overflow-y-auto p-1">
          {items.length === 0 ? (
            <li className="text-muted-foreground px-2 py-6 text-center text-sm">
              {results._tag === "failure"
                ? "Could not run the search"
                : query === ""
                  ? provider.idleLabel
                  : provider.emptyLabel}
            </li>
          ) : (
            items.map((item, index) => (
              <Row
                key={item.key}
                ref={index === active ? activeRef : undefined}
                as="li"
                id={`${listId}-${index}`}
                variant="option"
                active={index === active}
                role="option"
                aria-selected={index === active}
                onMouseDown={(event: { preventDefault: () => void }) => {
                  event.preventDefault()
                  choose(index)
                }}
                onMouseMove={() => setActive(index)}
                className="cursor-pointer"
              >
                {item.content}
              </Row>
            ))
          )}
        </ul>
      </div>
    </div>
  )
}

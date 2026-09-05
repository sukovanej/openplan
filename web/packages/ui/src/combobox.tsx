import { Loader2, Search } from "lucide-react"
import { type KeyboardEvent, type ReactNode, useEffect, useId, useMemo, useRef, useState } from "react"

import { cn } from "./cn"
import { Row } from "./row"
import { useDismissOnOutsideClick } from "./use-dismiss"

export interface ComboOption {
  readonly key: string
  readonly content: ReactNode
  readonly onSelect: () => void
}

function useDebounced<T>(value: T, ms: number): T {
  const [debounced, setDebounced] = useState(value)
  useEffect(() => {
    const id = setTimeout(() => setDebounced(value), ms)
    return () => clearTimeout(id)
  }, [value, ms])
  return debounced
}

// A single-line search field over a popover list, driven by a caller-supplied `buildOptions`. It
// owns the debounced query, keyboard navigation (↑ ↓ Enter Esc), and dismissal on outside click;
// callers decide what the options are and what selecting one does. Reused for the parent picker, the
// add-subtask box, and anything else that wants the same contract.
export function Combobox({
  placeholder,
  buildOptions,
  onClose,
  debounceMs = 140,
  emptyLabel = "No matches",
  className,
  inline = false,
}: {
  placeholder: string
  buildOptions: (query: string) => ReadonlyArray<ComboOption>
  onClose?: () => void
  debounceMs?: number
  emptyLabel?: string
  className?: string
  // Render the list in normal flow instead of an overlay, so a box opened near the bottom of a
  // scroll container extends its scroll height and can be revealed.
  inline?: boolean
}) {
  const [text, setText] = useState("")
  const query = useDebounced(text, debounceMs)
  const options = useMemo(() => buildOptions(query.trim()), [buildOptions, query])
  const [active, setActive] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)
  const rootRef = useRef<HTMLDivElement>(null)
  const listRef = useRef<HTMLUListElement>(null)
  const listId = useId()

  useEffect(() => inputRef.current?.focus(), [])
  useEffect(() => setActive(0), [options])
  useEffect(() => {
    if (inline) listRef.current?.scrollIntoView({ block: "nearest" })
  }, [inline, options])
  useDismissOnOutsideClick(rootRef, onClose)

  const choose = (index: number) => {
    const option = options[index]
    if (option === undefined) return
    option.onSelect()
    onClose?.()
  }

  const onKeyDown = (event: KeyboardEvent) => {
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault()
        if (options.length > 0) setActive((a) => (a + 1) % options.length)
        break
      case "ArrowUp":
        event.preventDefault()
        if (options.length > 0) setActive((a) => (a - 1 + options.length) % options.length)
        break
      case "Enter":
        event.preventDefault()
        choose(active)
        break
      case "Escape":
        event.preventDefault()
        onClose?.()
        break
    }
  }

  const pending = text !== query
  return (
    <div ref={rootRef} className={cn("relative", className)}>
      <div className="border-input focus-within:border-foreground/20 bg-background flex h-9 items-center gap-2 rounded-md border px-2.5 transition-colors">
        <Search className="text-muted-foreground size-4 shrink-0" />
        <input
          ref={inputRef}
          value={text}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={onKeyDown}
          placeholder={placeholder}
          autoComplete="off"
          spellCheck={false}
          role="combobox"
          aria-expanded
          aria-controls={listId}
          className="placeholder:text-muted-foreground min-w-0 flex-1 bg-transparent text-sm outline-none"
        />
        {pending && <Loader2 className="text-muted-foreground size-3.5 shrink-0 animate-spin" />}
      </div>
      <ul
        ref={listRef}
        id={listId}
        role="listbox"
        className={cn(
          "bg-popover mt-1.5 max-h-64 w-full scroll-mb-6 overflow-y-auto rounded-md border p-1",
          inline ? "shadow-sm" : "absolute z-30 shadow-md",
        )}
      >
        {options.length === 0 ? (
          <li className="text-muted-foreground px-2 py-3 text-center text-sm">{emptyLabel}</li>
        ) : (
          options.map((option, index) => (
            <Row
              key={option.key}
              as="li"
              variant="option"
              active={index === active}
              role="option"
              aria-selected={index === active}
              onMouseDown={(event) => {
                event.preventDefault()
                choose(index)
              }}
              onMouseMove={() => setActive(index)}
              className="cursor-pointer"
            >
              {option.content}
            </Row>
          ))
        )}
      </ul>
    </div>
  )
}

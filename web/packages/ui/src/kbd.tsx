import { Fragment } from "react"

import { cn } from "./cn"

const KEY_LABELS: Record<string, string> = {
  Escape: "Esc",
  Enter: "⏎",
  ArrowUp: "↑",
  ArrowDown: "↓",
  " ": "Space",
}

// Read through `globalThis` rather than the bare global: this module sits behind the package barrel,
// so a node-environment test importing any other export evaluates it where `navigator` is absent.
const APPLE = /Mac|iPhone|iPad/.test(globalThis.navigator?.userAgent ?? "")

const MODIFIER_LABELS: Record<string, string> = APPLE
  ? { mod: "⌘", alt: "⌥", shift: "⇧" }
  : { mod: "Ctrl", alt: "Alt", shift: "Shift" }

// macOS writes a chord as its symbols side by side, in this order, with no plus between them.
const MODIFIER_ORDER = ["alt", "shift", "mod"]

// A line box keeps room for descenders under the capitals, so a centered line sits high. Trimmed to
// the cap height and the baseline, each part is as tall as its capitals, and the badge centers them.
const PART = "[text-box:trim-both_cap_alphabetic]"

function keyLabels(token: string): ReadonlyArray<string> {
  const parts = token.split("+")
  const base = parts.pop() ?? token
  const named = KEY_LABELS[base] ?? (base.length === 1 ? base.toUpperCase() : base)
  const modifiers = APPLE ? parts.toSorted((a, b) => MODIFIER_ORDER.indexOf(a) - MODIFIER_ORDER.indexOf(b)) : parts
  return [...modifiers.map((part) => MODIFIER_LABELS[part] ?? part), named]
}

export function Kbd({ token, className }: { token: string; className?: string }) {
  return (
    <kbd
      className={cn(
        "bg-prose-code-surface text-prose-code border-prose-code-border inline-flex h-5 min-w-5 items-center justify-center gap-0.5 rounded border px-1 font-sans text-[0.6875rem]",
        className,
      )}
    >
      {keyLabels(token).map((label, index) => (
        <Fragment key={index}>
          {index > 0 && !APPLE && <span className={PART}>+</span>}
          <span className={PART}>{label}</span>
        </Fragment>
      ))}
    </kbd>
  )
}

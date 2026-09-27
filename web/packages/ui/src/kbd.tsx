import { Fragment } from "react"

import { cn } from "./cn"

const KEY_LABELS: Record<string, string> = {
  Escape: "Esc",
  Enter: "↵",
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

// These symbols sit small beside the capitals of the monospace face.
const SYMBOLS = new Set(["⌘", "⌥", "⇧", "↵", "↑", "↓"])

// A line box keeps room for descenders under the capitals, so a centered line sits high. Trimmed to
// the cap height and the baseline, each part is as tall as its capitals, and the badge centers them.
// The return arrow is drawn no higher than the x-height, so it is trimmed to that.
const PART = "[text-box:trim-both_cap_alphabetic]"
const LOW_PART = "[text-box:trim-both_ex_alphabetic]"

function keyLabels(token: string): ReadonlyArray<string> {
  const parts = token.split("+")
  const base = parts.pop() ?? token
  const named = KEY_LABELS[base] ?? (base.length === 1 ? base.toUpperCase() : base)
  return [...parts.map((part) => MODIFIER_LABELS[part] ?? part), named]
}

export function Kbd({ token, className }: { token: string; className?: string }) {
  return (
    <kbd
      className={cn(
        "bg-prose-code-surface text-prose-code border-prose-code-border inline-flex h-5 min-w-5 items-center justify-center gap-0.5 rounded border px-1 font-mono text-[0.6875rem]",
        className,
      )}
    >
      {keyLabels(token).map((label, index) => (
        <Fragment key={index}>
          {index > 0 && <span className={PART}>+</span>}
          <span className={cn(label === "↵" ? LOW_PART : PART, SYMBOLS.has(label) && "text-[1.3em]")}>{label}</span>
        </Fragment>
      ))}
    </kbd>
  )
}

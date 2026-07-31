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

function keyLabel(token: string): string {
  const parts = token.split("+")
  const base = parts.pop() ?? token
  const named = KEY_LABELS[base] ?? (base.length === 1 ? base.toUpperCase() : base)
  return [...parts.map((part) => MODIFIER_LABELS[part] ?? part), named].join(APPLE ? "" : "+")
}

export function Kbd({ token, className }: { token: string; className?: string }) {
  return (
    <kbd
      className={cn(
        "bg-muted text-muted-foreground inline-flex h-6 min-w-6 items-center justify-center rounded border px-1.5 text-xs font-medium",
        className,
      )}
    >
      {keyLabel(token)}
    </kbd>
  )
}

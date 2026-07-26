import type { KeySpec } from "./types"

const MOD_ALIASES = new Set(["mod", "cmd", "command", "meta", "ctrl", "control"])
const ALT_ALIASES = new Set(["alt", "option"])
const SHIFT_ALIASES = new Set(["shift"])
const ACTIVATION_KEYS = new Set(["Enter", " "])

interface Modifiers {
  readonly mod: boolean
  readonly alt: boolean
  readonly shift: boolean
}

function canonical(base: string, { mod, alt, shift }: Modifiers): string {
  const named = base.length !== 1
  const key = named ? base : base.toLowerCase()
  // A bare printable key already encodes Shift in its glyph ("?" is Shift+"/"), so recording
  // Shift again would double-count it; only track it where the character can't reveal it —
  // named keys (Shift+Tab) and modified chords (Cmd+Shift+K vs Cmd+K).
  const withShift = shift && (named || mod || alt)
  return `${mod ? "mod+" : ""}${alt ? "alt+" : ""}${withShift ? "shift+" : ""}${key}`
}

export function normalizeToken(authored: string): string {
  const segments = authored
    .split("+")
    .map((segment) => segment.trim())
    .filter((segment) => segment.length > 0)
  const base = segments.pop() ?? ""
  const modifiers = segments.map((segment) => segment.toLowerCase())
  return canonical(base, {
    mod: modifiers.some((modifier) => MOD_ALIASES.has(modifier)),
    alt: modifiers.some((modifier) => ALT_ALIASES.has(modifier)),
    shift: modifiers.some((modifier) => SHIFT_ALIASES.has(modifier)),
  })
}

export function fromEvent(event: KeyboardEvent): string {
  return canonical(event.key, { mod: event.metaKey || event.ctrlKey, alt: event.altKey, shift: event.shiftKey })
}

export function chordOf(keys: KeySpec): ReadonlyArray<string> {
  return (typeof keys === "string" ? [keys] : keys).map(normalizeToken)
}

export function startsWith(sequence: ReadonlyArray<string>, prefix: ReadonlyArray<string>): boolean {
  if (prefix.length > sequence.length) return false
  for (let i = 0; i < prefix.length; i++) {
    if (sequence[i] !== prefix[i]) return false
  }
  return true
}

export function sameChord(a: ReadonlyArray<string>, b: ReadonlyArray<string>): boolean {
  return a.length === b.length && startsWith(a, b)
}

export function hasCommandModifier(event: KeyboardEvent): boolean {
  return event.metaKey || event.ctrlKey || event.altKey
}

export function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  if (target.isContentEditable) return true
  if (target.closest("[data-keys-ignore]") !== null) return true
  const tag = target.tagName
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT"
}

export function isActivationKey(event: KeyboardEvent): boolean {
  return ACTIVATION_KEYS.has(event.key)
}

export function isActivationTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  return target.closest('a[href], button, summary, [role="button"], [role="link"]') !== null
}

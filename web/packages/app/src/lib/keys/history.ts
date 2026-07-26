// react-router stamps a monotonic `idx` into each history entry's state. It is the only way to tell
// a Back from a Forward — `useNavigationType` reports both as POP — and it survives a reload, so a
// counter kept in a ref cannot replace it.
export function historyIndex(): number {
  const state: unknown = globalThis.history?.state
  const idx = (state as { idx?: unknown } | null)?.idx
  return typeof idx === "number" ? idx : 0
}

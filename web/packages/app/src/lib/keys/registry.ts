import { chordOf } from "./match"
import type { Binding } from "./types"

export interface HelpEntry {
  readonly id: string
  readonly keys: ReadonlyArray<string>
  readonly label: string
}

export interface HelpGroup {
  readonly name: string
  readonly entries: ReadonlyArray<HelpEntry>
}

export function helpGroups(bindings: ReadonlyArray<Binding>): ReadonlyArray<HelpGroup> {
  const order: Array<string> = []
  const byGroup = new Map<string, Array<HelpEntry>>()
  for (const binding of bindings) {
    if (binding.scope === "overlay") continue
    let entries = byGroup.get(binding.group)
    if (entries === undefined) {
      entries = []
      byGroup.set(binding.group, entries)
      order.push(binding.group)
    }
    entries.push({ id: binding.id, keys: chordOf(binding.keys), label: binding.label })
  }
  return order.map((name) => ({ name, entries: byGroup.get(name)! }))
}

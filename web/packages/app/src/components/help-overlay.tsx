import { Dialog, Kbd } from "@openplan/ui"

import { bindings, type HelpEntry, helpGroups } from "../lib/keys"

const GROUPS = helpGroups(bindings)

export function HelpOverlay({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <Dialog open={open} onClose={onClose} title="Keyboard shortcuts">
      <div className="space-y-5">
        {GROUPS.map((group) => (
          <section key={group.name}>
            <h3 className="text-muted-foreground mb-2 text-xs font-medium tracking-wide uppercase">{group.name}</h3>
            <ul className="space-y-1.5">
              {group.entries.map((entry) => (
                <HelpRow key={entry.id} entry={entry} />
              ))}
            </ul>
          </section>
        ))}
      </div>
    </Dialog>
  )
}

function HelpRow({ entry }: { entry: HelpEntry }) {
  return (
    <li className="flex items-center justify-between gap-4">
      <span className="text-foreground/90 text-sm">{entry.label}</span>
      <span className="flex items-center gap-1">
        {entry.keys.map((token, index) => (
          <Kbd key={index} token={token} />
        ))}
      </span>
    </li>
  )
}

import { Effect } from "effect"
import { useMemo } from "react"
import { useNavigate } from "react-router-dom"

import type { SearchHit } from "@open-planner/api-client"
import { statusField, TaskIdentity } from "@open-planner/task-ui"
import { Palette, type PaletteItem, type PaletteProvider } from "@open-planner/ui"

import { searchTasks } from "../lib/api"
import type { PaletteTarget } from "../lib/keys"
import { hitKey, hitPath } from "../lib/palette-search"
import { runtime } from "../lib/runtime"

function searchProvider(open: (to: string) => void): PaletteProvider {
  return {
    id: "search",
    placeholder: "Search tasks",
    idleLabel: "Type to search titles, bodies, and frontmatter",
    emptyLabel: "No matching tasks",
    items: (query) => runtime.runPromise(Effect.map(searchTasks(query), (hits) => hits.map((hit) => row(hit, open)))),
  }
}

function row(hit: SearchHit, open: (to: string) => void): PaletteItem {
  return {
    key: hitKey(hit),
    content: <TaskIdentity status={statusField(hit.task.metadata)} id={hit.task.id} title={hit.task.title} />,
    onSelect: () => open(hitPath(hit)),
  }
}

// The palette's consumers, by the target a key binding opens. `home` is the general command
// interface's seat; the only consumer registered there so far is search, so ⌘K lands where `/` does
// until that interface ships.
function providerFor(target: PaletteTarget, open: (to: string) => void): PaletteProvider {
  switch (target) {
    case "home":
    case "search":
      return searchProvider(open)
  }
}

export function CommandPalette({
  open,
  target,
  onClose,
}: {
  open: boolean
  target: PaletteTarget
  onClose: () => void
}) {
  const navigate = useNavigate()
  const provider = useMemo(() => providerFor(target, (to) => navigate(to)), [target, navigate])
  return <Palette open={open} provider={provider} onClose={onClose} />
}

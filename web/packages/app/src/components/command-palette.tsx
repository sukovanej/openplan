import { Effect } from "effect"
import { Waypoints, type LucideIcon } from "lucide-react"
import { useMemo } from "react"
import { useNavigate } from "react-router-dom"

import type { SearchHit } from "@open-planner/api-client"
import { FLOW_ROUTE, statusField, TaskIdentity } from "@open-planner/task-ui"
import { FuzzyText, fuzzyMatch, Palette, type PaletteItem, type PaletteProvider } from "@open-planner/ui"

import { searchTasks } from "../lib/api"
import type { PaletteTarget } from "../lib/keys"
import { hitKey, hitPath } from "../lib/palette-search"
import { runtime } from "../lib/runtime"

interface Command {
  readonly label: string
  readonly icon: LucideIcon
  readonly to: string
}

const COMMANDS: ReadonlyArray<Command> = [{ label: "Show the implementation flow", icon: Waypoints, to: FLOW_ROUTE }]

function commandItems(query: string, open: (to: string) => void): ReadonlyArray<PaletteItem> {
  return COMMANDS.flatMap((command) => {
    const match = fuzzyMatch(query, command.label)
    return match === null ? [] : [{ match, command }]
  })
    .sort((a, b) => a.match.score - b.match.score)
    .map(({ match, command }) => ({
      key: `command ${command.to}`,
      content: (
        <span className="flex min-w-0 items-center gap-2">
          <command.icon className="text-muted-foreground size-4 shrink-0" />
          <span className="min-w-0 truncate">
            <FuzzyText text={command.label} indices={match.indices} />
          </span>
        </span>
      ),
      onSelect: () => open(command.to),
    }))
}

function searchItems(query: string, open: (to: string) => void): Promise<ReadonlyArray<PaletteItem>> {
  return runtime.runPromise(Effect.map(searchTasks(query), (hits) => hits.map((hit) => row(hit, open))))
}

function searchProvider(open: (to: string) => void): PaletteProvider {
  return {
    id: "search",
    placeholder: "Search tasks",
    idleLabel: "Type to search titles, bodies, and frontmatter",
    emptyLabel: "No matching tasks",
    items: (query) => searchItems(query, open),
  }
}

// The general command interface: the commands the app answers for, and the tasks a query finds, in
// one list. A search the daemon refuses takes the tasks with it and leaves the commands, which need
// no daemon to run.
function homeProvider(open: (to: string) => void): PaletteProvider {
  return {
    id: "home",
    placeholder: "Search tasks or run a command",
    idleLabel: "Type to search titles, bodies, and frontmatter",
    emptyLabel: "No matching command or task",
    items: async (query) => [...commandItems(query, open), ...(await searchItems(query, open).catch(() => []))],
  }
}

function row(hit: SearchHit, open: (to: string) => void): PaletteItem {
  return {
    key: hitKey(hit),
    content: <TaskIdentity status={statusField(hit.task.metadata)} id={hit.task.id} title={hit.task.title} />,
    onSelect: () => open(hitPath(hit)),
  }
}

function providerFor(target: PaletteTarget, open: (to: string) => void): PaletteProvider {
  switch (target) {
    case "home":
      return homeProvider(open)
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

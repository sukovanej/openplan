import { Effect } from "effect"
import { Plus, Waypoints, type LucideIcon } from "lucide-react"
import { useMemo } from "react"
import { useNavigate } from "react-router-dom"

import type { SearchHit } from "@openplan/api-client"
import { FLOW_ROUTE, statusField, TaskIdentity, taskPath } from "@openplan/task-ui"
import { FuzzyText, fuzzyMatch, Palette, type PaletteItem, type PaletteProvider } from "@openplan/ui"

import { searchTasks } from "../lib/api"
import type { PaletteTarget } from "../lib/keys"
import { runtime } from "../lib/runtime"

interface Command {
  readonly label: string
  readonly icon: LucideIcon
  readonly run: (actions: PaletteActions) => void
}

interface PaletteActions {
  readonly open: (to: string) => void
  readonly newTask: () => void
}

const COMMANDS: ReadonlyArray<Command> = [
  { label: "Create a task", icon: Plus, run: (actions) => actions.newTask() },
  { label: "Show the implementation flow", icon: Waypoints, run: (actions) => actions.open(FLOW_ROUTE) },
]

function commandItems(query: string, actions: PaletteActions): ReadonlyArray<PaletteItem> {
  return COMMANDS.flatMap((command) => {
    const match = fuzzyMatch(query, command.label)
    return match === null ? [] : [{ match, command }]
  })
    .sort((a, b) => a.match.score - b.match.score)
    .map(({ match, command }) => ({
      key: `command ${command.label}`,
      content: (
        <span className="flex min-w-0 items-center gap-2">
          <command.icon className="text-muted-foreground size-4 shrink-0" />
          <span className="min-w-0 truncate">
            <FuzzyText text={command.label} indices={match.indices} />
          </span>
        </span>
      ),
      onSelect: () => command.run(actions),
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
function homeProvider(actions: PaletteActions): PaletteProvider {
  return {
    id: "home",
    placeholder: "Search tasks or run a command",
    idleLabel: "Type to search titles, bodies, and frontmatter",
    emptyLabel: "No matching command or task",
    items: async (query) => [
      ...commandItems(query, actions),
      ...(await searchItems(query, actions.open).catch(() => [])),
    ],
  }
}

// A key is unique only inside its project, so the two together name a row.
function row(hit: SearchHit, open: (to: string) => void): PaletteItem {
  return {
    key: `${hit.task.project} ${hit.task.id}`,
    content: <TaskIdentity status={statusField(hit.task.metadata)} id={hit.task.id} title={hit.task.title} />,
    onSelect: () => open(taskPath(hit.task.project, hit.task.id)),
  }
}

function providerFor(target: PaletteTarget, actions: PaletteActions): PaletteProvider {
  switch (target) {
    case "home":
      return homeProvider(actions)
    case "search":
      return searchProvider(actions.open)
  }
}

export function CommandPalette({
  open,
  target,
  onClose,
  onNewTask,
}: {
  open: boolean
  target: PaletteTarget
  onClose: () => void
  onNewTask: () => void
}) {
  const navigate = useNavigate()
  const provider = useMemo(
    () => providerFor(target, { open: (to) => navigate(to), newTask: onNewTask }),
    [target, navigate, onNewTask],
  )
  return <Palette open={open} provider={provider} onClose={onClose} />
}

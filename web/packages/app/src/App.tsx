import { Waypoints } from "lucide-react"
import { Link, Outlet } from "react-router-dom"

import { FLOW_ROUTE } from "@open-planner/task-ui"

import { CommandPalette } from "./components/command-palette"
import { ConnectionStatus } from "./components/connection-status"
import { Flash } from "./components/flash"
import { HelpOverlay } from "./components/help-overlay"
import { MutationError } from "./components/mutation-error"
import { ProjectSwitcher } from "./components/project-switcher"
import { ThemeToggle } from "./components/theme-toggle"
import { useKeyboard } from "./lib/keys"

export function App() {
  const { activeOverlay, paletteTarget, closeOverlay } = useKeyboard()
  return (
    <div className="bg-background text-foreground flex h-screen flex-col">
      <header className="shrink-0 border-b">
        <div className="flex items-center gap-3 px-6 py-4">
          <Link to="/" className="shrink-0 text-2xl font-semibold tracking-tight">
            Open Plan
          </Link>
          <ProjectSwitcher />
          <ConnectionStatus />
          <Link
            to={FLOW_ROUTE}
            className="text-muted-foreground hover:text-foreground ml-auto inline-flex shrink-0 items-center gap-1.5 text-xs"
          >
            <Waypoints className="size-3.5" />
            Flow
          </Link>
          <ThemeToggle />
        </div>
      </header>
      <main className="min-h-0 flex-1 overflow-hidden px-4 py-4">
        <Outlet />
      </main>
      <HelpOverlay open={activeOverlay === "help"} onClose={closeOverlay} />
      <CommandPalette open={activeOverlay === "palette"} target={paletteTarget} onClose={closeOverlay} />
      <MutationError />
      <Flash />
    </div>
  )
}

import { Link, Outlet } from "react-router-dom"

import { DiagramDrawer } from "@openplan/task-ui"

import { CommandPalette } from "./components/command-palette"
import { ConnectionStatus } from "./components/connection-status"
import { Flash } from "./components/flash"
import { HelpOverlay } from "./components/help-overlay"
import { MutationError } from "./components/mutation-error"
import { PageNav } from "./components/page-nav"
import { ProjectSelect } from "./components/project-select"
import { SyncStatus } from "./components/sync-status"
import { ThemeToggle } from "./components/theme-toggle"
import { drawDiagramOnce } from "./lib/diagrams"
import { useKeyboard } from "./lib/keys"

export function App() {
  const { activeOverlay, paletteTarget, closeOverlay } = useKeyboard()
  return (
    <div className="bg-background text-foreground flex h-screen flex-col">
      <header className="shrink-0 border-b">
        <div className="flex flex-wrap items-center gap-3 px-6 py-4">
          <Link to="/" className="shrink-0 text-2xl font-semibold tracking-tight">
            Open Plan
          </Link>
          {/* A narrow window has no room for one row, and the project and its pages take a second. */}
          <div aria-hidden className="basis-full max-md:order-1 md:hidden" />
          {/* A row wraps before its items shrink, so the menu starts from no width and grows back to
              its own. At full width it would push the row onto two lines where it fits on one. */}
          <ProjectSelect className="max-w-fit grow basis-0 max-md:order-2 max-md:max-w-none" />
          <PageNav className="max-md:order-2" />
          <ConnectionStatus />
          <div className="ml-auto flex items-center gap-3">
            <SyncStatus />
            <ThemeToggle />
          </div>
        </div>
      </header>
      <main className="min-h-0 flex-1 overflow-hidden px-4 py-4">
        <DiagramDrawer value={drawDiagramOnce}>
          <Outlet />
        </DiagramDrawer>
      </main>
      <HelpOverlay open={activeOverlay === "help"} onClose={closeOverlay} />
      <CommandPalette open={activeOverlay === "palette"} target={paletteTarget} onClose={closeOverlay} />
      <MutationError />
      <Flash />
    </div>
  )
}

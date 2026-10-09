import { Plus, Search } from "lucide-react"
import { Link, Outlet } from "react-router-dom"

import { DiagramDrawer, RefReader } from "@openplan/task-ui"
import { Button, Kbd, Tooltip } from "@openplan/ui"

import { CommandPalette } from "./components/command-palette"
import { ConnectionStatus } from "./components/connection-status"
import { FaultStatus } from "./components/fault-status"
import { Flash } from "./components/flash"
import { HelpOverlay } from "./components/help-overlay"
import { MutationError } from "./components/mutation-error"
import { NewTaskDialog } from "./components/new-task-dialog"
import { PageNav, TabBar } from "./components/page-nav"
import { ProjectSelect } from "./components/project-select"
import { SyncStatus } from "./components/sync-status"
import { ThemeButton, ThemeToggle } from "./components/theme-toggle"
import { drawDiagramOnce } from "./lib/diagrams"
import { useKeyboard } from "./lib/keys"
import { refReader } from "./lib/ref-reader"

export function App() {
  const { activeOverlay, paletteTarget, openPalette, openNewTask, closeOverlay } = useKeyboard()
  return (
    // A phone's browser bar comes and goes, and `dvh` follows it where `vh` would hide the tab bar
    // under it.
    <div className="bg-background text-foreground flex h-dvh flex-col">
      <header className="shrink-0 border-b">
        <div className="flex items-center gap-3 px-6 py-4 max-md:gap-2 max-md:px-3 max-md:py-2">
          <Link to="/" aria-label="Open Plan" className="shrink-0 text-2xl font-semibold tracking-tight">
            <img src="/icon.svg" alt="" className="size-8 md:hidden" />
            <span className="max-md:hidden">Open Plan</span>
          </Link>
          <ProjectSelect className="max-w-fit grow basis-0" />
          <PageNav className="max-md:hidden" />
          <ConnectionStatus />
          <div className="ml-auto flex items-center gap-3 max-md:gap-1">
            <Tooltip
              content={
                <span className="inline-flex items-center gap-2">
                  Create a task <Kbd token="c" className="h-5 px-1" />
                </span>
              }
            >
              <Button
                variant="accent"
                size="md"
                aria-label="New task"
                onClick={openNewTask}
                className="max-md:size-9 max-md:justify-center max-md:px-0"
              >
                <Plus className="size-4 max-md:size-5" aria-hidden />
                <span className="max-md:hidden">New task</span>
              </Button>
            </Tooltip>
            <FaultStatus />
            <SyncStatus />
            <Button
              size="icon"
              aria-label="Search"
              onClick={() => openPalette("home")}
              className="text-muted-foreground size-9 md:hidden"
            >
              <Search className="size-5" aria-hidden />
            </Button>
            <ThemeButton className="md:hidden" />
            <ThemeToggle className="max-md:hidden" />
          </div>
        </div>
      </header>
      <main className="min-h-0 flex-1 overflow-hidden p-4 max-md:p-2">
        <DiagramDrawer value={drawDiagramOnce}>
          <RefReader value={refReader}>
            <Outlet />
          </RefReader>
        </DiagramDrawer>
      </main>
      <TabBar className="md:hidden" />
      <HelpOverlay open={activeOverlay === "help"} onClose={() => closeOverlay("help")} />
      <CommandPalette
        open={activeOverlay === "palette"}
        target={paletteTarget}
        onClose={() => closeOverlay("palette")}
        onNewTask={openNewTask}
      />
      <NewTaskDialog open={activeOverlay === "new-task"} onClose={() => closeOverlay("new-task")} />
      <MutationError />
      <Flash />
    </div>
  )
}

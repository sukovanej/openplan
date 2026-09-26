import type { ReactNode } from "react"
import { Link } from "react-router-dom"

import { cn, Panel, PanelBody, PanelHeader, SkeletonList } from "@openplan/ui"

import { listPath, type ListView } from "../lib/project-scope"
import { useRowCursor } from "../lib/row-cursor"

const VIEWS: ReadonlyArray<{ readonly view: ListView; readonly label: string }> = [
  { view: "tasks", label: "Tasks" },
  { view: "docs", label: "Docs" },
]

const NO_ROWS: ReadonlyArray<string> = []

export function ListHeader({
  view,
  project,
  action,
}: {
  view: ListView
  project: string | undefined
  action?: ReactNode
}) {
  return (
    <PanelHeader className="gap-3">
      <nav aria-label="Lists" className="bg-muted flex items-center gap-0.5 rounded-md p-0.5">
        {VIEWS.map((entry) => (
          <Link
            key={entry.view}
            to={listPath(entry.view, project)}
            aria-current={entry.view === view ? "page" : undefined}
            className={cn(
              "focus-visible:ring-ring rounded-sm px-2 py-0.5 text-xs whitespace-nowrap transition-colors focus-visible:ring-2 focus-visible:outline-none",
              entry.view === view
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            {entry.label}
          </Link>
        ))}
      </nav>
      {action !== undefined && <div className="ml-auto flex min-w-0 items-center">{action}</div>}
    </PanelHeader>
  )
}

// While the list loads, the cursor keeps the rows of the page it left, and `j` then Enter would open
// one of them.
export function ListSkeleton({
  view,
  project,
  action,
}: {
  view: ListView
  project: string | undefined
  action?: ReactNode
}) {
  useRowCursor(NO_ROWS)
  return (
    <Panel>
      <ListHeader view={view} project={project} action={action} />
      <PanelBody className="p-6">
        <SkeletonList count={3} className="h-10 w-full" />
      </PanelBody>
    </Panel>
  )
}

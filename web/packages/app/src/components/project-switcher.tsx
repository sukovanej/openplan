import { TriangleAlert } from "lucide-react"
import { Link, useLocation } from "react-router-dom"

import { boardPath, taskRouteOf } from "@open-planner/task-ui"
import { cn, Tooltip } from "@open-planner/ui"

import { demotedReason, useProjects } from "../lib/projects"

const MERGED = "All projects"

// A demoted project keeps its entry: it is registered, it is listed, and hiding it would leave the
// user looking for a project the daemon is still holding. The mark says it cannot answer, and the
// tooltip says why.
export function ProjectSwitcher() {
  const projects = useProjects()
  const current = currentProject(useLocation().pathname)
  if (projects === undefined || projects.length === 0) return null
  return (
    <nav aria-label="Projects" className="bg-muted flex min-w-0 items-center gap-0.5 overflow-x-auto rounded-md p-0.5">
      <Entry to="/" label={MERGED} active={current === undefined} />
      {projects.map((project) => (
        <Entry
          key={project.name}
          to={boardPath(project.name)}
          label={project.name}
          active={current === project.name}
          reason={demotedReason(project)}
        />
      ))}
    </nav>
  )
}

function Entry({ to, label, active, reason }: { to: string; label: string; active: boolean; reason?: string }) {
  const link = (
    <Link
      to={to}
      aria-current={active ? "page" : undefined}
      className={cn(
        "focus-visible:ring-ring inline-flex items-center gap-1.5 rounded-sm px-2 py-1 text-sm whitespace-nowrap transition-colors focus-visible:ring-2 focus-visible:outline-none",
        active ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground",
      )}
    >
      {reason !== undefined && <TriangleAlert className="text-warning size-3.5 shrink-0" aria-hidden />}
      {label}
    </Link>
  )
  return reason === undefined ? link : <Tooltip content={reason}>{link}</Tooltip>
}

// The merged board and every task detail spell their project the same way: as the first segment.
function currentProject(pathname: string): string | undefined {
  const task = taskRouteOf(pathname)
  if (task !== undefined) return task.project
  const [, project] = pathname.split("/")
  return project === undefined || project === "" ? undefined : decodeURIComponent(project)
}

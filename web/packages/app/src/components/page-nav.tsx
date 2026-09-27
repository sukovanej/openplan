import { Activity, FileText, ListChecks, type LucideIcon, Tags, Waypoints } from "lucide-react"
import { Link, useLocation } from "react-router-dom"

import { cn, segment, SEGMENT_GROUP } from "@openplan/ui"

import { type Page, pageOf, pagePath, selectedProject } from "../lib/project-scope"

const PAGES: ReadonlyArray<{ readonly page: Page; readonly label: string; readonly Icon: LucideIcon }> = [
  { page: "tasks", label: "Tasks", Icon: ListChecks },
  { page: "docs", label: "Docs", Icon: FileText },
  { page: "activity", label: "Activity", Icon: Activity },
  { page: "tags", label: "Tags", Icon: Tags },
  { page: "flow", label: "Flow", Icon: Waypoints },
]

export function PageNav({ className }: { className?: string }) {
  const { pathname, search } = useLocation()
  const project = selectedProject(pathname, search)
  const current = pageOf(pathname)
  return (
    <nav aria-label="Pages" className={cn(SEGMENT_GROUP, className)}>
      {PAGES.map(({ page, label, Icon }) => (
        <Link
          key={page}
          to={pagePath(page, project)}
          aria-current={page === current ? "page" : undefined}
          className={cn(segment(page === current), "gap-1.5 px-2 text-xs whitespace-nowrap")}
        >
          <Icon className="size-3 shrink-0" aria-hidden />
          <span className="max-sm:sr-only">{label}</span>
        </Link>
      ))}
    </nav>
  )
}

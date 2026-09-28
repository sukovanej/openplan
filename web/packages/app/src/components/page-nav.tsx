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

function usePageLinks() {
  const { pathname, search } = useLocation()
  const project = selectedProject(pathname, search)
  const current = pageOf(pathname)
  return PAGES.map((entry) => ({ ...entry, to: pagePath(entry.page, project), current: entry.page === current }))
}

export function PageNav({ className }: { className?: string }) {
  return (
    <nav aria-label="Pages" className={cn(SEGMENT_GROUP, className)}>
      {usePageLinks().map(({ page, label, Icon, to, current }) => (
        <Link
          key={page}
          to={to}
          aria-current={current ? "page" : undefined}
          className={cn(segment(current), "gap-1.5 px-2 text-xs whitespace-nowrap")}
        >
          <Icon className="size-3 shrink-0" aria-hidden />
          {label}
        </Link>
      ))}
    </nav>
  )
}

export function TabBar({ className }: { className?: string }) {
  return (
    <nav aria-label="Pages" className={cn("bg-background grid shrink-0 grid-cols-5 border-t", className)}>
      {usePageLinks().map(({ page, label, Icon, to, current }) => (
        <Link
          key={page}
          to={to}
          aria-current={current ? "page" : undefined}
          className={cn(
            "focus-visible:ring-ring flex flex-col items-center gap-1 pt-2 pb-1.5 text-[0.6875rem] font-medium transition-colors focus-visible:ring-2 focus-visible:outline-none focus-visible:ring-inset",
            current ? "text-foreground" : "text-muted-foreground",
          )}
        >
          <span
            className={cn(
              "flex h-7 w-12 items-center justify-center rounded-full transition-colors",
              current && "bg-muted",
            )}
          >
            <Icon className="size-5" aria-hidden />
          </span>
          {label}
        </Link>
      ))}
    </nav>
  )
}

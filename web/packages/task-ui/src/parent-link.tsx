import { CornerLeftUp } from "lucide-react"
import type { ReactNode } from "react"
import { Link } from "react-router-dom"

import { MetaItem, Tooltip } from "@openplan/ui"

import { docPath, taskPath } from "./task-path"

export function ParentLink({ project, id, title }: { project: string; id: string; title: string }) {
  return (
    <ParentItem to={taskPath(project, id)} relation={`Subtask of ${title}`}>
      <span className="text-muted-foreground shrink-0 tabular-nums">{id}</span>
      <span className="max-w-[15rem] truncate group-hover:underline">{title}</span>
    </ParentItem>
  )
}

export function DocParentLink({ project, name, title }: { project: string; name: string; title: string }) {
  return (
    <ParentItem to={docPath(project, name)} relation={`Nested under ${title}`}>
      <span className="max-w-[15rem] truncate group-hover:underline">{title}</span>
    </ParentItem>
  )
}

function ParentItem({ to, relation, children }: { to: string; relation: string; children: ReactNode }) {
  return (
    <Tooltip content={relation} className="min-w-0">
      <MetaItem icon={CornerLeftUp}>
        <Link to={to} className="text-foreground/90 group flex min-w-0 items-center gap-1.5">
          {children}
        </Link>
      </MetaItem>
    </Tooltip>
  )
}

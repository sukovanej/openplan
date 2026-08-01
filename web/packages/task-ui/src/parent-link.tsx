import { CornerLeftUp } from "lucide-react"
import { Link } from "react-router-dom"

import { MetaItem, Tooltip } from "@open-planner/ui"

import { taskPath } from "./task-path"

export function ParentLink({ id, title }: { id: string; title: string }) {
  return (
    <Tooltip content={`Subtask of ${title}`} className="min-w-0">
      <MetaItem icon={CornerLeftUp}>
        <Link to={taskPath(id)} className="text-foreground/90 group flex min-w-0 items-center gap-1.5">
          <span className="text-muted-foreground shrink-0 tabular-nums">{id}</span>
          <span className="max-w-[15rem] truncate group-hover:underline">{title}</span>
        </Link>
      </MetaItem>
    </Tooltip>
  )
}

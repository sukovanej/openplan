import { CornerLeftUp } from "lucide-react"
import { Link } from "react-router-dom"

import { MetaItem } from "@open-planner/ui"

import { taskPath } from "./task-path"

export function ParentLink({ id, title }: { id: string; title: string }) {
  return (
    <MetaItem icon={CornerLeftUp} title={`Subtask of ${title}`}>
      <Link to={taskPath(id)} className="text-foreground/90 group flex min-w-0 items-center gap-1.5">
        <span className="text-muted-foreground shrink-0 tabular-nums">{id}</span>
        <span className="max-w-[15rem] truncate group-hover:underline">{title}</span>
      </Link>
    </MetaItem>
  )
}

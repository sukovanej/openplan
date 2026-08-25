import { Link } from "react-router-dom"

import type { TaskRef } from "@open-planner/api-client"
import { cn } from "@open-planner/ui"

import { TaskIdentity, UnresolvedMark } from "./task-identity"

// `align-middle` centres the chip on the surrounding font's x-height, which leaves it sitting ~1.5px
// below the optical middle of the line; the nudge takes that back without disturbing the line box.
const CHIP =
  "not-prose relative -top-px mx-0.5 inline-flex max-w-full items-center rounded-md border px-1.5 py-0.5 align-middle text-sm font-medium leading-none no-underline transition-colors"

// A reference the store cannot resolve has no status and no title to show; it renders dashed, with
// its key as all there is to name it.
export function TaskRefChip({ to, id, task }: { to: string; id: string; task: TaskRef | undefined }) {
  return (
    <Link
      to={to}
      className={cn(
        CHIP,
        task === undefined
          ? "border-border border-dashed text-muted-foreground hover:bg-muted/40"
          : "border-border bg-muted/40 text-foreground hover:bg-muted",
      )}
    >
      <TaskIdentity
        variant="chip"
        status={task?.status}
        mark={task === undefined ? <UnresolvedMark /> : undefined}
        id={id}
        title={task?.title}
      />
    </Link>
  )
}

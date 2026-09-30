import { Link } from "react-router-dom"

import { cn } from "@openplan/ui"

import { useRefReader } from "./ref-reader"
import { TaskIdentity, UnresolvedMark } from "./task-identity"

// `align-middle` centres the chip on the surrounding font's x-height, which leaves it sitting ~1.5px
// below the optical middle of the line; the nudge takes that back without disturbing the line box.
const CHIP =
  "not-prose relative -top-px mx-0.5 inline-flex max-w-full items-center rounded-md border px-1.5 py-0.5 align-middle text-sm font-medium leading-5 no-underline transition-colors"

// A task the store does not hold has no status to show; it renders dashed. A task still being read
// is neither yet, so it holds the place of its mark and claims nothing. `sign` marks a task that
// joined or left a set.
export function TaskRefChip({ to, project, id, sign }: { to: string; project: string; id: string; sign?: "+" | "−" }) {
  const { useTask } = useRefReader()
  const task = useTask(project, id)
  return (
    <Link
      to={to}
      className={cn(
        CHIP,
        task === "gone"
          ? "border-border border-dashed text-muted-foreground hover:bg-muted/40"
          : "border-border bg-muted/40 text-foreground hover:bg-muted",
      )}
    >
      {sign !== undefined && <span className="mr-1 font-semibold">{sign}</span>}
      <TaskIdentity
        variant="chip"
        status={typeof task === "object" ? task.status : undefined}
        mark={task === "gone" ? <UnresolvedMark /> : task === "reading" ? <ReadingMark /> : undefined}
        id={id}
      />
    </Link>
  )
}

function ReadingMark() {
  return <span aria-hidden className="bg-muted-foreground/20 size-4 shrink-0 rounded-full" />
}

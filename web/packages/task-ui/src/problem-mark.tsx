import { CircleAlert } from "lucide-react"

import type { Problem, ProblemCode } from "@openplan/api-client"
import { cn, Tooltip } from "@openplan/ui"

import { META_BADGE } from "./conflict-mark"

const PROBLEM_TINT = "border-danger/40 bg-danger/10 text-danger"

// The daemon finds these in the saved text, so an unsaved edit can already have fixed them.
const TEXT_CODES: ReadonlySet<ProblemCode> = new Set(["title", "comment", "diagram"])

export const withoutTextProblems = (problems: ReadonlyArray<Problem>): ReadonlyArray<Problem> =>
  problems.filter((problem) => !TEXT_CODES.has(problem.code))

export const problemCount = (count: number): string => `${count} ${count === 1 ? "problem" : "problems"}`

function ProblemMessages({ problems, className }: { problems: ReadonlyArray<Problem>; className?: string }) {
  return (
    <ul className={cn("flex list-disc flex-col gap-0.5 pl-5", className)}>
      {problems.map((problem, at) => (
        <li key={at}>{problem.message}</li>
      ))}
    </ul>
  )
}

export function ProblemBadge({ problems, className }: { problems: ReadonlyArray<Problem>; className?: string }) {
  return (
    <Tooltip content={<ProblemMessages problems={problems} className="pl-3.5" />}>
      <span tabIndex={0} className={cn(META_BADGE, PROBLEM_TINT, className)}>
        <CircleAlert aria-hidden className="size-3 shrink-0" />
        {problemCount(problems.length)}
      </span>
    </Tooltip>
  )
}

export function ProblemBanner({ problems }: { problems: ReadonlyArray<Problem> }) {
  if (problems.length === 0) return null
  return (
    <div role="note" className={cn("mb-5 flex flex-col gap-1.5 rounded-md border px-3 py-2 text-xs", PROBLEM_TINT)}>
      <p className="flex items-center gap-1.5 font-medium">
        <CircleAlert aria-hidden className="size-3.5 shrink-0" />
        {problemCount(problems.length)} in this task.
      </p>
      <ProblemMessages problems={problems} className="text-foreground/90" />
    </div>
  )
}

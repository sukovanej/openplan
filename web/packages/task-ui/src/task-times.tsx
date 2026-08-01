import { CalendarPlus, CircleAlert, History } from "lucide-react"

import type { Field_Rfc3339 } from "@open-planner/api-client"
import { MetaItem, TimeAgo, Tooltip } from "@open-planner/ui"

import { type FieldProblem, fieldFailure, fieldValue } from "./metadata"

export function TaskTimes({
  created,
  updated,
  problems,
}: {
  created: string | undefined
  updated: Field_Rfc3339 | undefined
  problems: ReadonlyArray<FieldProblem>
}) {
  const updatedAt = updated === undefined ? undefined : fieldValue(updated)
  // A commit git cannot date leaves the field carrying why instead of when, and that reads where the
  // time would have — not folded in with the frontmatter's own problems, which it is not one of.
  const undatable = updated === undefined ? undefined : fieldFailure(updated)
  return (
    <>
      {created !== undefined && (
        <MetaItem icon={CalendarPlus} className="shrink-0 whitespace-nowrap">
          <TimeAgo iso={created} label="Created" />
        </MetaItem>
      )}
      {updatedAt !== undefined && (
        <MetaItem icon={History} className="shrink-0 whitespace-nowrap">
          <TimeAgo iso={updatedAt} label="Updated" />
        </MetaItem>
      )}
      {undatable !== undefined && undatable.kind === "invalid" && (
        <Tooltip content={undatable.message} className="min-w-0">
          <MetaItem icon={History} className="text-danger/90 min-w-0">
            <span className="truncate">{undatable.message}</span>
          </MetaItem>
        </Tooltip>
      )}
      {problems.length > 0 && (
        <MetaItem icon={CircleAlert} className="text-danger/90">
          <span className="truncate">{problems.map(({ field, message }) => `${field}: ${message}`).join(", ")}</span>
        </MetaItem>
      )}
    </>
  )
}

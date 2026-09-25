import type { RevisionView } from "@openplan/api-client"
import { cn, MetaLine, TimeAgo, Tooltip } from "@openplan/ui"

import { AgentTag } from "./agent-tag"

const SHORT_REVISION = 7

export const shortRevision = (id: string): string => id.slice(0, SHORT_REVISION)

export function revisionSummary(message: string): string {
  return (
    message
      .split("\n")
      .map((line) => line.trim())
      .find((line) => line !== "") ?? ""
  )
}

export function RevisionMeta({ revision, className }: { revision: RevisionView; className?: string }) {
  const author = <span className="text-foreground/90 font-medium">{revision.author}</span>
  return (
    <MetaLine className={cn("gap-x-2", className)}>
      {revision.email === undefined ? author : <Tooltip content={revision.email}>{author}</Tooltip>}
      {revision.agent !== undefined && <AgentTag agent={revision.agent} />}
      <TimeAgo iso={revision.at} label="Changed" />
      <span className="font-mono">{shortRevision(revision.id)}</span>
    </MetaLine>
  )
}

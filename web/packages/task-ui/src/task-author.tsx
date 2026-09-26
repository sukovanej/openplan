import { UserRound } from "lucide-react"

import type { Author } from "@openplan/api-client"
import { MetaItem, Tooltip } from "@openplan/ui"

import { AgentTag } from "./agent-tag"

export function TaskAuthor({ author, withAgent }: { author: Author | undefined; withAgent: boolean }) {
  if (author === undefined) return null
  const name = (
    <MetaItem icon={UserRound} className="min-w-0 whitespace-nowrap">
      <span className="truncate">{author.name}</span>
    </MetaItem>
  )
  return (
    <>
      {author.email === undefined ? name : <Tooltip content={author.email}>{name}</Tooltip>}
      {withAgent && author.agent !== undefined && <AgentTag agent={author.agent} />}
    </>
  )
}

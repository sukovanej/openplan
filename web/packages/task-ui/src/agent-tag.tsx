import { Bot } from "lucide-react"

import { Tag, Tooltip } from "@openplan/ui"

export function AgentTag({ agent }: { agent: string }) {
  return (
    <Tag className="border-border text-muted-foreground">
      <Bot aria-hidden className="size-3" />
      <span>{agent}</span>
    </Tag>
  )
}

export function AgentMark({ agent }: { agent: string }) {
  return (
    <Tooltip content={`via ${agent}`}>
      <Bot aria-label={`via ${agent}`} className="text-muted-foreground size-3.5 shrink-0" />
    </Tooltip>
  )
}

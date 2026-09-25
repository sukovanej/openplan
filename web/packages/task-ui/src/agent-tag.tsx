import { Bot } from "lucide-react"

import { Tag } from "@openplan/ui"

export function AgentTag({ agent }: { agent: string }) {
  return (
    <Tag className="border-border text-muted-foreground">
      <Bot aria-hidden className="size-3" />
      <span>{agent}</span>
    </Tag>
  )
}

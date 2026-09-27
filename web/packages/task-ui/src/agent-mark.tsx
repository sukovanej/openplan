import { Bot } from "lucide-react"

import { Tooltip } from "@openplan/ui"

export function AgentMark({ agent }: { agent: string }) {
  return (
    <Tooltip content={`via ${agent}`}>
      <Bot aria-label={`via ${agent}`} className="text-muted-foreground size-3.5 shrink-0" />
    </Tooltip>
  )
}

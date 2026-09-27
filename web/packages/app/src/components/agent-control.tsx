import { Sparkles } from "lucide-react"

import { Button, cn, CountPill, Spinner, Tooltip } from "@openplan/ui"

import { useAgentSessions, working } from "../lib/agent-sessions"
import { overlayRequests } from "../lib/overlay-requests"

export function AgentControl() {
  const sessions = useAgentSessions()
  const busy = sessions.filter(working).length
  const approvals = sessions.reduce((sum, session) => sum + session.approvals, 0)
  return (
    <Tooltip content={tooltip(sessions.length, busy, approvals)}>
      <Button
        aria-label="Agent"
        onClick={() => overlayRequests.toggle("prompt")}
        className={cn(
          "gap-1.5 px-1.5 py-1.5",
          approvals > 0 ? "text-warning" : busy > 0 ? "text-foreground" : "text-muted-foreground/60",
        )}
      >
        {busy > 0 ? <Spinner label="Working" className="size-4" /> : <Sparkles className="size-4" aria-hidden />}
        {sessions.length > 0 && <CountPill count={sessions.length} />}
      </Button>
    </Tooltip>
  )
}

function tooltip(total: number, busy: number, approvals: number): string {
  if (total === 0) return "Ask the agent (e)"
  const parts = [`${busy} of ${total} ${total === 1 ? "session" : "sessions"} working`]
  if (approvals > 0) parts.push(`${approvals} ${approvals === 1 ? "approval" : "approvals"} waiting`)
  return `${parts.join(", ")} (e)`
}

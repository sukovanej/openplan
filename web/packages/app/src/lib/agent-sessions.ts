import { useQuery } from "@tanstack/react-query"

import { type AgentSessionSummary, listAgentSessions } from "./api"
import { agentSessionsKey } from "./query-client"
import { abortable } from "./runtime"

const NO_SESSIONS: ReadonlyArray<AgentSessionSummary> = []

export function useAgentSessions(): ReadonlyArray<AgentSessionSummary> {
  const sessions = useQuery({
    queryKey: agentSessionsKey,
    queryFn: abortable(listAgentSessions()),
  })
  return sessions.data ?? NO_SESSIONS
}

export function working(session: AgentSessionSummary): boolean {
  return session.status.kind === "starting" || session.status.kind === "running"
}

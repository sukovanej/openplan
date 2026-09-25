import { useMutation } from "@tanstack/react-query"
import { useState } from "react"

import type { SessionView } from "./agent-events"
import { type AgentSession, useAgentSession } from "./agent-session"
import { emptyTranscript, type Transcript, running } from "./agent-transcript"
import {
  type ApprovalDecision,
  approveAgentTool,
  createAgentSession,
  interruptAgentSession,
  promptAgentSession,
} from "./api"
import { runtime } from "./runtime"

export interface AgentChat {
  readonly live: AgentSession | undefined
  readonly view: SessionView | undefined
  readonly transcript: Transcript
  readonly running: boolean
  // No prompt can go: the session is still connecting, the agent is still starting, or the session
  // is gone. A stopped agent takes one: the daemon resumes it.
  readonly closed: boolean
  readonly sends: number
  readonly send: (text: string) => void
  readonly sending: boolean
  readonly stop: () => void
  readonly stopping: boolean
  readonly decide: (approval: string, decision: ApprovalDecision) => void
  readonly deciding: boolean
  readonly error: unknown
}

// The chat with one daemon session, or with the one its first prompt starts when `session` is
// undefined. `context` goes with that first prompt: the task open in front of the user.
export function useAgentChat({
  project,
  session,
  context,
  onStarted,
}: {
  project: string
  session: string | undefined
  context: string | undefined
  onStarted: (session: string) => void
}): AgentChat {
  const [sends, setSends] = useState(0)
  const live = useAgentSession(project, session)
  const view = live?.phase === "live" ? live.view : undefined

  const send = useMutation({
    mutationFn: (text: string): Promise<string> =>
      session === undefined
        ? runtime.runPromise(createAgentSession(project, text, context))
        : runtime.runPromise(promptAgentSession(project, session, text)).then(() => session),
    onSuccess: (started) => {
      if (started !== session) onStarted(started)
    },
  })
  const stop = useMutation({
    mutationFn: () =>
      session === undefined ? Promise.resolve() : runtime.runPromise(interruptAgentSession(project, session)),
  })
  const decide = useMutation({
    mutationFn: ({ approval, decision }: { approval: string; decision: ApprovalDecision }) =>
      session === undefined
        ? Promise.resolve()
        : runtime.runPromise(approveAgentTool(project, session, approval, decision)),
  })

  const transcript = view?.transcript ?? emptyTranscript
  const status = view?.status
  return {
    live,
    view,
    transcript,
    running: running(transcript),
    closed: live?.phase === "connecting" || live?.phase === "ended" || status?.kind === "starting",
    sends,
    send: (text) => {
      setSends((count) => count + 1)
      send.mutate(text)
    },
    sending: send.isPending,
    stop: () => stop.mutate(),
    stopping: stop.isPending,
    decide: (approval, decision) => decide.mutate({ approval, decision }),
    deciding: decide.isPending,
    error: send.error ?? stop.error ?? decide.error ?? undefined,
  }
}

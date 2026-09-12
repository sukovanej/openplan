import { useMutation, useQuery } from "@tanstack/react-query"
import { useEffect, useState } from "react"

import type { SessionView } from "./agent-events"
import { type AgentSession, attachableSession, useAgentSession } from "./agent-session"
import { emptyTranscript, type Transcript, running } from "./agent-transcript"
import {
  type ApprovalDecision,
  approveAgentTool,
  createAgentSession,
  interruptAgentSession,
  listAgentSessions,
  promptAgentSession,
  TaskRejected,
} from "./api"
import { agentSessionsKey } from "./query-client"
import { runtime } from "./runtime"

export interface AgentChat {
  readonly live: AgentSession | undefined
  readonly view: SessionView | undefined
  readonly transcript: Transcript
  // The session the page named is gone: the daemon restarted, or the agent was stopped elsewhere.
  readonly ended: boolean
  readonly running: boolean
  // No prompt can go: the session is still connecting, the agent is still starting, or it exited.
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

// The chat with one daemon session. The page owns the session id, because it lives in the URL;
// the chat asks for a change when it starts one, or when the one it holds is gone. `task` binds
// the session to a task on start, and a running session on that task is picked up before a new
// one starts. `onView` hands the page what the session reports: the task it wrote and the branch
// it writes on.
export function useAgentChat({
  project,
  task,
  session,
  onSession,
  onView,
}: {
  project: string
  task: string | undefined
  session: string | undefined
  onSession: (next: string | undefined) => void
  onView?: (view: SessionView) => void
}): AgentChat {
  const [ended, setEnded] = useState(false)
  const [sends, setSends] = useState(0)

  const held = useQuery({
    queryKey: agentSessionsKey(project),
    queryFn: () => runtime.runPromise(listAgentSessions(project)),
    enabled: task !== undefined && session === undefined && !ended,
    staleTime: 0,
  })
  useEffect(() => {
    if (session !== undefined || held.data === undefined) return
    const found = attachableSession(held.data, task)
    if (found !== undefined) onSession(found)
  }, [session, held.data, task, onSession])

  const live = useAgentSession(project, session)
  const view = live?.phase === "live" ? live.view : undefined
  const phase = live?.phase

  useEffect(() => {
    if (phase === "ended") {
      setEnded(true)
      onSession(undefined)
    }
  }, [phase, onSession])

  useEffect(() => {
    if (view !== undefined) onView?.(view)
  }, [view, onView])

  const send = useMutation({
    mutationFn: (text: string): Promise<string> =>
      session === undefined
        ? runtime.runPromise(createAgentSession(project, text, task))
        : runtime.runPromise(promptAgentSession(project, session, text)).then(() => session),
    onSuccess: (started) => {
      setEnded(false)
      if (started !== session) onSession(started)
    },
    onError: (error) => {
      if (error instanceof TaskRejected && error.status === 404 && session !== undefined) {
        setEnded(true)
        onSession(undefined)
      }
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
    ended,
    running: running(transcript),
    closed: live?.phase === "connecting" || status?.kind === "starting" || status?.kind === "exited",
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

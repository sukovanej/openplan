import { Schema } from "effect"
import { useEffect, useState } from "react"

import { SessionEvent, type SessionView } from "./agent-events"
import { applyAgentEvent } from "./agent-transcript"
import type { AgentSessionSummary } from "./api"

const decode = Schema.decodeUnknownSync(SessionEvent)

const RECONNECT_BASE_MS = 1000
const RECONNECT_CAP_MS = 8000

// `ended` is a session the daemon no longer holds: it answered the stream with a refusal rather than
// a snapshot. A stale link after a restart lands here.
export type AgentSession =
  | { readonly phase: "connecting" }
  | { readonly phase: "live"; readonly view: SessionView }
  | { readonly phase: "ended" }

// The session a reload comes back to: the newest one bound to the task that has not exited. A page
// for a task that does not exist yet has nothing to come back to.
export function attachableSession(
  sessions: ReadonlyArray<AgentSessionSummary>,
  task: string | undefined,
): string | undefined {
  if (task === undefined) return undefined
  return sessions.find((session) => session.task === task && session.status.kind !== "exited")?.id
}

export function fold(session: AgentSession, event: SessionEvent): AgentSession {
  switch (event.kind) {
    case "snapshot": {
      const { kind: _kind, ...view } = event
      return { phase: "live", view }
    }
    case "task":
      return session.phase === "live" ? { phase: "live", view: { ...session.view, task: event.id } } : session
    default: {
      if (session.phase !== "live") return session
      const transcript = applyAgentEvent(session.view.transcript, event)
      return { phase: "live", view: { ...session.view, transcript, status: transcript.status } }
    }
  }
}

function eventsUrl(project: string, id: string): string {
  return `/api/projects/${encodeURIComponent(project)}/agent/sessions/${encodeURIComponent(id)}/events`
}

// A second stream beside the app's one `/api/events`, open only while the page is. The snapshot it
// opens with is the transcript; every later event folds into it.
export function useAgentSession(project: string, id: string | undefined): AgentSession | undefined {
  const [session, setSession] = useState<AgentSession>({ phase: "connecting" })

  useEffect(() => {
    if (id === undefined) return
    setSession({ phase: "connecting" })
    let source: EventSource | undefined
    let timer: ReturnType<typeof setTimeout> | undefined
    let attempts = 0

    const connect = () => {
      source = new EventSource(eventsUrl(project, id))
      source.onopen = () => {
        attempts = 0
      }
      source.onmessage = (message: MessageEvent<string>) => {
        let event: SessionEvent
        try {
          event = decode(JSON.parse(message.data))
        } catch {
          return
        }
        setSession((current) => fold(current, event))
      }
      source.onerror = () => {
        // The browser gives up on an answer that is not a stream, which is the 404 for a session the
        // daemon no longer holds, and would retry a dropped connection on its own schedule. It is
        // closed here so the backoff stays ours; what it reported first tells the two apart.
        const refused = source?.readyState === EventSource.CLOSED
        source?.close()
        source = undefined
        if (refused) {
          setSession({ phase: "ended" })
          return
        }
        timer = setTimeout(connect, Math.min(RECONNECT_BASE_MS * 2 ** attempts, RECONNECT_CAP_MS))
        attempts += 1
      }
    }
    connect()

    return () => {
      clearTimeout(timer)
      source?.close()
    }
  }, [project, id])

  return id === undefined ? undefined : session
}

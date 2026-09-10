import { useMutation, useQuery } from "@tanstack/react-query"
import { type ReactNode, useEffect, useState } from "react"

import { Button, cn, Panel, PanelHeader, PanelTitle, Spinner } from "@openplan/ui"

import type { AgentKind, SessionView } from "../lib/agent-events"
import { type AgentSession, attachableSession, useAgentSession } from "../lib/agent-session"
import { emptyTranscript, running } from "../lib/agent-transcript"
import {
  type ApprovalDecision,
  approveAgentTool,
  createAgentSession,
  interruptAgentSession,
  listAgentSessions,
  promptAgentSession,
  TaskRejected,
} from "../lib/api"
import { errorText } from "../lib/format"
import { useAbbreviation } from "../lib/projects"
import { agentSessionsKey } from "../lib/query-client"
import { runtime } from "../lib/runtime"
import { ChatLog, Composer, StopButton } from "./agent-chat"

const agentNames: Record<AgentKind, string> = {
  claude_code: "Claude Code",
  codex: "Codex",
}

// The chat with one daemon session. The page owns the session id, because it lives in the URL;
// the panel asks for a change when it starts one, or when the one it holds is gone. `task` binds
// the session to a task on start. `onView` hands the page what the session reports: the task it
// wrote and the branch it writes on.
export function AgentPanel({
  project,
  task,
  session,
  onSession,
  onView,
  lead,
  className,
}: {
  project: string
  task: string | undefined
  session: string | undefined
  onSession: (next: string | undefined) => void
  onView?: (view: SessionView) => void
  lead?: ReactNode
  className?: string
}) {
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
  const composerDisabled = live?.phase === "connecting" || status?.kind === "starting" || status?.kind === "exited"
  const abbreviation = useAbbreviation(project)

  return (
    <Panel className={cn("min-w-0", className)}>
      <PanelHeader className="gap-3">
        <PanelTitle>Agent</PanelTitle>
        {lead}
        <SessionState live={live} ended={ended} />
        {running(transcript) && <StopButton stopping={stop.isPending} onStop={() => stop.mutate()} />}
      </PanelHeader>
      {status?.kind === "exited" && <Exited code={status.code} onRestart={() => onSession(undefined)} />}
      {transcript.over_budget && (
        <p className="text-warning border-b px-4 py-2 text-xs">The session spent its budget.</p>
      )}
      <ChatLog
        project={project}
        abbreviation={abbreviation}
        transcript={transcript}
        sends={sends}
        onDecide={(approval, decision) => decide.mutate({ approval, decision })}
        deciding={decide.isPending}
      />
      {(send.isError || stop.isError || decide.isError) && (
        <p className="text-warning border-t px-4 py-2 text-xs">{errorText(send.error ?? stop.error ?? decide.error)}</p>
      )}
      <Composer
        disabled={composerDisabled}
        sending={send.isPending}
        live={running(transcript)}
        usage={view === undefined ? undefined : transcript.usage}
        onSend={(text) => {
          setSends((count) => count + 1)
          send.mutate(text)
        }}
      />
    </Panel>
  )
}

function SessionState({ live, ended }: { live: AgentSession | undefined; ended: boolean }) {
  if (live === undefined) {
    return <span className="text-muted-foreground text-xs">{ended ? "This session has ended" : "New session"}</span>
  }
  if (live.phase === "connecting") return <Waiting label="Connecting" />
  if (live.phase === "ended") return <span className="text-muted-foreground text-xs">This session has ended</span>
  return <Model view={live.view} />
}

function Model({ view }: { view: SessionView }) {
  if (view.status.kind === "starting") return <Waiting label="Starting the agent" />
  const name = view.transcript.info?.model ?? agentNames[view.agent]
  return (
    <span className="flex min-w-0 items-center gap-1.5 text-xs">
      <span
        className={
          view.status.kind === "exited" ? "bg-muted-foreground/40 size-2 rounded-full" : "bg-info size-2 rounded-full"
        }
      />
      <span className="text-muted-foreground truncate normal-case">{name}</span>
    </span>
  )
}

function Waiting({ label }: { label: string }) {
  return (
    <span className="text-info flex items-center gap-1.5 text-xs">
      <Spinner label={label} className="size-3.5" />
      {label}
    </span>
  )
}

function Exited({ code, onRestart }: { code: number | null; onRestart: () => void }) {
  return (
    <div className="flex items-center gap-2 border-b px-4 py-2 text-xs">
      <span className="text-muted-foreground">The agent exited{code !== null && ` with code ${code}`}.</span>
      <Button variant="accent" onClick={onRestart} className="ml-auto">
        Start a new session
      </Button>
    </div>
  )
}

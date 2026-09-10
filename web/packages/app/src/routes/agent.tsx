import { useMutation, useQuery } from "@tanstack/react-query"
import { useCallback, useEffect, useState } from "react"
import { useNavigate, useParams, useSearchParams } from "react-router-dom"

import { agentPath } from "@openplan/task-ui"
import { Button, EmptyState, Panel, PanelHeader, PanelTitle, Skeleton, Spinner } from "@openplan/ui"

import { ChatLog, Composer, StopButton } from "../components/agent-chat"
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
import { TaskView } from "./task-view"

const SESSION_PARAM = "session"

const agentNames: Record<AgentKind, string> = {
  claude_code: "Claude Code",
  codex: "Codex",
}

export function AgentRoute() {
  const { project = "", id } = useParams()
  // Keyed by the project alone: the page moves from the new-task route to the task's own route
  // when the session binds, and the chat must stay mounted across that move.
  return <AgentPage key={project} project={project} id={id} />
}

interface Pinned {
  readonly branch: string | undefined
}

function AgentPage({ project, id }: { project: string; id: string | undefined }) {
  const navigate = useNavigate()
  const [params, setParams] = useSearchParams()
  const session = params.get(SESSION_PARAM) ?? undefined
  const setSession = useCallback(
    (next: string | undefined) => setParams(next === undefined ? {} : { [SESSION_PARAM]: next }, { replace: true }),
    [setParams],
  )
  const [ended, setEnded] = useState(false)
  const [sends, setSends] = useState(0)
  // The preview shows the branch the agent writes until the reader pins another. Local, not in the
  // URL: the URL names the session, and the branch is the session's.
  const [pinned, setPinned] = useState<Pinned>()

  const held = useQuery({
    queryKey: agentSessionsKey(project),
    queryFn: () => runtime.runPromise(listAgentSessions(project)),
    enabled: id !== undefined && session === undefined && !ended,
    staleTime: 0,
  })
  useEffect(() => {
    if (session !== undefined || held.data === undefined) return
    const found = attachableSession(held.data, id)
    if (found !== undefined) setSession(found)
  }, [session, held.data, id, setSession])

  const live = useAgentSession(project, session)
  const view = live?.phase === "live" ? live.view : undefined
  const task = view?.task ?? undefined
  const phase = live?.phase

  useEffect(() => {
    if (phase === "ended") {
      setEnded(true)
      setSession(undefined)
    }
  }, [phase, setSession])

  useEffect(() => {
    if (id === undefined && task !== undefined) navigate(agentPath(project, task, session), { replace: true })
  }, [id, task, project, session, navigate])

  const send = useMutation({
    mutationFn: (text: string): Promise<string> =>
      session === undefined
        ? runtime.runPromise(createAgentSession(project, text, id))
        : runtime.runPromise(promptAgentSession(project, session, text)).then(() => session),
    onSuccess: (started) => {
      setEnded(false)
      if (started !== session) setSession(started)
    },
    onError: (error) => {
      if (error instanceof TaskRejected && error.status === 404 && session !== undefined) {
        setEnded(true)
        setSession(undefined)
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
  const branch = pinned === undefined ? view?.branch : pinned.branch

  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto lg:flex-row lg:overflow-hidden">
      {/* The task keeps the width it has on its own page; the chat takes what is left. */}
      <div className="min-w-0 lg:h-full lg:w-[59rem] lg:shrink-0 lg:overflow-hidden">
        {id === undefined ? (
          <NoTaskYet writing={running(transcript)} />
        ) : (
          <TaskView
            project={project}
            id={id}
            branch={branch}
            onSelect={(next) => setPinned({ branch: next })}
            stacked
          />
        )}
      </div>
      <Panel className="h-[70vh] min-w-0 lg:h-full lg:min-w-[20rem] lg:flex-1">
        <PanelHeader className="gap-3">
          <PanelTitle>Agent</PanelTitle>
          <SessionState live={live} ended={ended} />
          {running(transcript) && <StopButton stopping={stop.isPending} onStop={() => stop.mutate()} />}
        </PanelHeader>
        {status?.kind === "exited" && <Exited code={status.code} onRestart={() => setSession(undefined)} />}
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
          <p className="text-warning border-t px-4 py-2 text-xs">
            {errorText(send.error ?? stop.error ?? decide.error)}
          </p>
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
    </div>
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

function NoTaskYet({ writing }: { writing: boolean }) {
  return (
    <div className="space-y-4">
      <EmptyState
        title="The agent has not written a task yet"
        detail={writing ? "The preview mounts as soon as it does." : "Describe what you want in the chat."}
      />
      {writing && (
        <div className="space-y-4">
          <Skeleton className="h-8 w-2/3" />
          <Skeleton className="h-5 w-24" />
          <Skeleton className="h-40 w-full" />
        </div>
      )}
    </div>
  )
}

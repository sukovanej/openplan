import type { ReactNode } from "react"

import { cn, Panel, PanelHeader, PanelTitle } from "@openplan/ui"

import type { SessionView } from "../lib/agent-events"
import { errorText } from "../lib/format"
import { useAbbreviation } from "../lib/projects"
import { useAgentChat } from "../lib/use-agent-chat"
import { ChatLog, Composer, Exited, SessionState, StopButton, UsageLine } from "./agent-chat"

// The chat as a box of its own, with the whole transcript in view: the draft page, where there is
// no task to look at yet. `lead` sits in the header after the title.
export function AgentPanel({
  project,
  session,
  onSession,
  onView,
  lead,
  className,
}: {
  project: string
  session: string | undefined
  onSession: (next: string | undefined) => void
  onView?: (view: SessionView) => void
  lead?: ReactNode
  className?: string
}) {
  const chat = useAgentChat({ project, task: undefined, session, onSession, onView })
  const status = chat.view?.status
  const abbreviation = useAbbreviation(project)

  return (
    <Panel className={cn("min-w-0", className)}>
      <PanelHeader className="gap-3">
        <PanelTitle>Agent</PanelTitle>
        {lead}
        <SessionState live={chat.live} ended={chat.ended} />
        {chat.running && <StopButton stopping={chat.stopping} onStop={chat.stop} className="ml-auto" />}
      </PanelHeader>
      {status?.kind === "exited" && <Exited code={status.code} onRestart={() => onSession(undefined)} />}
      {chat.transcript.over_budget && (
        <p className="text-warning border-b px-4 py-2 text-xs">The session spent its budget.</p>
      )}
      <ChatLog
        project={project}
        abbreviation={abbreviation}
        transcript={chat.transcript}
        sends={chat.sends}
        onDecide={chat.decide}
        deciding={chat.deciding}
      />
      {chat.error !== undefined && <p className="text-warning border-t px-4 py-2 text-xs">{errorText(chat.error)}</p>}
      <div className="flex shrink-0 flex-col gap-1.5 border-t px-4 py-3">
        <Composer disabled={chat.closed} sending={chat.sending} live={chat.running} onSend={chat.send} />
        {chat.view !== undefined && <UsageLine usage={chat.transcript.usage} />}
      </div>
    </Panel>
  )
}

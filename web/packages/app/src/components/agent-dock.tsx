import { ScrollText } from "lucide-react"
import { useState } from "react"

import { Button, Dialog, Spinner } from "@openplan/ui"

import { type Latest, latest } from "../lib/agent-activity"
import type { SessionView } from "../lib/agent-events"
import { errorText } from "../lib/format"
import { useAbbreviation } from "../lib/projects"
import { useAgentChat } from "../lib/use-agent-chat"
import { ChatLog, Composer, SessionState, StopButton, Waiting } from "./agent-chat"

// The chat at the foot of the task box: the prompt, one line under it with what the agent is doing
// or last said, and a way to the whole transcript. The task above is the output that matters; the
// transcript is there for when a reader wants the working.
export function AgentDock({
  project,
  task,
  session,
  onSession,
  onView,
}: {
  project: string
  task: string
  session: string | undefined
  onSession: (next: string | undefined) => void
  onView?: (view: SessionView) => void
}) {
  const chat = useAgentChat({ project, task, session, onSession, onView })
  const [transcriptOpen, setTranscriptOpen] = useState(false)
  const abbreviation = useAbbreviation(project)
  const status = chat.view?.status
  const line = latest(chat.transcript)

  return (
    <div className="flex shrink-0 flex-col gap-2 border-t px-6 py-3">
      <Composer disabled={chat.closed} sending={chat.sending} live={chat.running} onSend={chat.send} />
      <div className="flex min-h-5 items-center gap-3 text-xs">
        <div className="min-w-0 flex-1">
          {status?.kind === "exited" ? (
            <span className="flex items-center gap-2">
              <span className="text-muted-foreground">
                The agent exited{status.code !== null && ` with code ${status.code}`}.
              </span>
              <Button variant="accent" onClick={() => onSession(undefined)}>
                Start a new session
              </Button>
            </span>
          ) : chat.error !== undefined ? (
            <span className="text-warning">{errorText(chat.error)}</span>
          ) : line.kind === "none" ? (
            <SessionState live={chat.live} ended={chat.ended} />
          ) : (
            <LatestLine line={line} onOpen={() => setTranscriptOpen(true)} />
          )}
        </div>
        {chat.running && <StopButton stopping={chat.stopping} onStop={chat.stop} />}
        <Button
          size="icon"
          aria-label="Transcript"
          title="Transcript"
          disabled={chat.view === undefined}
          onClick={() => setTranscriptOpen(true)}
        >
          <ScrollText className="size-4" aria-hidden />
        </Button>
      </div>
      <Dialog
        open={transcriptOpen}
        onClose={() => setTranscriptOpen(false)}
        title="Transcript"
        className="flex max-h-[85vh] max-w-3xl flex-col"
      >
        <ChatLog
          project={project}
          abbreviation={abbreviation}
          transcript={chat.transcript}
          sends={chat.sends}
          onDecide={chat.decide}
          deciding={chat.deciding}
          className="-mx-6 -mb-6 border-t"
        />
      </Dialog>
    </div>
  )
}

function LatestLine({ line, onOpen }: { line: Latest; onOpen: () => void }) {
  switch (line.kind) {
    case "none":
      return null
    case "approval":
      return (
        <button type="button" onClick={onOpen} className="text-warning cursor-pointer hover:underline">
          {line.count === 1 ? "The agent needs your approval" : `The agent needs ${line.count} approvals`}
        </button>
      )
    case "step":
      return <Waiting label={line.phrase} />
    case "message":
      return (
        <span className="text-muted-foreground flex min-w-0 items-center gap-1.5">
          {!line.done && <Spinner label="Writing" className="size-3.5" />}
          <span className="line-clamp-2 min-w-0 whitespace-pre-wrap">{line.text}</span>
        </span>
      )
    case "failure":
      return <span className="text-warning">{line.message}</span>
  }
}

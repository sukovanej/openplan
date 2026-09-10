import { ChevronRight, Square } from "lucide-react"
import { type KeyboardEvent, useEffect, useLayoutEffect, useRef, useState } from "react"

import { TaskBody } from "@openplan/task-ui"
import { Button, cn, PanelBody, Spinner } from "@openplan/ui"

import { type ChatItem, chatItems, newest, phrase, type Step } from "../lib/agent-activity"
import type { AgentKind, ApprovalRequest, SessionView, Transcript, Usage } from "../lib/agent-events"
import type { AgentSession } from "../lib/agent-session"
import { running } from "../lib/agent-transcript"
import type { ApprovalDecision } from "../lib/api"
import { useDetailAction } from "../lib/detail-actions"

const agentNames: Record<AgentKind, string> = {
  claude_code: "Claude Code",
  codex: "Codex",
}

const FOLLOW_SLACK_PX = 8

// The transcript as a reader sees it. It follows the newest text as it streams, unless the reader
// scrolled up to read something older; `sends` counts the prompts sent, and each one brings the
// log back to following.
export function ChatLog({
  project,
  abbreviation,
  transcript,
  sends,
  onDecide,
  deciding,
  className,
}: {
  project: string
  abbreviation: string | undefined
  transcript: Transcript
  sends: number
  onDecide: (approval: string, decision: ApprovalDecision) => void
  deciding: boolean
  className?: string
}) {
  const body = useRef<HTMLDivElement>(null)
  const following = useRef(true)
  const items = chatItems(transcript.entries)
  const live = running(transcript)
  const last = items[items.length - 1]
  const tail =
    last === undefined
      ? ""
      : last.kind === "message"
        ? last.text
        : String(last.kind === "activity" ? last.steps.length : 0)

  useEffect(() => {
    following.current = true
  }, [sends])

  useLayoutEffect(() => {
    const node = body.current
    if (node !== null && following.current) node.scrollTop = node.scrollHeight
  }, [items.length, tail, transcript.approvals.length, sends])

  return (
    <PanelBody
      ref={body}
      className={cn("flex flex-col gap-3 px-4 py-4", className)}
      onScroll={() => {
        const node = body.current
        if (node === null) return
        following.current = node.scrollHeight - node.scrollTop - node.clientHeight < FOLLOW_SLACK_PX
      }}
    >
      {items.map((item, at) => (
        <ChatRow
          key={at}
          project={project}
          abbreviation={abbreviation}
          item={item}
          open={live && at === items.length - 1}
        />
      ))}
      {transcript.approvals.map((request) => (
        <ApprovalCard key={request.id} request={request} onDecide={onDecide} deciding={deciding} />
      ))}
    </PanelBody>
  )
}

function ChatRow({
  project,
  abbreviation,
  item,
  open,
}: {
  project: string
  abbreviation: string | undefined
  item: ChatItem
  open: boolean
}) {
  switch (item.kind) {
    case "prompt":
      return <div className="bg-muted ml-8 self-end rounded-lg px-3 py-2 text-sm whitespace-pre-wrap">{item.text}</div>
    case "message":
      return (
        <div className="mr-4 min-w-0">
          <TaskBody
            project={project}
            markdown={item.text}
            abbreviation={abbreviation}
            // The chat is on the page's own face, not the task body's serif: it is talk, not a
            // document.
            className="font-sans text-sm leading-6 dark:[font-weight:400] prose-p:my-2 prose-ul:my-2 prose-ol:my-2 prose-headings:mt-4 prose-headings:mb-1 prose-h2:border-t-0 prose-h2:pt-0 prose-h2:text-base prose-h3:text-sm"
            data-keys-ignore
          />
          {!item.done && <span className="bg-info ml-0.5 inline-block h-4 w-0.5 animate-pulse align-text-bottom" />}
        </div>
      )
    case "activity":
      return open ? <LiveActivity steps={item.steps} /> : <FinishedActivity steps={item.steps} />
    case "failure":
      return (
        <p className="text-warning text-xs">
          {item.message}
          {item.retrying && <span className="text-muted-foreground"> · retrying</span>}
        </p>
      )
  }
}

function LiveActivity({ steps }: { steps: ReadonlyArray<Step> }) {
  const step = newest(steps)
  return (
    <div className="text-info flex items-center gap-2 text-xs">
      <Spinner label={step.phrase} className="size-3.5" />
      <span>{step.phrase}</span>
    </div>
  )
}

function FinishedActivity({ steps }: { steps: ReadonlyArray<Step> }) {
  const [expanded, setExpanded] = useState(false)
  const failed = steps.some((step) => step.failed)
  return (
    <div className="text-muted-foreground text-xs">
      <button
        type="button"
        aria-expanded={expanded}
        onClick={() => setExpanded((open) => !open)}
        className="hover:text-foreground inline-flex items-center gap-1"
      >
        <ChevronRight className={cn("size-3.5 transition-transform", expanded && "rotate-90")} aria-hidden />
        <span className={cn(failed && "text-warning")}>
          {steps.length} {steps.length === 1 ? "step" : "steps"}
        </span>
      </button>
      {expanded && (
        <ul className="mt-1 ml-4.5 flex flex-col gap-0.5">
          {steps.map((step, at) => (
            <li key={at} className={cn(step.failed && "text-warning")}>
              {step.phrase}
              {step.failed && " · failed"}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

// What the agent is asking to do, in the words of its activity row, plus the command or the path
// that a person decides on. It leaves when the decision arrives or the turn ends.
function ApprovalCard({
  request,
  onDecide,
  deciding,
}: {
  request: ApprovalRequest
  onDecide: (approval: string, decision: ApprovalDecision) => void
  deciding: boolean
}) {
  const detail = approvalDetail(request.input)
  return (
    <div className="border-warning/40 bg-warning/5 flex flex-col gap-2 rounded-md border px-3 py-2 text-xs">
      <p className="text-warning">
        {phrase({
          kind: "tool",
          item: "",
          name: request.tool,
          input: request.input,
          output: "",
          done: false,
          failed: false,
        })}
      </p>
      {detail !== undefined && <code className="text-foreground/90 font-mono break-all">{detail}</code>}
      {request.reason !== null && <p className="text-muted-foreground">{request.reason}</p>}
      <div className="flex items-center gap-1">
        <Button variant="accent" disabled={deciding} onClick={() => onDecide(request.id, "allow")}>
          Allow
        </Button>
        <Button disabled={deciding} onClick={() => onDecide(request.id, "allow_for_session")}>
          Allow for this session
        </Button>
        <Button variant="danger" disabled={deciding} onClick={() => onDecide(request.id, { deny: { reason: null } })}>
          Deny
        </Button>
      </div>
    </div>
  )
}

function approvalDetail(input: unknown): string | undefined {
  if (typeof input !== "object" || input === null) return undefined
  const fields = input as Record<string, unknown>
  const command = fields.command
  if (typeof command === "string") return command
  if (Array.isArray(command)) return command.join(" ")
  const path = fields.file_path ?? fields.path
  return typeof path === "string" ? path : undefined
}

// Enter sends and Shift+Enter breaks the line; there is no send button. The input takes the focus
// when it opens and when `e` is pressed on the page; Escape hands the keyboard back to the page.
// While a turn runs, Enter waits: the Stop button is beside the input.
export function Composer({
  disabled,
  sending,
  live,
  onSend,
}: {
  disabled: boolean
  sending: boolean
  live: boolean
  onSend: (text: string) => void
}) {
  const [text, setText] = useState("")
  const area = useRef<HTMLTextAreaElement>(null)
  useDetailAction("focus-prompt", () => area.current?.focus())

  const send = () => {
    const trimmed = text.trim()
    if (trimmed === "" || disabled || sending) return
    onSend(trimmed)
    setText("")
  }

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Escape") {
      event.currentTarget.blur()
      return
    }
    if (event.key !== "Enter" || event.shiftKey) return
    event.preventDefault()
    if (live) return
    send()
  }

  return (
    <textarea
      ref={area}
      value={text}
      rows={2}
      disabled={disabled}
      placeholder={disabled ? "The agent is not ready" : "Describe the task, or ask for a change"}
      aria-label="Prompt"
      autoFocus
      onChange={(event) => setText(event.target.value)}
      onKeyDown={onKeyDown}
      className="bg-background min-h-12 w-full resize-none rounded-md border px-3 py-2 text-sm outline-none disabled:opacity-50"
    />
  )
}

export function StopButton({
  stopping,
  onStop,
  className,
}: {
  stopping: boolean
  onStop: () => void
  className?: string
}) {
  return (
    <Button variant="danger" disabled={stopping} onClick={onStop} className={cn("text-danger gap-1.5", className)}>
      <Square className="size-3" aria-hidden />
      Stop
    </Button>
  )
}

export function SessionState({ live, ended }: { live: AgentSession | undefined; ended: boolean }) {
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

export function Waiting({ label }: { label: string }) {
  return (
    <span className="text-info flex items-center gap-1.5 text-xs">
      <Spinner label={label} className="size-3.5" />
      {label}
    </span>
  )
}

export function Exited({ code, onRestart }: { code: number | null; onRestart: () => void }) {
  return (
    <div className="flex items-center gap-2 border-b px-4 py-2 text-xs">
      <span className="text-muted-foreground">The agent exited{code !== null && ` with code ${code}`}.</span>
      <Button variant="accent" onClick={onRestart} className="ml-auto">
        Start a new session
      </Button>
    </div>
  )
}

export function UsageLine({ usage }: { usage: Usage }) {
  const total = usage.input_tokens + usage.cache_write_tokens + usage.output_tokens
  return (
    <p className="text-muted-foreground text-xs tabular-nums">
      {total.toLocaleString()} tokens
      {usage.cost_usd !== null && ` · $${usage.cost_usd.toFixed(2)}`}
    </p>
  )
}

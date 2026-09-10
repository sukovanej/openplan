import { ChevronRight, Square } from "lucide-react"
import { type KeyboardEvent, useEffect, useLayoutEffect, useRef, useState } from "react"

import { TaskBody } from "@openplan/task-ui"
import { Button, cn, PanelBody, Spinner } from "@openplan/ui"

import { type ChatItem, chatItems, newest, phrase, type Step } from "../lib/agent-activity"
import type { ApprovalRequest, Transcript, Usage } from "../lib/agent-events"
import { running } from "../lib/agent-transcript"
import type { ApprovalDecision } from "../lib/api"
import { useDetailAction } from "../lib/detail-actions"

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
}: {
  project: string
  abbreviation: string | undefined
  transcript: Transcript
  sends: number
  onDecide: (approval: string, decision: ApprovalDecision) => void
  deciding: boolean
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
      className="flex flex-col gap-3 px-4 py-4"
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
// when the page opens and when `e` is pressed on it; Escape hands the keyboard back to the page.
// While a turn runs, Enter waits: the Stop button is in the panel header.
export function Composer({
  disabled,
  sending,
  live,
  usage,
  onSend,
}: {
  disabled: boolean
  sending: boolean
  live: boolean
  usage: Usage | undefined
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
    <div className="flex shrink-0 flex-col gap-1.5 border-t px-4 py-3">
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
      {usage !== undefined && <UsageLine usage={usage} />}
    </div>
  )
}

export function StopButton({ stopping, onStop }: { stopping: boolean; onStop: () => void }) {
  return (
    <Button variant="danger" disabled={stopping} onClick={onStop} className="text-danger ml-auto gap-1.5">
      <Square className="size-3" aria-hidden />
      Stop
    </Button>
  )
}

function UsageLine({ usage }: { usage: Usage }) {
  const total = usage.input_tokens + usage.cache_write_tokens + usage.output_tokens
  return (
    <p className="text-muted-foreground text-xs tabular-nums">
      {total.toLocaleString()} tokens
      {usage.cost_usd !== null && ` · $${usage.cost_usd.toFixed(2)}`}
    </p>
  )
}

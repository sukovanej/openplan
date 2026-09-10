import type { AgentEvent, Entry, Status, Transcript } from "./agent-events"

export type { AgentEvent, Entry, Status, Transcript } from "./agent-events"

export const emptyTranscript: Transcript = {
  info: null,
  status: { kind: "starting" },
  turn: null,
  entries: [],
  approvals: [],
  result_text: "",
  result: null,
  usage: {
    input_tokens: 0,
    cached_input_tokens: 0,
    cache_write_tokens: 0,
    output_tokens: 0,
    reasoning_tokens: 0,
    cost_usd: null,
  },
  context_window: null,
  rate_limit: null,
  over_budget: false,
}

export function running(transcript: Transcript): boolean {
  return transcript.status.kind === "running"
}

// The same fold as `Transcript::apply` in `op-agent`, so a page that joined late and one that saw
// every event draw the same thing.
export function applyAgentEvent(transcript: Transcript, event: AgentEvent): Transcript {
  switch (event.event) {
    case "ready": {
      const info = { session: event.session, cwd: event.cwd, model: event.model, tools: event.tools }
      const status: Status = transcript.status.kind === "starting" ? { kind: "idle" } : transcript.status
      return { ...transcript, info, status }
    }
    case "turn_started":
      return {
        ...transcript,
        status: { kind: "running" },
        turn: event.turn,
        result_text: "",
        entries: [...transcript.entries, { kind: "prompt", text: event.prompt }],
      }
    case "thinking":
      return { ...transcript, entries: appended(transcript.entries, event.item, "thinking", event.delta) }
    case "thinking_ended":
      return {
        ...transcript,
        entries: finished(replaced(transcript.entries, event.item, "thinking", event.text), event.item),
      }
    case "message":
      return { ...transcript, entries: appended(transcript.entries, event.item, "message", event.delta) }
    case "message_ended":
      return {
        ...transcript,
        entries: finished(replaced(transcript.entries, event.item, "message", event.text), event.item),
      }
    case "result_delta":
      return { ...transcript, result_text: transcript.result_text + event.delta }
    case "result_ready":
      return { ...transcript, result: event.value }
    case "tool_started":
      return {
        ...transcript,
        entries: [
          ...transcript.entries,
          {
            kind: "tool",
            item: event.item,
            name: event.name,
            input: event.input,
            output: "",
            done: false,
            failed: false,
          },
        ],
      }
    case "tool_output":
      return {
        ...transcript,
        entries: updated(transcript.entries, event.item, (entry) =>
          entry.kind === "tool" ? { ...entry, output: entry.output + event.delta } : entry,
        ),
      }
    case "tool_ended":
      return {
        ...transcript,
        entries: updated(transcript.entries, event.item, (entry) =>
          entry.kind === "tool"
            ? // A backend that streamed the output reports it again at the end, and one that did not
              // reports it only here; an empty report keeps what streamed.
              { ...entry, output: event.output === "" ? entry.output : event.output, done: true, failed: event.failed }
            : entry,
        ),
      }
    case "approval_requested": {
      const { id, item, tool, input, reason } = event
      return { ...transcript, approvals: [...transcript.approvals, { id, item, tool, input, reason }] }
    }
    case "approval_resolved":
      return { ...transcript, approvals: transcript.approvals.filter((request) => request.id !== event.id) }
    case "usage_updated":
      return { ...transcript, usage: event.session, context_window: event.context_window }
    case "rate_limit":
      return { ...transcript, rate_limit: { windows: event.windows } }
    case "budget_exhausted":
      return { ...transcript, usage: event.session, over_budget: true }
    case "turn_ended":
      return settled(transcript, { kind: "idle" })
    case "failed":
      return {
        ...transcript,
        entries: [...transcript.entries, { kind: "failure", message: event.message, retrying: event.retrying }],
      }
    case "exited":
      return settled(transcript, { kind: "exited", code: event.code })
  }
}

function itemOf(entry: Entry): string | undefined {
  return entry.kind === "thinking" || entry.kind === "message" || entry.kind === "tool" ? entry.item : undefined
}

function lastIndexOf(entries: ReadonlyArray<Entry>, item: string): number {
  for (let at = entries.length - 1; at >= 0; at--) {
    if (itemOf(entries[at]) === item) return at
  }
  return -1
}

function updated(entries: ReadonlyArray<Entry>, item: string, change: (entry: Entry) => Entry): ReadonlyArray<Entry> {
  const at = lastIndexOf(entries, item)
  if (at === -1) return entries
  return entries.map((entry, index) => (index === at ? change(entry) : entry))
}

// A delta for an item the transcript has not seen opens a text entry of the asked kind; one it has
// seen grows that entry, whatever its kind, the way the Rust fold does.
function appended(
  entries: ReadonlyArray<Entry>,
  item: string,
  kind: "thinking" | "message",
  delta: string,
): ReadonlyArray<Entry> {
  if (lastIndexOf(entries, item) === -1) return [...entries, { kind, item, text: delta, done: false }]
  return updated(entries, item, (entry) => withText(entry, (text) => text + delta))
}

function replaced(
  entries: ReadonlyArray<Entry>,
  item: string,
  kind: "thinking" | "message",
  text: string,
): ReadonlyArray<Entry> {
  if (lastIndexOf(entries, item) === -1) return [...entries, { kind, item, text, done: false }]
  return updated(entries, item, (entry) => withText(entry, () => text))
}

function withText(entry: Entry, change: (text: string) => string): Entry {
  switch (entry.kind) {
    case "prompt":
    case "thinking":
    case "message":
      return { ...entry, text: change(entry.text) }
    case "tool":
      return { ...entry, output: change(entry.output) }
    case "failure":
      return { ...entry, message: change(entry.message) }
  }
}

function finish(entry: Entry): Entry {
  return entry.kind === "thinking" || entry.kind === "message" || entry.kind === "tool"
    ? { ...entry, done: true }
    : entry
}

function finished(entries: ReadonlyArray<Entry>, item: string): ReadonlyArray<Entry> {
  return updated(entries, item, finish)
}

function settled(transcript: Transcript, status: Status): Transcript {
  return { ...transcript, status, turn: null, approvals: [], entries: transcript.entries.map(finish) }
}

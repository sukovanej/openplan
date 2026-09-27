import type { Entry } from "./agent-events"

// What the chat says the agent is doing, in a verb phrase a reader who knows no tool names can
// follow. Nothing here is a raw tool name or a JSON input.
export function phrase(entry: Entry): string {
  if (entry.kind === "thinking") return "Thinking"
  if (entry.kind !== "tool") return "Working"
  switch (entry.name) {
    case "Read":
      return named("Reading", fileName(entry.input))
    case "Grep":
    case "Glob":
    case "web_search":
      return "Searching"
    case "Edit":
    case "Write":
    case "apply_patch":
      return named("Writing", fileName(entry.input))
    case "Bash":
    case "shell":
      return named("Running", commandName(entry.input))
    default:
      return "Working"
  }
}

export interface Step {
  readonly phrase: string
  readonly done: boolean
  readonly failed: boolean
}

// One prompt, one message, one failure, or one run of thinking and tools, in transcript order.
export type ChatItem =
  | { readonly kind: "prompt"; readonly text: string }
  | { readonly kind: "message"; readonly item: string; readonly text: string; readonly done: boolean }
  | { readonly kind: "activity"; readonly steps: ReadonlyArray<Step> }
  | { readonly kind: "failure"; readonly message: string; readonly retrying: boolean }

// Thinking and tool entries between two messages fold into one activity row. A message, a prompt,
// or a failure closes the run.
export function chatItems(entries: ReadonlyArray<Entry>): ReadonlyArray<ChatItem> {
  const items: Array<ChatItem> = []
  let run: Array<Step> | undefined
  for (const entry of entries) {
    if (entry.kind === "thinking" || entry.kind === "tool") {
      const step = { phrase: phrase(entry), done: entry.done, failed: entry.kind === "tool" && entry.failed }
      if (run === undefined) {
        run = [step]
        items.push({ kind: "activity", steps: run })
      } else {
        run.push(step)
      }
      continue
    }
    run = undefined
    if (entry.kind === "prompt") items.push({ kind: "prompt", text: entry.text })
    else if (entry.kind === "message")
      items.push({ kind: "message", item: entry.item, text: entry.text, done: entry.done })
    else items.push({ kind: "failure", message: entry.message, retrying: entry.retrying })
  }
  return items
}

export function newest(steps: ReadonlyArray<Step>): Step {
  return steps[steps.length - 1]
}

function named(verb: string, name: string | undefined): string {
  return name === undefined ? verb : `${verb} ${name}`
}

function field(input: unknown, key: string): unknown {
  return typeof input === "object" && input !== null ? (input as Record<string, unknown>)[key] : undefined
}

function lastSegment(path: string): string | undefined {
  const segments = path.split("/").filter((segment) => segment !== "")
  return segments.length === 0 ? undefined : segments[segments.length - 1]
}

// Claude Code names a file `file_path`; Codex names it `path`.
function fileName(input: unknown): string | undefined {
  const path = field(input, "file_path") ?? field(input, "path")
  return typeof path === "string" ? lastSegment(path) : undefined
}

// The first word of the command, as its own name rather than its path: the daemon's binary is
// named by an absolute path, and that reads as "Running openplan".
function commandName(input: unknown): string | undefined {
  const command = field(input, "command")
  const text = typeof command === "string" ? command : Array.isArray(command) ? command.join(" ") : undefined
  const first = text?.trim().split(/\s+/, 1)[0]
  return first === undefined || first === "" ? undefined : lastSegment(first)
}

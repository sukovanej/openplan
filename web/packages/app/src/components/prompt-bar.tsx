import { useMutation, useQuery } from "@tanstack/react-query"
import { Check, Circle, CircleAlert, CirclePause, Pencil, Plus, X } from "lucide-react"
import { type PointerEvent, type ReactNode, useCallback, useState } from "react"
import { Link, useLocation } from "react-router-dom"

import { projectRouteOf, taskPath, taskRouteOf } from "@openplan/task-ui"
import {
  Button,
  cn,
  Combobox,
  type ComboOption,
  EmptyState,
  fuzzyMatch,
  FuzzyText,
  Skeleton,
  Spinner,
} from "@openplan/ui"

import { useAgentSessions } from "../lib/agent-sessions"
import { type AgentSessionSummary, listTasks, markAgentSessionDone } from "../lib/api"
import { type Edges, useFloatingFrame } from "../lib/floating-frame"
import { errorText } from "../lib/format"
import { useAbbreviation, useProjects } from "../lib/projects"
import { promptBar, usePromptBarSelection } from "../lib/prompt-bar"
import { tasksKey } from "../lib/query-client"
import { runtime } from "../lib/runtime"
import { useAgentChat } from "../lib/use-agent-chat"
import { ChatLog, Composer, SessionState, StopButton, UsageLine } from "./agent-chat"

// The handles straddle the border, outside the rounded box: its clip would cut the corners away.
const RESIZE_HANDLES: ReadonlyArray<{ readonly edges: Edges; readonly className: string }> = [
  { edges: { top: true }, className: "inset-x-3 -top-1 h-2 cursor-ns-resize" },
  { edges: { bottom: true }, className: "inset-x-3 -bottom-1 h-2 cursor-ns-resize" },
  { edges: { left: true }, className: "inset-y-3 -left-1 w-2 cursor-ew-resize" },
  { edges: { right: true }, className: "inset-y-3 -right-1 w-2 cursor-ew-resize" },
  { edges: { top: true, left: true }, className: "-top-1.5 -left-1.5 size-4.5 cursor-nwse-resize" },
  { edges: { top: true, right: true }, className: "-top-1.5 -right-1.5 size-4.5 cursor-nesw-resize" },
  { edges: { bottom: true, left: true }, className: "-bottom-1.5 -left-1.5 size-4.5 cursor-nesw-resize" },
  { edges: { bottom: true, right: true }, className: "-right-1.5 -bottom-1.5 size-4.5 cursor-nwse-resize" },
]

// The agent over the foot of any page: a tab for each session the daemon holds, in every project,
// and the chat of the one picked. A session works on while the bar is hidden, so several can run at
// once, and the header control counts them.
export function PromptBar({ open, onClose }: { open: boolean; onClose: () => void }) {
  const sessions = useAgentSessions()
  const selected = usePromptBarSelection()
  const { ref, frame, resize, move, reset } = useFloatingFrame()
  if (!open) return null
  const sized = frame !== undefined
  return (
    <div className={cn("pointer-events-none fixed inset-0 z-40", !sized && "flex items-end justify-center px-4 pb-4")}>
      <section
        ref={ref}
        aria-label="Agent"
        style={sized ? { left: frame.x, top: frame.y, width: frame.width, height: frame.height } : undefined}
        // The keyboard layer leaves keys typed into an input alone, so the bar answers Escape from
        // its own prompt. A picker that took the Escape to close itself marks it handled.
        onKeyDown={(event) => {
          if (event.key === "Escape" && !event.defaultPrevented) onClose()
        }}
        className={cn("pointer-events-auto", sized ? "absolute" : "relative w-full max-w-3xl")}
      >
        {RESIZE_HANDLES.map(({ edges, className }) => (
          <div
            key={className}
            aria-hidden
            onPointerDown={resize(edges)}
            className={cn("absolute z-10", className)}
          />
        ))}
        <div
          className={cn(
            "bg-background flex flex-col overflow-hidden rounded-xl border shadow-xl",
            sized ? "h-full" : "max-h-[75vh]",
          )}
        >
          <SessionTabs
            sessions={sessions}
            selected={selected?.id}
            onClose={onClose}
            onMove={move}
            onReset={reset}
          />
          {selected === undefined ? (
            <NewPrompt sized={sized} />
          ) : (
            <ChatPane key={selected.id} project={selected.project} session={selected.id} sized={sized} />
          )}
        </div>
      </section>
    </div>
  )
}

// The strip is the handle the bar moves by, everywhere but on its buttons; a double click puts the
// bar back over the foot of the page.
function SessionTabs({
  sessions,
  selected,
  onClose,
  onMove,
  onReset,
}: {
  sessions: ReadonlyArray<AgentSessionSummary>
  selected: string | undefined
  onClose: () => void
  onMove: (event: PointerEvent) => void
  onReset: () => void
}) {
  const onStrip = (event: { target: EventTarget }) =>
    !(event.target instanceof Element && event.target.closest("button, a") !== null)
  return (
    <div
      onPointerDown={(event) => {
        if (onStrip(event)) onMove(event)
      }}
      onDoubleClick={(event) => {
        if (onStrip(event)) onReset()
      }}
      className="flex shrink-0 cursor-grab items-center gap-2 border-b px-2 py-1.5 active:cursor-grabbing"
    >
      <div className="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
        <button
          type="button"
          aria-pressed={selected === undefined}
          onClick={() => promptBar.select(undefined)}
          className={cn(
            "inline-flex shrink-0 cursor-pointer items-center gap-1 rounded-md px-2 py-1 text-xs",
            selected === undefined ? "bg-muted" : "text-muted-foreground hover:bg-muted/60",
          )}
        >
          <Plus className="size-3.5" aria-hidden />
          New
        </button>
        {sessions.map((session) => (
          <SessionTab key={session.id} session={session} active={session.id === selected} />
        ))}
      </div>
      <Button size="icon" aria-label="Hide the agent" title="Hide the agent (e)" onClick={onClose}>
        <X className="size-4" aria-hidden />
      </Button>
    </div>
  )
}

// A tab names the session by its first prompt and by the first task it is on; the pane below links
// every one of them.
function SessionTab({ session, active }: { session: AgentSessionSummary; active: boolean }) {
  const done = useMutation({
    mutationFn: () => runtime.runPromise(markAgentSessionDone(session.project, session.id)),
    onSuccess: () => {
      if (promptBar.getSnapshot()?.id === session.id) promptBar.select(undefined)
    },
  })
  const keys = related(session.context, session.tasks)
  return (
    <span
      className={cn("group inline-flex shrink-0 items-center rounded-md", active ? "bg-muted" : "hover:bg-muted/60")}
    >
      <button
        type="button"
        aria-pressed={active}
        title={session.title}
        onClick={() => promptBar.select({ project: session.project, id: session.id })}
        className="inline-flex max-w-64 cursor-pointer items-center gap-1.5 py-1 pr-1 pl-2 text-xs"
      >
        <SessionMark session={session} />
        <span className="truncate">{session.title}</span>
        {keys.length > 0 && (
          <span className="text-muted-foreground shrink-0 font-mono">
            {keys[0]}
            {keys.length > 1 && ` +${keys.length - 1}`}
          </span>
        )}
      </button>
      <button
        type="button"
        aria-label={`Mark the session "${session.title}" done`}
        title="Mark done"
        disabled={done.isPending}
        onClick={() => done.mutate()}
        className="text-muted-foreground hover:text-foreground mr-1 cursor-pointer rounded p-0.5 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
      >
        <Check className="size-3" aria-hidden />
      </button>
    </span>
  )
}

function SessionMark({ session }: { session: AgentSessionSummary }) {
  if (session.approvals > 0) {
    return <CircleAlert className="text-warning size-3.5 shrink-0" aria-label="Needs your approval" />
  }
  switch (session.status.kind) {
    case "starting":
    case "running":
      return <Spinner label="Working" className="size-3 shrink-0" />
    case "idle":
      return <Circle className="text-muted-foreground size-2 shrink-0 fill-current" aria-label="Waiting for you" />
    case "exited":
      return (
        <CirclePause
          className="text-muted-foreground/60 size-3.5 shrink-0"
          aria-label="Stopped; a new prompt resumes it"
        />
      )
  }
}

function related(context: string | undefined, tasks: ReadonlyArray<string>): ReadonlyArray<string> {
  return context === undefined ? tasks : [context, ...tasks.filter((key) => key !== context)]
}

// A new prompt goes to the project of the page under the bar, and on a task page it goes with that
// task, which the agent then takes as the one a prompt means when it names none.
function NewPrompt({ sized }: { sized: boolean }) {
  const pathname = useLocation().pathname
  const names = useProjects()?.map((known) => known.name)
  const [chosen, setChosen] = useState<string>()
  const [dropped, setDropped] = useState<string>()

  const project =
    [chosen, projectRouteOf(pathname)].find((name) => name !== undefined && names?.includes(name)) ?? names?.[0]
  const open = taskRouteOf(pathname)
  const context = open !== undefined && open.project === project && open.id !== dropped ? open.id : undefined

  if (names === undefined) return <Skeleton className="m-4 h-24" />
  if (project === undefined) {
    return (
      <div className="p-4">
        <EmptyState title="No projects yet" detail="Register a repository with `openplan project add`." />
      </div>
    )
  }
  return (
    <ChatPane
      key={project}
      project={project}
      session={undefined}
      context={context}
      sized={sized}
      header={
        <>
          <span className="text-muted-foreground">Project</span>
          <ProjectPicker projects={names} value={project} onChange={setChosen} />
          {context !== undefined && (
            <span className="bg-muted ml-auto inline-flex min-w-0 items-center gap-1 rounded-md py-0.5 pr-0.5 pl-2">
              <span className="text-muted-foreground">On</span>
              <TaskLink project={project} id={context} />
              <button
                type="button"
                aria-label="Ask without this task"
                title="Ask without this task"
                onClick={() => setDropped(context)}
                className="text-muted-foreground hover:text-foreground cursor-pointer rounded p-0.5"
              >
                <X className="size-3" aria-hidden />
              </button>
            </span>
          )}
        </>
      }
    />
  )
}

function ChatPane({
  project,
  session,
  context,
  sized,
  header,
}: {
  project: string
  session: string | undefined
  context?: string
  sized: boolean
  header?: ReactNode
}) {
  const chat = useAgentChat({
    project,
    session,
    context,
    onStarted: (id) => promptBar.select({ project, id }),
  })
  const abbreviation = useAbbreviation(project)
  const view = chat.view
  const status = view?.status

  return (
    <>
      <div className="flex min-h-9 shrink-0 items-center gap-2 border-b px-4 py-1.5 text-xs">
        {header ??
          (view !== undefined && (
            <RelatedTasks project={project} context={view.context ?? undefined} tasks={view.tasks} />
          ))}
      </div>
      {view !== undefined && view.transcript.entries.length > 0 ? (
        <ChatLog
          project={project}
          abbreviation={abbreviation}
          transcript={chat.transcript}
          sends={chat.sends}
          onDecide={chat.decide}
          deciding={chat.deciding}
          className={cn("border-b", !sized && "max-h-[50vh]")}
        />
      ) : (
        sized && <div className="min-h-0 flex-1" />
      )}
      <div className="flex shrink-0 flex-col gap-2 px-4 py-3">
        <Composer disabled={chat.closed} sending={chat.sending} live={chat.running} onSend={chat.send} />
        <div className="flex min-h-5 items-center gap-3 text-xs">
          <div className="min-w-0 flex-1">
            {status?.kind === "exited" ? (
              <span className="text-muted-foreground">
                The agent stopped{status.code !== null && ` with code ${status.code}`}. A new prompt resumes it.
              </span>
            ) : chat.error !== undefined ? (
              <span className="text-warning">{errorText(chat.error)}</span>
            ) : (
              <SessionState live={chat.live} />
            )}
          </div>
          {view !== undefined && !chat.running && <UsageLine usage={chat.transcript.usage} />}
          {chat.running && <StopButton stopping={chat.stopping} onStop={chat.stop} />}
        </div>
      </div>
    </>
  )
}

function RelatedTasks({
  project,
  context,
  tasks,
}: {
  project: string
  context: string | undefined
  tasks: ReadonlyArray<string>
}) {
  return (
    <span className="flex min-w-0 flex-wrap items-center gap-x-4 gap-y-1">
      <span className="text-muted-foreground">{project}</span>
      {context !== undefined && (
        <span className="inline-flex min-w-0 items-center gap-1.5">
          <span className="text-muted-foreground">On</span>
          <TaskLink project={project} id={context} />
        </span>
      )}
      {tasks.length > 0 && (
        <span className="inline-flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1">
          <span className="text-muted-foreground">Wrote</span>
          {tasks.map((id) => (
            <TaskLink key={id} project={project} id={id} />
          ))}
        </span>
      )}
    </span>
  )
}

function TaskLink({ project, id }: { project: string; id: string }) {
  const tasks = useQuery({
    queryKey: tasksKey(project),
    queryFn: () => runtime.runPromise(listTasks(project)),
  })
  const title = tasks.data?.find((task) => task.id === id)?.title
  return (
    <Link
      to={taskPath(project, id)}
      title={title}
      className="hover:text-foreground inline-flex max-w-64 min-w-0 items-center gap-1.5 hover:underline"
    >
      <span className="shrink-0 font-mono">{id}</span>
      {title !== undefined && <span className="text-muted-foreground truncate">{title}</span>}
    </Link>
  )
}

// The project a new prompt goes to, shown as a name and changed through the same search box the
// parent picker uses.
function ProjectPicker({
  projects,
  value,
  onChange,
}: {
  projects: ReadonlyArray<string>
  value: string
  onChange: (next: string) => void
}) {
  const [editing, setEditing] = useState(false)
  const buildOptions = useCallback(
    (query: string): ReadonlyArray<ComboOption> =>
      projects
        .flatMap((name) => {
          const match = fuzzyMatch(query, name)
          return match === null ? [] : [{ name, match }]
        })
        .sort((a, b) => a.match.score - b.match.score)
        .map(({ name, match }) => ({
          key: name,
          content: <FuzzyText text={name} indices={match.indices} />,
          onSelect: () => onChange(name),
        })),
    [projects, onChange],
  )
  if (editing) {
    return (
      <Combobox
        placeholder="Project…"
        buildOptions={buildOptions}
        onClose={() => setEditing(false)}
        emptyLabel="No matching project"
        className="w-56"
      />
    )
  }
  return (
    <span className="flex min-w-0 items-center gap-1">
      <span className="truncate">{value}</span>
      {projects.length > 1 && (
        <Button onClick={() => setEditing(true)} aria-label="Change project" className="px-1.5">
          <Pencil className="size-3.5" />
        </Button>
      )}
    </span>
  )
}

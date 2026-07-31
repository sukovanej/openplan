import { CircleDashed, Square, SquareCheckBig } from "lucide-react"
import { createContext, useContext, useMemo } from "react"
import Markdown, { type Components } from "react-markdown"
import { Link } from "react-router-dom"
import remarkGfm from "remark-gfm"

import type { TaskRef as TaskRefData } from "@open-planner/api-client"
import { cn, Prose } from "@open-planner/ui"

import { useAbbreviation } from "../lib/abbreviation"
import { StatusChip } from "./status-badge"
import { taskLinkPlugins } from "./task-links"

const RefsContext = createContext<ReadonlyMap<string, TaskRefData>>(new Map())

const linkClass =
  "font-medium text-foreground underline decoration-1 decoration-muted-foreground/50 underline-offset-2 transition-colors hover:decoration-foreground"

// `align-middle` centres the chip on the surrounding font's x-height, which leaves it sitting ~1.5px
// below the optical middle of the line; the nudge takes that back without disturbing the line box.
const chipClass =
  "not-prose relative -top-px mx-0.5 inline-flex max-w-full items-center gap-1 rounded-md border px-1.5 py-0.5 align-middle text-sm font-medium leading-none no-underline transition-colors"

const TASK_ROUTE = "/task/"

function TaskRef({ href }: { href: string }) {
  const refs = useContext(RefsContext)
  const id = href.slice(TASK_ROUTE.length).split("#")[0]
  const task = refs.get(id)
  return (
    <Link
      to={href}
      className={cn(
        chipClass,
        task === undefined
          ? "border-dashed border-border text-muted-foreground hover:bg-muted/40"
          : "border-border bg-muted/40 text-foreground hover:bg-muted",
      )}
    >
      {task === undefined ? (
        <CircleDashed className="size-4 shrink-0 text-muted-foreground/60" aria-hidden />
      ) : (
        <StatusChip status={task.status} className="size-4 shrink-0" />
      )}
      <span className="text-muted-foreground shrink-0 tabular-nums">{id}</span>
      {/* A reference the store cannot resolve has no title to show; its key is all there is to name it. */}
      {task !== undefined && <span className="min-w-0 truncate">{task.title}</span>}
    </Link>
  )
}

const components: Components = {
  a({ href, children }) {
    if (href !== undefined && href.startsWith(TASK_ROUTE)) {
      return <TaskRef href={href} />
    }
    if (href !== undefined && href.startsWith("/")) {
      return (
        <Link to={href} className={linkClass}>
          {children}
        </Link>
      )
    }
    return (
      <a href={href} target="_blank" rel="noreferrer">
        {children}
      </a>
    )
  },
  table({ children }) {
    return (
      <div className="my-3 overflow-hidden rounded-lg border border-border">
        <table className="my-0">{children}</table>
      </div>
    )
  },
  input({ type, checked }) {
    if (type !== "checkbox") return null
    const Icon = checked ? SquareCheckBig : Square
    return (
      <Icon
        role="checkbox"
        aria-checked={checked}
        className={cn(
          "mr-1.5 inline-block size-[1.05em] shrink-0 align-[-0.2em]",
          checked ? "text-emerald-600 dark:text-emerald-500" : "text-muted-foreground/60",
        )}
      />
    )
  },
}

export function TaskBody({ markdown, refs }: { markdown: string; refs?: ReadonlyArray<TaskRefData> }) {
  const refMap = useMemo(() => new Map((refs ?? []).map((ref) => [ref.id, ref])), [refs])
  const abbreviation = useAbbreviation()
  const plugins = useMemo(() => [remarkGfm, taskLinkPlugins(abbreviation)], [abbreviation])
  return (
    <RefsContext.Provider value={refMap}>
      <Prose data-keys-ignore>
        <Markdown remarkPlugins={plugins} components={components}>
          {markdown}
        </Markdown>
      </Prose>
    </RefsContext.Provider>
  )
}

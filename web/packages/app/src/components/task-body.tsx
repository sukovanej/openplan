import { CircleDashed, Square, SquareCheckBig } from "lucide-react"
import { createContext, type ReactNode, useContext, useMemo } from "react"
import Markdown, { type Components } from "react-markdown"
import { Link } from "react-router-dom"
import remarkGfm from "remark-gfm"

import type { TaskRef as TaskRefData } from "@open-planner/api-client"

import { useAbbreviation } from "../lib/abbreviation"
import { cn } from "../lib/utils"
import { StatusChip } from "./status-badge"
import { taskLinkPlugins } from "./task-links"

const RefsContext = createContext<ReadonlyMap<string, TaskRefData>>(new Map())

const proseColors =
  "[--tw-prose-body:var(--color-neutral-700)] [--tw-prose-bold:var(--color-neutral-700)] [--tw-prose-headings:var(--color-neutral-900)] [--tw-prose-code:var(--color-neutral-900)] dark:[--tw-prose-invert-body:var(--color-neutral-300)] dark:[--tw-prose-invert-bold:var(--color-neutral-300)] dark:[--tw-prose-invert-headings:var(--color-neutral-200)] dark:[--tw-prose-invert-code:var(--color-neutral-200)]"

const proseSpacing =
  "prose-headings:mt-7 prose-headings:mb-3 prose-h2:text-lg prose-h3:text-base prose-h4:text-sm prose-p:my-2 prose-ul:my-2 prose-ol:my-2 prose-li:my-0.5 prose-pre:my-3 prose-pre:bg-muted prose-pre:text-foreground"

// Inline code is a muted chip with the Typography backtick pseudo-elements removed.
// The [&_pre_code] reset keeps these chip styles from leaking into fenced blocks,
// which already carry their own background and padding via prose-pre.
const proseCode =
  "prose-code:font-normal prose-code:text-[0.875em] prose-code:bg-muted prose-code:rounded prose-code:px-1 prose-code:py-0.5 prose-code:before:content-none prose-code:after:content-none [&_pre_code]:bg-transparent [&_pre_code]:p-0 [&_pre_code]:text-[1em]"

// GFM task-list items carry the `task-list-item` class; drop their bullet and keep the checkbox
// inline so code chips and text stay in normal inline flow instead of becoming flex items.
const proseTaskList = "[&_li.task-list-item]:list-none [&_ul:has(li.task-list-item)]:pl-1"

const proseTable =
  "prose-table:my-0 prose-thead:bg-muted/40 prose-th:border-b prose-th:border-r prose-th:border-border prose-th:px-3 prose-th:py-1.5 prose-td:border-b prose-td:border-r prose-td:border-border prose-td:px-3 prose-td:py-1.5 [&_th:last-child]:border-r-0 [&_td:last-child]:border-r-0 [&_tbody_tr:last-child_td]:border-b-0"

const proseClass = `prose prose-base prose-neutral dark:prose-invert max-w-none ${proseColors} ${proseSpacing} ${proseCode} ${proseTaskList} ${proseTable}`

const linkClass =
  "font-medium text-foreground underline decoration-1 decoration-muted-foreground/50 underline-offset-2 transition-colors hover:decoration-foreground"

const chipClass =
  "not-prose mx-0.5 inline-flex max-w-full items-center gap-1 rounded-md border px-1.5 py-0.5 align-middle text-sm font-medium leading-none no-underline transition-colors"

const TASK_ROUTE = "/task/"

function TaskRef({ href, fallback }: { href: string; fallback: ReactNode }) {
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
      <span className="min-w-0 truncate">{task?.title ?? fallback}</span>
    </Link>
  )
}

const components: Components = {
  a({ href, children }) {
    if (href !== undefined && href.startsWith(TASK_ROUTE)) {
      return <TaskRef href={href} fallback={children} />
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
      <article data-keys-ignore className={proseClass}>
        <Markdown remarkPlugins={plugins} components={components}>
          {markdown}
        </Markdown>
      </article>
    </RefsContext.Provider>
  )
}

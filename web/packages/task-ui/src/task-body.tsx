import { Square, SquareCheckBig } from "lucide-react"
import { type ComponentProps, createContext, useContext, useMemo } from "react"
import Markdown, { type Components } from "react-markdown"
import { Link } from "react-router-dom"
import remarkGfm from "remark-gfm"

import type { TaskRef } from "@open-planner/api-client"
import { cn, Prose } from "@open-planner/ui"

import { taskLinkPlugins } from "./task-links"
import { TASK_ROUTE } from "./task-path"
import { TaskRefChip } from "./task-ref-chip"

const RefsContext = createContext<ReadonlyMap<string, TaskRef>>(new Map())

const linkClass =
  "font-medium text-foreground underline decoration-1 decoration-muted-foreground/50 underline-offset-2 transition-colors hover:decoration-foreground"

function BodyTaskRef({ href }: { href: string }) {
  const refs = useContext(RefsContext)
  const id = href.slice(TASK_ROUTE.length).split("#")[0]
  return <TaskRefChip to={href} id={id} task={refs.get(id)} />
}

const components: Components = {
  a({ href, children }) {
    if (href !== undefined && href.startsWith(TASK_ROUTE)) {
      return <BodyTaskRef href={href} />
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
      <div className="border-border my-3 overflow-hidden rounded-lg border">
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
          checked ? "text-success" : "text-muted-foreground/60",
        )}
      />
    )
  },
}

export function TaskBody({
  markdown,
  refs,
  abbreviation,
  ...props
}: ComponentProps<typeof Prose> & {
  markdown: string
  refs?: ReadonlyArray<TaskRef>
  abbreviation: string | undefined
}) {
  const refMap = useMemo(() => new Map((refs ?? []).map((ref) => [ref.id, ref])), [refs])
  const plugins = useMemo(() => [remarkGfm, taskLinkPlugins(abbreviation)], [abbreviation])
  return (
    <RefsContext.Provider value={refMap}>
      <Prose {...props}>
        <Markdown remarkPlugins={plugins} components={components}>
          {markdown}
        </Markdown>
      </Prose>
    </RefsContext.Provider>
  )
}

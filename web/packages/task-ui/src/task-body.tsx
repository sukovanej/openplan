import { Square, SquareCheckBig } from "lucide-react"
import { type ComponentProps, createContext, useContext, useMemo } from "react"
import Markdown, { type Components } from "react-markdown"
import { Link } from "react-router-dom"
import remarkGfm from "remark-gfm"

import type { TaskRef } from "@openplan/api-client"
import { cn, Prose } from "@openplan/ui"

import { CodeBlock } from "./code-block"
import { taskLinkPlugins } from "./task-links"
import { taskRouteOf } from "./task-path"
import { TaskRefChip } from "./task-ref-chip"

const RefsContext = createContext<ReadonlyMap<string, TaskRef>>(new Map())

const linkClass =
  "font-medium text-foreground underline decoration-1 decoration-muted-foreground/50 underline-offset-2 transition-colors hover:decoration-foreground"

function BodyTaskRef({ href, id }: { href: string; id: string }) {
  const refs = useContext(RefsContext)
  return <TaskRefChip to={href} id={id} task={refs.get(id)} />
}

const components: Components = {
  pre: CodeBlock,
  a({ href, children }) {
    const task = href === undefined ? undefined : taskRouteOf(href)
    if (href !== undefined && task !== undefined) {
      return <BodyTaskRef href={href} id={task.id} />
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
  project,
  markdown,
  refs,
  abbreviation,
  ...props
}: ComponentProps<typeof Prose> & {
  project: string
  markdown: string
  refs?: ReadonlyArray<TaskRef>
  abbreviation: string | undefined
}) {
  const refMap = useMemo(() => new Map((refs ?? []).map((ref) => [ref.id, ref])), [refs])
  const plugins = useMemo(() => [remarkGfm, taskLinkPlugins({ project, abbreviation })], [project, abbreviation])
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

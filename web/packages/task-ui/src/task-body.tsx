import { Square, SquareCheckBig } from "lucide-react"
import { type ComponentProps, createContext, memo, useContext, useMemo } from "react"
import Markdown, { type Components } from "react-markdown"
import { Link } from "react-router-dom"
import remarkGfm from "remark-gfm"

import type { DocRef, TaskRef } from "@openplan/api-client"
import { cn, Prose } from "@openplan/ui"

import { CodeBlock } from "./code-block"
import { DocRefChip } from "./doc-ref-chip"
import { taskLinkPlugins } from "./task-links"
import { docRouteOf, taskRouteOf } from "./task-path"
import { TaskRefChip } from "./task-ref-chip"

const RefsContext = createContext<ReadonlyMap<string, TaskRef>>(new Map())
const DocRefsContext = createContext<ReadonlyMap<string, DocRef>>(new Map())

const linkClass =
  "font-medium text-foreground underline decoration-1 decoration-muted-foreground/50 underline-offset-2 transition-colors hover:decoration-foreground"

function BodyTaskRef({ href, id }: { href: string; id: string }) {
  const refs = useContext(RefsContext)
  return <TaskRefChip to={href} id={id} task={refs.get(id)} />
}

function BodyDocRef({ href, name }: { href: string; name: string }) {
  const refs = useContext(DocRefsContext)
  return <DocRefChip to={href} name={name} doc={refs.get(name)} />
}

const components: Components = {
  pre: CodeBlock,
  a({ href, children }) {
    const task = href === undefined ? undefined : taskRouteOf(href)
    if (href !== undefined && task !== undefined) {
      return <BodyTaskRef href={href} id={task.id} />
    }
    const doc = href === undefined ? undefined : docRouteOf(href)
    if (href !== undefined && doc !== undefined) {
      return <BodyDocRef href={href} name={doc.name} />
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

// react-markdown parses the whole text on each render, and the page around a body renders on each
// move of its cursor.
export const TaskBody = memo(function TaskBody({
  project,
  markdown,
  refs,
  docRefs,
  abbreviation,
  ...props
}: ComponentProps<typeof Prose> & {
  project: string
  markdown: string
  refs?: ReadonlyArray<TaskRef>
  docRefs?: ReadonlyArray<DocRef>
  abbreviation: string | undefined
}) {
  const refMap = useMemo(() => new Map((refs ?? []).map((ref) => [ref.id, ref])), [refs])
  const docRefMap = useMemo(() => new Map((docRefs ?? []).map((ref) => [ref.name, ref])), [docRefs])
  const plugins = useMemo(() => [remarkGfm, taskLinkPlugins({ project, abbreviation })], [project, abbreviation])
  return (
    <RefsContext.Provider value={refMap}>
      <DocRefsContext.Provider value={docRefMap}>
        <Prose {...props}>
          <Markdown remarkPlugins={plugins} components={components}>
            {markdown}
          </Markdown>
        </Prose>
      </DocRefsContext.Provider>
    </RefsContext.Provider>
  )
})

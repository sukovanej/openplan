import { Square, SquareCheckBig } from "lucide-react"
import { type ComponentProps, memo, useMemo } from "react"
import Markdown, { type Components } from "react-markdown"
import { Link } from "react-router-dom"
import remarkGfm from "remark-gfm"

import { cn, Prose } from "@openplan/ui"

import { CodeBlock } from "./code-block"
import { DocRefChip } from "./doc-ref-chip"
import { taskLinkPlugins } from "./task-links"
import { docRouteOf, taskRouteOf } from "./task-path"
import { TaskRefChip } from "./task-ref-chip"

const linkClass =
  "font-medium text-foreground underline decoration-1 decoration-muted-foreground/50 underline-offset-2 transition-colors hover:decoration-foreground"

const components: Components = {
  pre: CodeBlock,
  a({ href, children }) {
    const task = href === undefined ? undefined : taskRouteOf(href)
    if (href !== undefined && task !== undefined) {
      return <TaskRefChip to={href} project={task.project} id={task.id} />
    }
    const doc = href === undefined ? undefined : docRouteOf(href)
    if (href !== undefined && doc !== undefined) {
      return <DocRefChip to={href} project={doc.project} name={doc.name} />
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

const inlineComponents: Components = { ...components, p: ({ children }) => <>{children}</> }

interface BodyScope {
  project: string
  markdown: string
  abbreviation: string | undefined
}

function Rendered({ project, markdown, abbreviation, parts }: BodyScope & { parts: Components }) {
  const plugins = useMemo(() => [remarkGfm, taskLinkPlugins({ project, abbreviation })], [project, abbreviation])
  return (
    <Markdown remarkPlugins={plugins} components={parts}>
      {markdown}
    </Markdown>
  )
}

// react-markdown parses the whole text on each render, and the page around a body renders on each
// move of its cursor.
export const TaskBody = memo(function TaskBody({
  project,
  markdown,
  abbreviation,
  ...props
}: ComponentProps<typeof Prose> & BodyScope) {
  return (
    <Prose {...props}>
      <Rendered project={project} markdown={markdown} abbreviation={abbreviation} parts={components} />
    </Prose>
  )
})

export const TaskInline = memo(function TaskInline(scope: BodyScope) {
  return <Rendered {...scope} parts={inlineComponents} />
})

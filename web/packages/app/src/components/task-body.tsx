import Markdown from "react-markdown"
import remarkGfm from "remark-gfm"

export function TaskBody({ markdown }: { markdown: string }) {
  return (
    <article className="prose prose-neutral dark:prose-invert max-w-none prose-pre:bg-muted prose-pre:text-foreground">
      <Markdown remarkPlugins={[remarkGfm]}>{markdown}</Markdown>
    </article>
  )
}

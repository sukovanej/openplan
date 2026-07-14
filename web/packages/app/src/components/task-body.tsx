import Markdown from "react-markdown"
import remarkGfm from "remark-gfm"

export function TaskBody({ markdown }: { markdown: string }) {
  return (
    <article className="prose prose-base prose-neutral dark:prose-invert max-w-none [--tw-prose-body:var(--color-neutral-800)] [--tw-prose-bold:var(--color-neutral-900)] [--tw-prose-headings:var(--color-neutral-900)] [--tw-prose-code:var(--color-neutral-900)] dark:[--tw-prose-invert-body:var(--color-neutral-400)] dark:[--tw-prose-invert-bold:var(--color-neutral-200)] dark:[--tw-prose-invert-headings:var(--color-neutral-200)] dark:[--tw-prose-invert-code:var(--color-neutral-200)] prose-headings:mt-7 prose-headings:mb-3 prose-h2:text-lg prose-h3:text-base prose-h4:text-sm prose-p:my-2 prose-ul:my-2 prose-ol:my-2 prose-li:my-0.5 prose-pre:my-3 prose-pre:bg-muted prose-pre:text-foreground">
      <Markdown remarkPlugins={[remarkGfm]}>{markdown}</Markdown>
    </article>
  )
}

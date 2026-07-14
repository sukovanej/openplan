import Markdown from "react-markdown"
import remarkGfm from "remark-gfm"

const proseColors =
  "[--tw-prose-body:var(--color-neutral-800)] [--tw-prose-bold:var(--color-neutral-800)] [--tw-prose-headings:var(--color-neutral-900)] [--tw-prose-code:var(--color-neutral-800)] dark:[--tw-prose-invert-body:var(--color-neutral-400)] dark:[--tw-prose-invert-bold:var(--color-neutral-400)] dark:[--tw-prose-invert-headings:var(--color-neutral-200)] dark:[--tw-prose-invert-code:var(--color-neutral-400)]"

const proseSpacing =
  "prose-headings:mt-7 prose-headings:mb-3 prose-h2:text-lg prose-h3:text-base prose-h4:text-sm prose-p:my-2 prose-ul:my-2 prose-ol:my-2 prose-li:my-0.5 prose-pre:my-3 prose-pre:bg-muted prose-pre:text-foreground"

// Inline code is a muted chip with the Typography backtick pseudo-elements removed.
// The [&_pre_code] reset keeps these chip styles from leaking into fenced blocks,
// which already carry their own background and padding via prose-pre.
const proseCode =
  "prose-code:font-normal prose-code:bg-muted prose-code:rounded prose-code:px-1.5 prose-code:before:content-none prose-code:after:content-none [&_pre_code]:bg-transparent [&_pre_code]:p-0"

const proseClass =
  `prose prose-base prose-neutral dark:prose-invert max-w-none ${proseColors} ${proseSpacing} ${proseCode}`

export function TaskBody({ markdown }: { markdown: string }) {
  return (
    <article className={proseClass}>
      <Markdown remarkPlugins={[remarkGfm]}>{markdown}</Markdown>
    </article>
  )
}

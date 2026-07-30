import type * as React from "react"

import { cn } from "./cn"

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

export function Prose({ className, ...props }: React.ComponentProps<"article">) {
  return <article className={cn(proseClass, className)} {...props} />
}

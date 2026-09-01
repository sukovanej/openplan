import type * as React from "react"

import { cn } from "./cn"

const proseColors =
  "[--tw-prose-body:var(--prose-body)] [--tw-prose-bold:var(--prose-body)] [--tw-prose-headings:var(--prose-heading)] [--tw-prose-code:var(--prose-code)] [--tw-prose-bullets:var(--prose-marker)] [--tw-prose-counters:var(--prose-marker)] dark:[--tw-prose-invert-body:var(--prose-body)] dark:[--tw-prose-invert-bold:var(--prose-body)] dark:[--tw-prose-invert-headings:var(--prose-heading)] dark:[--tw-prose-invert-code:var(--prose-code)] dark:[--tw-prose-invert-bullets:var(--prose-marker)] dark:[--tw-prose-invert-counters:var(--prose-marker)]"

// The serif reads thin against the dark ground at 400, so the body spends one step of the variable
// face there.
const proseBody = "font-serif text-[17px] leading-7 max-w-none dark:[font-weight:450]"

// Headings stay on the UI face. The rule above an h2 is what gives a task with a dozen `##` sections
// visible breaks, which margin alone did not.
const proseHeadings =
  "prose-headings:font-sans prose-headings:mt-7 prose-headings:mb-3 prose-h2:mt-[34px] prose-h2:border-t prose-h2:border-border prose-h2:pt-[18px] prose-h2:text-[19px] prose-h2:font-semibold prose-h3:mt-[26px] prose-h3:mb-1.5 prose-h3:text-[15px] prose-h4:text-sm"

const proseSpacing =
  "prose-p:my-[14px] prose-ul:my-[14px] prose-ol:my-[14px] prose-li:my-1 prose-pre:my-3 prose-pre:bg-muted prose-pre:font-mono prose-pre:text-foreground"

// Inline code is a muted chip with the Typography backtick pseudo-elements removed.
// The [&_pre_code] reset keeps these chip styles from leaking into fenced blocks,
// which already carry their own background and padding via prose-pre.
const proseCode =
  "prose-code:font-mono prose-code:font-normal prose-code:text-[0.82em] prose-code:bg-prose-code-surface prose-code:rounded prose-code:px-1 prose-code:py-0.5 prose-code:before:content-none prose-code:after:content-none [&_pre_code]:bg-transparent [&_pre_code]:p-0 [&_pre_code]:text-[1em]"

// GFM task-list items carry the `task-list-item` class; drop their bullet and keep the checkbox
// inline so code chips and text stay in normal inline flow instead of becoming flex items.
const proseTaskList = "[&_li.task-list-item]:list-none [&_ul:has(li.task-list-item)]:pl-1"

const proseTable =
  "prose-table:my-0 prose-thead:bg-muted/40 prose-th:border-b prose-th:border-r prose-th:border-border prose-th:px-3 prose-th:py-1.5 prose-td:border-b prose-td:border-r prose-td:border-border prose-td:px-3 prose-td:py-1.5 [&_th:last-child]:border-r-0 [&_td:last-child]:border-r-0 [&_tbody_tr:last-child_td]:border-b-0"

const proseClass = `prose prose-neutral dark:prose-invert ${proseBody} ${proseColors} ${proseHeadings} ${proseSpacing} ${proseCode} ${proseTaskList} ${proseTable}`

export function Prose({ className, ...props }: React.ComponentProps<"article">) {
  return <article className={cn(proseClass, className)} {...props} />
}

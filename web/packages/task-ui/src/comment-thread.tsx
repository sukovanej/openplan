import { Bot, CircleAlert } from "lucide-react"

import type { Comment, FieldError, TaskRef } from "@openplan/api-client"
import { absoluteTime, MetaLine, Section, Tag, Tooltip } from "@openplan/ui"

import { fieldFailure, fieldMessage, fieldValue } from "./metadata"
import { TaskBody } from "./task-body"

// A comment log is append-only, and the web reads it. There is no input box and no edit control,
// because the CLI is what writes an entry and the daemon is the single writer behind it.
export function CommentThread({
  project,
  comments,
  refs,
  abbreviation,
}: {
  project: string
  comments: ReadonlyArray<Comment>
  refs?: ReadonlyArray<TaskRef>
  abbreviation: string | undefined
}) {
  return (
    <Section title="Comments" count={comments.length}>
      {comments.length === 0 ? (
        <p className="text-muted-foreground text-sm">No comments yet.</p>
      ) : (
        <ol className="space-y-3">
          {comments.map((comment, index) => (
            <li key={index} className="border-border bg-muted/20 rounded-lg border p-4">
              <MetaLine className="mb-1">
                <span className="text-foreground text-sm font-medium">
                  <Damaged field={comment.author}>{(author) => author}</Damaged>
                </span>
                <Damaged field={comment.at}>{(at) => <time dateTime={at}>{absoluteTime(at)}</time>}</Damaged>
                {comment.agent !== undefined && comment.agent !== null && (
                  <Tag className="border-border text-muted-foreground">
                    <Bot aria-hidden className="size-3" />
                    <span>{comment.agent}</span>
                  </Tag>
                )}
              </MetaLine>
              <TaskBody
                project={project}
                markdown={comment.text}
                refs={refs}
                abbreviation={abbreviation}
                className="text-[15px] leading-6"
                data-keys-ignore
              />
            </li>
          ))}
        </ol>
      )}
    </Section>
  )
}

// A hand-damaged heading still delivers the text it introduces, so the field that failed reads as
// the reason it failed and the entry keeps its place in the thread.
function Damaged({ field, children }: { field: string | FieldError; children: (value: string) => React.ReactNode }) {
  const value = fieldValue(field)
  if (value !== undefined) return <>{children(value)}</>
  const failure = fieldFailure(field)
  const message = failure === undefined ? "unreadable" : fieldMessage(failure)
  return (
    <Tooltip content={message}>
      {/* Focusable, so the keyboard reaches the reason: it stands where a value would have. */}
      <span tabIndex={0} className="text-danger/90 inline-flex min-w-0 items-center gap-1">
        <CircleAlert aria-hidden className="size-3.5 shrink-0" />
        <span className="truncate">{message}</span>
      </span>
    </Tooltip>
  )
}

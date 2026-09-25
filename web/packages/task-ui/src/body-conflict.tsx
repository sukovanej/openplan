import { Pencil, TriangleAlert } from "lucide-react"
import { type ComponentProps, useState } from "react"

import type { TaskRef } from "@openplan/api-client"
import { Button, cn } from "@openplan/ui"

import type { BodySegment, ConflictBlock, ConflictVersion } from "./body-segments"
import { CONFLICT_TINT, InForceMark } from "./conflict-mark"
import { TaskBody } from "./task-body"

interface Markdown {
  readonly project: string
  readonly refs?: ReadonlyArray<TaskRef>
  readonly abbreviation: string | undefined
}

function Version({ version, inForce, markdown }: { version: ConflictVersion; inForce: boolean; markdown: Markdown }) {
  return (
    <div role="group" aria-label={version.label} className="px-5 py-4">
      <div className="mb-2 flex min-w-0 items-center gap-2">
        <span className="text-muted-foreground min-w-0 truncate font-mono text-xs">{version.label}</span>
        {inForce && <InForceMark />}
      </div>
      {version.text.trim() === "" ? (
        <p className="text-muted-foreground text-sm italic">No text. This version removes the passage.</p>
      ) : (
        <TaskBody {...markdown} markdown={version.text} />
      )}
    </div>
  )
}

// Without `onResolve` the block only shows its two versions, as a revision of the past does.

// A blank line keeps two passages as two paragraphs in markdown.
const joined = (first: string, second: string) =>
  first.trim() === "" ? second : second.trim() === "" ? first : `${first.trimEnd()}\n\n${second}`

export function BodyConflict({
  conflict,
  onResolve,
  pending = false,
  ...markdown
}: Markdown & {
  conflict: ConflictBlock
  onResolve?: (text: string) => void
  pending?: boolean
}) {
  const { other, published } = conflict
  const both = joined(other.text, published.text)
  const [draft, setDraft] = useState<string>()
  return (
    <section aria-label="Conflict from a sync" className="border-warning/40 my-6 overflow-hidden rounded-lg border">
      <header className={cn("flex items-center gap-1.5 border-b px-3 py-2 text-xs font-medium", CONFLICT_TINT)}>
        <TriangleAlert aria-hidden className="size-3.5 shrink-0" />A sync kept two versions of this passage.
      </header>
      {draft === undefined ? (
        <div className="divide-y">
          <Version version={other} inForce={false} markdown={markdown} />
          <Version version={published} inForce markdown={markdown} />
        </div>
      ) : (
        <div className="p-3">
          <textarea
            aria-label="Merged text"
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            rows={Math.max(4, draft.split("\n").length + 1)}
            spellCheck={false}
            className="border-input focus:border-foreground/20 bg-background block w-full resize-y rounded-md border p-2.5 font-mono text-sm outline-none"
          />
        </div>
      )}
      {onResolve !== undefined && (
        <footer className="flex flex-wrap items-center gap-1 border-t px-2 py-1.5">
          {draft === undefined ? (
            <>
              <Button variant="accent" disabled={pending} onClick={() => onResolve(other.text)}>
                Keep {other.label}
              </Button>
              <Button variant="accent" disabled={pending} onClick={() => onResolve(published.text)}>
                Keep {published.label}
              </Button>
              <Button variant="accent" disabled={pending} onClick={() => onResolve(both)}>
                Keep both
              </Button>
              <Button disabled={pending} onClick={() => setDraft(both)} className="ml-auto">
                <Pencil aria-hidden className="size-3.5" />
                Edit
              </Button>
            </>
          ) : (
            <>
              <Button variant="accent" disabled={pending} onClick={() => onResolve(draft)}>
                Save
              </Button>
              <Button onClick={() => setDraft(undefined)}>Cancel</Button>
            </>
          )}
        </footer>
      )}
    </section>
  )
}

// A body with no block renders as one `TaskBody`, as it always did. `block` is what a resolve names
// the block by, so the segments must come from the body exactly as the daemon sent it.
export function TaskBodyWithConflicts({
  segments,
  project,
  refs,
  abbreviation,
  onResolve,
  pending,
  proseClassName,
  ...attributes
}: Markdown &
  ComponentProps<"div"> & {
    segments: ReadonlyArray<BodySegment>
    onResolve?: (block: string, text: string) => void
    pending?: boolean
    proseClassName?: string
  }) {
  const markdown = { project, refs, abbreviation }
  return (
    <div {...attributes}>
      {segments.map((segment, at) =>
        segment.kind === "text" ? (
          segment.text.trim() === "" ? null : (
            <TaskBody key={at} {...markdown} markdown={segment.text} className={proseClassName} />
          )
        ) : (
          <BodyConflict
            // The block's text is in the key, so a block that changed under an open edit drops the draft.
            key={`${at}:${segment.block}`}
            {...markdown}
            conflict={segment}
            onResolve={onResolve === undefined ? undefined : (text) => onResolve(segment.block, text)}
            pending={pending}
          />
        ),
      )}
    </div>
  )
}

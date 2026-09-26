import { cn } from "./cn"
import { type DiffHunk, type DiffLine, type DiffLineKind, parseDiff } from "./diff-parse"

const sign: Record<DiffLineKind, string> = { context: " ", added: "+", deleted: "-" }

const tint: Record<DiffLineKind, string> = {
  context: "",
  added: "text-change-added bg-change-added/8",
  deleted: "text-change-deleted bg-change-deleted/8",
}

const emphasis: Record<DiffLineKind, string> = {
  context: "",
  added: "text-change-added bg-change-added/25",
  deleted: "text-change-deleted bg-change-deleted/25",
}

// A unified diff and nothing else: the file it belongs to, and what kind of change it is, are the
// caller's to render. The `---` and `+++` lines are dropped for the same reason.
export function DiffView({ diff, className }: { diff: string; className?: string }) {
  const hunks = parseDiff(diff)
  if (hunks.length === 0) {
    return <p className={cn("text-muted-foreground px-2 py-1.5 text-xs", className)}>No differences</p>
  }
  return (
    <div className={cn("grid grid-cols-[auto_auto_1fr] font-mono text-[11px] leading-5", className)}>
      {hunks.map((hunk) => (
        <Hunk key={hunk.ranges} hunk={hunk} />
      ))}
    </div>
  )
}

function Hunk({ hunk }: { hunk: DiffHunk }) {
  return (
    <>
      <div className="text-muted-foreground/70 col-span-3 border-t px-2 py-1 first:border-t-0">@@ {hunk.ranges} @@</div>
      {hunk.lines.map((line, at) => (
        <Line key={at} line={line} />
      ))}
    </>
  )
}

function Line({ line }: { line: DiffLine }) {
  const gutter = cn("text-muted-foreground/50 px-1.5 text-right tabular-nums select-none", tint[line.kind])
  return (
    <>
      <span className={gutter}>{line.before ?? ""}</span>
      <span className={gutter}>{line.after ?? ""}</span>
      <span className={cn("pr-2 break-words whitespace-pre-wrap", tint[line.kind])}>
        <span className="select-none">{sign[line.kind]}</span>
        {line.spans.map((span, at) =>
          span.changed ? (
            <mark key={at} className={cn("rounded-xs", emphasis[line.kind])}>
              {span.text}
            </mark>
          ) : (
            <span key={at}>{span.text}</span>
          ),
        )}
      </span>
    </>
  )
}

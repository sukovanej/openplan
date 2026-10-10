import {
  ArrowRight,
  ArrowUpDown,
  CornerLeftUp,
  FileText,
  FileWarning,
  GitPullRequest,
  Hash,
  Link2,
  type LucideIcon,
  MessageSquare,
  Pencil,
  Plus,
  SlidersHorizontal,
  Tag,
  Trash2,
  TriangleAlert,
  Type,
} from "lucide-react"
import { Fragment, type ReactNode } from "react"

import type {
  ProjectCodeChange,
  DocChange,
  DocumentChangeKind,
  FieldChange,
  TagChange,
  TagView,
  TaskChange,
} from "@openplan/api-client"
import { cn } from "@openplan/ui"

import { StatusBadge } from "./status"
import { TagChip } from "./tag-chip"
import { documentChangeText, fieldChangeText, renameText, setDifference } from "./task-change"
import { taskPath } from "./task-path"
import { TaskRefChip } from "./task-ref-chip"

// The project's tag registry by name, or `undefined` while it is still being read. Until it arrives
// a name the registry holds looks like one it does not, so the change keeps its words instead of
// showing every chip as dangling.
type Registry = ReadonlyMap<string, TagView> | undefined

const fieldIcons: Record<Exclude<FieldChange["field"], "status">, LucideIcon> = {
  number: Hash,
  parent: CornerLeftUp,
  order: ArrowUpDown,
  dependencies: Link2,
  tags: Tag,
  pull_requests: GitPullRequest,
  title: Type,
  description: FileText,
  comments: MessageSquare,
  conflicts: TriangleAlert,
  other: SlidersHorizontal,
  frontmatter: FileWarning,
}

const kindIcons: Record<DocumentChangeKind, { icon: LucideIcon; tint: string }> = {
  added: { icon: Plus, tint: "text-change-added" },
  modified: { icon: Pencil, tint: "text-muted-foreground" },
  removed: { icon: Trash2, tint: "text-change-deleted" },
}

function Marked({
  icon: Icon,
  tint = "text-muted-foreground",
  children,
}: {
  icon: LucideIcon
  tint?: string
  children: ReactNode
}) {
  return (
    <span className="inline-flex min-w-0 items-start gap-1.5">
      <span className="flex h-lh shrink-0 items-center">
        <Icon aria-hidden className={cn("size-3.5", tint)} />
      </span>
      <span>{children}</span>
    </span>
  )
}

type Sign = "+" | "−"

function SetChange({
  icon: Icon,
  change,
  chip,
}: {
  icon: LucideIcon
  change: Extract<FieldChange, { field: "tags" | "dependencies" }>
  chip: (item: string, sign: Sign) => ReactNode
}) {
  const { added, removed } = setDifference(change.from, change.to)
  if (added.length === 0 && removed.length === 0) return <Marked icon={Icon}>{fieldChangeText(change)}</Marked>
  const signed = [...added.map((item) => [item, "+"] as const), ...removed.map((item) => [item, "−"] as const)]
  return (
    <span className="inline-flex min-w-0 flex-wrap items-center gap-1.5">
      <Icon aria-hidden className="text-muted-foreground size-3.5 shrink-0" />
      {signed.map(([item, sign]) => (
        <Fragment key={`${sign}${item}`}>{chip(item, sign)}</Fragment>
      ))}
    </span>
  )
}

// A status change is its two badges: the words would only repeat them.
function FieldChangeView({ project, change, tags }: { project: string; change: FieldChange; tags: Registry }) {
  if (change.field === "tags" && tags !== undefined) {
    return (
      <SetChange
        icon={Tag}
        change={change}
        chip={(name, sign) => <TagChip name={name} tag={tags.get(name)} sign={sign} />}
      />
    )
  }
  if (change.field === "dependencies") {
    return (
      <SetChange
        icon={Link2}
        change={change}
        chip={(id, sign) => <TaskRefChip to={taskPath(project, id)} project={project} id={id} sign={sign} />}
      />
    )
  }
  if (change.field === "status") {
    return (
      <span className="inline-flex flex-wrap items-center gap-1.5">
        <StatusBadge status={change.from} />
        <ArrowRight aria-label="to" className="text-muted-foreground size-3.5 shrink-0" />
        <StatusBadge status={change.to} />
      </span>
    )
  }
  return <Marked icon={fieldIcons[change.field]}>{fieldChangeText(change)}</Marked>
}

function Changes({ className, children }: { className?: string; children: ReactNode }) {
  return <span className={cn("flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1", className)}>{children}</span>
}

export function DocumentChangeView({ kind, className }: { kind: DocumentChangeKind; className?: string }) {
  const { icon, tint } = kindIcons[kind]
  return (
    <Changes className={className}>
      <Marked icon={icon} tint={tint}>
        {documentChangeText(kind)}
      </Marked>
    </Changes>
  )
}

export function TaskChangeView({
  project,
  change,
  tags,
  className,
}: {
  project: string
  change: TaskChange
  tags: Registry
  className?: string
}) {
  const fields = change.fields ?? []
  if (change.kind !== "modified" || fields.length === 0) {
    return <DocumentChangeView kind={change.kind} className={className} />
  }
  return (
    <Changes className={className}>
      {fields.map((field) => (
        <FieldChangeView
          key={field.field === "other" ? `other:${field.name}` : field.field}
          project={project}
          change={field}
          tags={tags}
        />
      ))}
    </Changes>
  )
}

// A rename names the tag twice, as it was and as it is, so it is the one tag change that says which
// tag it is about.
export function TagChangeView({ change, tags, className }: { change: TagChange; tags: Registry; className?: string }) {
  const { renamed_from } = change
  if (renamed_from === undefined) return <DocumentChangeView kind={change.kind} className={className} />
  if (tags === undefined) {
    return (
      <Changes className={className}>
        <Marked icon={Type}>{renameText(change)}</Marked>
      </Changes>
    )
  }
  return (
    <Changes className={className}>
      <span className="inline-flex flex-wrap items-center gap-1.5">
        <Type aria-hidden className="text-muted-foreground size-3.5 shrink-0" />
        <span>Renamed</span>
        <TagChip name={renamed_from} tag={tags.get(renamed_from)} />
        <ArrowRight aria-label="to" className="text-muted-foreground size-3.5 shrink-0" />
        <TagChip name={change.tag} tag={tags.get(change.tag)} />
      </span>
    </Changes>
  )
}

export function DocChangeView({ change, className }: { change: DocChange; className?: string }) {
  if (change.renamed_from === undefined) return <DocumentChangeView kind={change.kind} className={className} />
  return (
    <Changes className={className}>
      <Marked icon={Type}>{renameText(change)}</Marked>
    </Changes>
  )
}

export function ProjectCodeChangeView({ change, className }: { change: ProjectCodeChange; className?: string }) {
  return (
    <Changes className={className}>
      <span className="inline-flex flex-wrap items-center gap-1.5">
        <Hash aria-hidden className="text-muted-foreground size-3.5 shrink-0" />
        <span>Task keys</span>
        <span className="font-mono text-xs">{change.from}</span>
        <ArrowRight aria-label="to" className="text-muted-foreground size-3.5 shrink-0" />
        <span className="font-mono text-xs">{change.to}</span>
      </span>
    </Changes>
  )
}

import {
  ArrowRight,
  ArrowUpDown,
  CornerLeftUp,
  FileText,
  FileWarning,
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
import type { ReactNode } from "react"

import type { DocumentChangeKind, FieldChange, TagChange, TagView, TaskChange } from "@openplan/api-client"
import { cn } from "@openplan/ui"

import { StatusBadge } from "./status"
import { TagChip } from "./tag-chip"
import { documentChangeText, fieldChangeText, setDifference, tagChangeText } from "./task-change"

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

function TagSetChange({
  change,
  tags,
}: {
  change: Extract<FieldChange, { field: "tags" }>
  tags: ReadonlyMap<string, TagView>
}) {
  const { added, removed } = setDifference(change.from, change.to)
  if (added.length === 0 && removed.length === 0) return <Marked icon={Tag}>{fieldChangeText(change)}</Marked>
  return (
    <span className="inline-flex min-w-0 flex-wrap items-center gap-1.5">
      <Tag aria-hidden className="text-muted-foreground size-3.5 shrink-0" />
      {added.map((name) => (
        <TagChip key={`+${name}`} name={name} tag={tags.get(name)} sign="+" />
      ))}
      {removed.map((name) => (
        <TagChip key={`−${name}`} name={name} tag={tags.get(name)} sign="−" />
      ))}
    </span>
  )
}

// A status change is its two badges: the words would only repeat them.
function FieldChangeView({ change, tags }: { change: FieldChange; tags: Registry }) {
  if (change.field === "tags" && tags !== undefined) return <TagSetChange change={change} tags={tags} />
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
  return <span className={cn("flex flex-wrap items-center gap-x-3 gap-y-1", className)}>{children}</span>
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
  change,
  tags,
  className,
}: {
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
        <Marked icon={Type}>{tagChangeText(change)}</Marked>
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

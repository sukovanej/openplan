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

import type { DocumentChangeKind, FieldChange, TagChange, TaskChange } from "@openplan/api-client"
import { cn } from "@openplan/ui"

import { StatusBadge } from "./status"
import { documentChangeText, fieldChangeText, tagChangeText } from "./task-change"

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
    <span className="inline-flex min-w-0 items-center gap-1.5">
      <Icon aria-hidden className={cn("size-3.5 shrink-0", tint)} />
      <span>{children}</span>
    </span>
  )
}

// A status change is its two badges: the words would only repeat them.
function FieldChangeView({ change }: { change: FieldChange }) {
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

export function TaskChangeView({ change, className }: { change: TaskChange; className?: string }) {
  const fields = change.fields ?? []
  if (change.kind !== "modified" || fields.length === 0) {
    return <DocumentChangeView kind={change.kind} className={className} />
  }
  return (
    <Changes className={className}>
      {fields.map((field) => (
        <FieldChangeView key={field.field === "other" ? `other:${field.name}` : field.field} change={field} />
      ))}
    </Changes>
  )
}

export function TagChangeView({ change, className }: { change: TagChange; className?: string }) {
  if (change.renamed_from === undefined) return <DocumentChangeView kind={change.kind} className={className} />
  return (
    <Changes className={className}>
      <Marked icon={Type}>{tagChangeText(change)}</Marked>
    </Changes>
  )
}

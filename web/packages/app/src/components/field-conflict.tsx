import { TriangleAlert } from "lucide-react"
import type { ReactNode } from "react"

import type { FrontmatterFields, Metadata, Status, TagView, TaskPatch } from "@openplan/api-client"
import {
  CONFLICT_TINT,
  type ConflictChoice,
  conflictCount,
  conflictedFields,
  FieldConflict,
  fieldConflict,
  type FieldName,
  frontmatterFields,
  StatusMark,
  statusLabel,
  TagChip,
} from "@openplan/task-ui"
import { absoluteTime, cn } from "@openplan/ui"

import { patchTask } from "../lib/api"
import { useProjectMutation } from "../lib/query-client"
import { useTags } from "../lib/tags"

export const FIELD_LABELS: Record<FieldName, string> = {
  status: "Status",
  created: "Created",
  parent: "Parent",
  rank: "Rank",
  dependencies: "Dependencies",
  tags: "Tags",
}

function Nothing({ children }: { children: ReactNode }) {
  return <span className="text-muted-foreground text-sm italic">{children}</span>
}

function Keys({ keys }: { keys: ReadonlyArray<string> }) {
  return <span className="font-mono text-xs break-all">{keys.join(", ")}</span>
}

function StatusValue({ status }: { status: Status }) {
  return (
    <span className="inline-flex items-center gap-1.5 text-sm">
      <StatusMark status={status} className="size-4" />
      {statusLabel(status)}
    </span>
  )
}

function TagList({ names, tags }: { names: ReadonlyArray<string>; tags: ReadonlyMap<string, TagView> | undefined }) {
  return (
    <span className="flex flex-wrap gap-1">
      {names.map((name) => (
        <TagChip key={name} name={name} tag={tags?.get(name)} />
      ))}
    </span>
  )
}

interface Versions<T> {
  readonly sides: ReadonlyArray<{ readonly label: string; readonly value: T }>
}

// `patch` is how a version writes back; a version it has no patch for offers no button.
function choicesOf<T>(
  conflict: Versions<T> | undefined,
  show: (value: T) => ReactNode,
  keep: (patch: TaskPatch) => void,
  patch: (value: T) => TaskPatch | undefined = () => undefined,
): ReadonlyArray<ConflictChoice> | undefined {
  return conflict?.sides.map((side) => {
    const write = patch(side.value)
    return { label: side.label, value: show(side.value), keep: write === undefined ? undefined : () => keep(write) }
  })
}

function fieldChoices(
  fields: FrontmatterFields,
  field: FieldName,
  keep: (patch: TaskPatch) => void,
  tags: ReadonlyMap<string, TagView> | undefined,
): ReadonlyArray<ConflictChoice> | undefined {
  switch (field) {
    case "status":
      return choicesOf(
        fieldConflict(fields.status),
        (status) => <StatusValue status={status} />,
        keep,
        (status) => ({ status }),
      )
    // The daemon writes `created` once and takes no patch for it.
    case "created":
      return choicesOf(fieldConflict(fields.created), (at) => <time dateTime={at}>{absoluteTime(at)}</time>, keep)
    case "parent":
      return choicesOf(
        fieldConflict(fields.parent),
        (parent) => (parent === null ? <Nothing>No parent</Nothing> : <Keys keys={[parent]} />),
        keep,
        (parent) => ({ parent }),
      )
    // A patch can set a rank but not clear one.
    case "rank":
      return choicesOf(
        fieldConflict(fields.rank),
        (rank) =>
          rank === null ? (
            <span className="flex flex-col gap-0.5">
              <Nothing>No rank</Nothing>
              <span className="text-muted-foreground text-xs">A write can set a rank, but it cannot remove one.</span>
            </span>
          ) : (
            <Keys keys={[rank]} />
          ),
        keep,
        (rank) => (rank === null ? undefined : { rank }),
      )
    case "dependencies":
      return choicesOf(
        fieldConflict(fields.dependencies),
        (keys) => (keys.length === 0 ? <Nothing>No dependencies</Nothing> : <Keys keys={keys} />),
        keep,
        (dependencies) => ({ dependencies }),
      )
    case "tags":
      return choicesOf(
        fieldConflict(fields.tags),
        (names) => (names.length === 0 ? <Nothing>No tags</Nothing> : <TagList names={names} tags={tags} />),
        keep,
        (names) => ({ tags: names }),
      )
  }
}

export function FieldConflictControl({
  project,
  id,
  metadata,
  field,
  trigger,
  align,
}: {
  project: string
  id: string
  metadata: Metadata
  field: FieldName
  trigger?: ReactNode
  align?: "start" | "end"
}) {
  const { mutate, isPending } = useProjectMutation(project)
  const { byName: tags } = useTags(project)
  const fields = frontmatterFields(metadata)
  const choices =
    fields === undefined
      ? undefined
      : fieldChoices(fields, field, (patch) => mutate(patchTask(project, id, patch)), tags)
  if (choices === undefined) return null
  return (
    <FieldConflict field={FIELD_LABELS[field]} choices={choices} pending={isPending} trigger={trigger} align={align} />
  )
}

// A count the daemon took from the fields and the body together, so what the fields do not account
// for is in the text.
export function ConflictBanner({
  project,
  id,
  metadata,
  count,
}: {
  project: string
  id: string
  metadata: Metadata
  count: number
}) {
  if (count === 0) return null
  const fields = conflictedFields(metadata)
  const passages = count - fields.length
  return (
    <div role="note" className={cn("mb-5 flex flex-col gap-2 rounded-md border px-3 py-2 text-xs", CONFLICT_TINT)}>
      <p className="flex items-center gap-1.5 font-medium">
        <TriangleAlert aria-hidden className="size-3.5 shrink-0" />
        {conflictCount(count)} from a sync. Pick a version of each.
      </p>
      {(fields.length > 0 || passages > 0) && (
        <div className="text-muted-foreground flex flex-wrap items-center gap-1.5">
          {fields.map((field) => (
            <FieldConflictControl
              key={field}
              project={project}
              id={id}
              metadata={metadata}
              field={field}
              trigger={FIELD_LABELS[field]}
            />
          ))}
          {passages > 0 && <span>{passages === 1 ? "1 passage" : `${passages} passages`} in the text below.</span>}
        </div>
      )}
    </div>
  )
}

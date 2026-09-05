import { Effect } from "effect"
import { Plus } from "lucide-react"
import { useCallback, useMemo, useState } from "react"

import type { Metadata, TagView } from "@openplan/api-client"
import { ColorDot, fieldFailure, fieldMessage, tagsOf, TaskTags } from "@openplan/task-ui"
import { Button, type ComboOption, Combobox, FuzzyText, Tooltip } from "@openplan/ui"

import { createTag, patchTask } from "../lib/api"
import { useDetailAction } from "../lib/detail-actions"
import { useProjectMutation, type Write } from "../lib/query-client"
import { tagMatches, tagsWith, tagsWithout, tagSpelled, useTags } from "../lib/tags"
import { Blocked } from "./blocked"

// Frontmatter that did not parse at all takes the tags down with it, so it is the same refusal to
// edit as a `tags:` line that did not: neither shows the reader the set a write would replace.
const unreadable = (metadata: Metadata): string | undefined => {
  if ("kind" in metadata) return metadata.message
  const failure = fieldFailure(metadata.tags)
  return failure === undefined ? undefined : fieldMessage(failure)
}

export function TagsField({
  project,
  id,
  metadata,
  branch,
  blocked,
  className,
}: {
  project: string
  id: string
  metadata: Metadata
  branch: string | undefined
  blocked: string | undefined
  className?: string
}) {
  const names = tagsOf(metadata)
  // The registry a write is validated against is the one on the branch the write lands on, so a name
  // that branch holds is never mistaken for a dangling one and pruned away. A branch no worktree can
  // write has no registry to read either, so those chips fall back to the served worktree's.
  const registry = blocked === undefined ? branch : undefined
  const { byName: tags, failed: registryFailed } = useTags(project, registry)
  const [adding, setAdding] = useState(false)
  const broken = unreadable(metadata)
  const mutation = useProjectMutation(project)

  useDetailAction("edit-tags", () => {
    if (blocked === undefined && broken === undefined && !mutation.isPending) setAdding(true)
  })
  const close = useCallback(() => setAdding(false), [])

  const editable = blocked === undefined && broken === undefined && tags !== undefined && !mutation.isPending
  return (
    <TaskTags
      metadata={metadata}
      tags={tags}
      branch={registry}
      onRemove={
        editable
          ? (name) => {
              const next = tagsWithout(names, tags, name)
              mutation.mutate(patchTask(project, id, { tags: next }, branch))
            }
          : undefined
      }
      trailing={
        registryFailed ? (
          <Unreadable
            what="registry"
            reason="This branch's tag registry could not be read, so the names on this task cannot be resolved."
          />
        ) : broken !== undefined ? (
          <Unreadable what="tags" reason={`The tags of this task cannot be read (${broken}).`} />
        ) : blocked !== undefined ? (
          <Blocked reason={blocked} />
        ) : adding && tags !== undefined ? (
          <TagPicker
            project={project}
            id={id}
            names={names}
            tags={tags}
            branch={branch}
            mutate={mutation.mutate}
            onClose={close}
          />
        ) : (
          <Button
            variant="accent"
            onClick={() => setAdding(true)}
            aria-label="Add tag"
            disabled={mutation.isPending}
            className={names.length > 0 ? "px-1.5" : undefined}
          >
            <Plus className="size-3.5" />
            {names.length === 0 && "Add tag"}
          </Button>
        )
      }
      className={className}
    />
  )
}

// Nothing the reader can see stands for the set a write would replace, so the field refuses the
// write and says which half is missing rather than showing a task with tags as a task with none.
function Unreadable({ what, reason }: { what: "tags" | "registry"; reason: string }) {
  return (
    <Tooltip content={`${reason} Repair it before editing this task's tags.`}>
      <span tabIndex={0} className="text-warning text-xs italic">
        {what} unreadable
      </span>
    </Tooltip>
  )
}

function TagOption({ tag, indices }: { tag: TagView; indices: ReadonlyArray<number> }) {
  return (
    <>
      <ColorDot color={tag.color} />
      <span className="shrink-0">
        <FuzzyText text={tag.display} indices={indices} />
      </span>
      {tag.description !== undefined && (
        <span className="text-muted-foreground/70 truncate text-xs">{tag.description}</span>
      )}
    </>
  )
}

// Only the registry can be picked from — a tag exists before a task can carry it. A name the registry
// does not hold is offered as a tag to register, so the whole trip stays in one box; the registry's
// own answer names the new tag, which keeps the normalization rule where it belongs.
function TagPicker({
  project,
  id,
  names,
  tags,
  branch,
  mutate,
  onClose,
}: {
  project: string
  id: string
  names: ReadonlyArray<string>
  tags: ReadonlyMap<string, TagView>
  branch: string | undefined
  mutate: (effect: Write) => void
  onClose: () => void
}) {
  const all = useMemo(() => [...tags.values()], [tags])
  const assigned = useMemo(() => new Set(names), [names])
  const buildOptions = useCallback(
    (query: string): ReadonlyArray<ComboOption> => {
      const options: ComboOption[] = tagMatches(all, query, assigned).map(({ tag, indices }) => ({
        key: tag.name,
        content: <TagOption tag={tag} indices={indices} />,
        onSelect: () => {
          const next = tagsWith(names, tags, tag.name)
          mutate(patchTask(project, id, { tags: next }, branch))
        },
      }))
      if (query === "") return options
      const spelled = tagSpelled(all, query)
      // A tag the task already carries is not among the matches and cannot be registered again, so
      // without this the box would answer an exact name with "type a name to register a tag".
      if (spelled !== undefined && assigned.has(spelled.name)) {
        options.push({
          key: " carried",
          content: (
            <span className="text-muted-foreground flex items-center gap-2">
              <ColorDot color={spelled.color} />
              {spelled.display} is already on this task
            </span>
          ),
          onSelect: onClose,
        })
      } else if (spelled === undefined) {
        options.push({
          key: " create",
          content: (
            <span className="flex items-center gap-2">
              <Plus className="text-muted-foreground size-4" />
              <span>
                Register <span className="font-medium">“{query}”</span> as a new tag
              </span>
            </span>
          ),
          onSelect: () =>
            mutate(
              Effect.flatMap(createTag(project, { name: query }, branch), (tag) =>
                patchTask(project, id, { tags: tagsWith(names, tags, tag.name) }, branch),
              ),
            ),
        })
      }
      return options
    },
    [all, assigned, names, tags, project, id, branch, mutate, onClose],
  )

  return (
    <Combobox
      placeholder="Add a tag…"
      buildOptions={buildOptions}
      onClose={onClose}
      emptyLabel="Type a name to register a tag"
      className="w-64"
    />
  )
}

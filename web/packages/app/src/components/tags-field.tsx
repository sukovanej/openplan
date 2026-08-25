import { Effect } from "effect"
import type { HttpClient } from "effect/unstable/http"
import { Plus } from "lucide-react"
import { useCallback, useEffect, useMemo, useState } from "react"

import type { Metadata, TagView } from "@open-planner/api-client"
import { ColorDot, fieldFailure, fieldMessage, tagsOf, TaskTags } from "@open-planner/task-ui"
import { Button, type ComboOption, Combobox, FuzzyText, Tooltip } from "@open-planner/ui"

import { createTag, patchTask } from "../lib/api"
import { useDetailAction } from "../lib/detail-actions"
import { runMutation, runTagMutation } from "../lib/store"
import { tagMatches, tagsWith, tagsWithout, tagSpelled, useTags } from "../lib/tags"
import { Blocked } from "./blocked"

type Write = Effect.Effect<unknown, unknown, HttpClient.HttpClient>

const unreadable = (metadata: Metadata) => ("kind" in metadata ? undefined : fieldFailure(metadata.tags))

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
  const tags = useTags(project, registry)
  const [adding, setAdding] = useState(false)
  // A tags write replaces the whole set, so a second one built from the set on screen would undo the
  // first. Nothing more is sent until this one has been answered.
  const [writing, setWriting] = useState(false)
  const broken = unreadable(metadata)

  useDetailAction("edit-tags", () => {
    if (blocked === undefined && broken === undefined) setAdding(true)
  })
  // A refresh can take the write target away while the picker stands open — another worktree took
  // the branch, or a merge started there. Close it rather than leave a control that cannot land.
  useEffect(() => {
    if (blocked !== undefined) setAdding(false)
  }, [blocked])

  const track = (done: Promise<unknown>) => {
    setWriting(true)
    void done.then(() => setWriting(false))
  }
  const write = (effect: Write) => track(runMutation(project, effect))
  const register = (effect: Write) => track(runTagMutation(project, effect))

  const editable = blocked === undefined && broken === undefined && tags !== undefined && !writing
  return (
    <TaskTags
      metadata={metadata}
      tags={tags}
      branch={registry}
      onRemove={
        editable ? (name) => write(patchTask(project, id, { tags: tagsWithout(names, tags, name) }, branch)) : undefined
      }
      trailing={
        broken !== undefined ? (
          <Unreadable reason={fieldMessage(broken)} />
        ) : blocked !== undefined ? (
          <Blocked reason={blocked} />
        ) : adding && tags !== undefined ? (
          <TagPicker
            project={project}
            id={id}
            names={names}
            tags={tags}
            branch={branch}
            write={write}
            register={register}
            onClose={() => setAdding(false)}
          />
        ) : (
          <Button
            variant="accent"
            onClick={() => setAdding(true)}
            aria-label="Add tag"
            disabled={writing}
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

// A `tags:` line the parser could not read shows no chips, so an edit would replace a set the reader
// was never shown. The file has to be repaired before the field can be trusted with a write.
function Unreadable({ reason }: { reason: string }) {
  return (
    <Tooltip content={`The tags of this task cannot be read (${reason}), so they cannot be edited here.`}>
      <span tabIndex={0} className="text-warning text-xs italic">
        tags unreadable
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
  write,
  register,
  onClose,
}: {
  project: string
  id: string
  names: ReadonlyArray<string>
  tags: ReadonlyMap<string, TagView>
  branch: string | undefined
  write: (effect: Write) => void
  register: (effect: Write) => void
  onClose: () => void
}) {
  const all = useMemo(() => [...tags.values()], [tags])
  const assigned = useMemo(() => new Set(names), [names])

  const buildOptions = useCallback(
    (query: string): ReadonlyArray<ComboOption> => {
      const options: ComboOption[] = tagMatches(all, query, assigned).map(({ tag, indices }) => ({
        key: tag.name,
        content: <TagOption tag={tag} indices={indices} />,
        onSelect: () => write(patchTask(project, id, { tags: tagsWith(names, tags, tag.name) }, branch)),
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
            register(
              Effect.flatMap(createTag(project, { name: query }, branch), (tag) =>
                patchTask(project, id, { tags: tagsWith(names, tags, tag.name) }, branch),
              ),
            ),
        })
      }
      return options
    },
    [all, assigned, names, tags, project, id, branch, write, register, onClose],
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

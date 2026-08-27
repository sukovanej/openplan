import { useMutation } from "@tanstack/react-query"
import { Effect } from "effect"
import type { HttpClient } from "effect/unstable/http"
import { Plus } from "lucide-react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"

import type { Metadata, TagView } from "@open-planner/api-client"
import { ColorDot, fieldFailure, fieldMessage, tagsOf, TaskTags } from "@open-planner/task-ui"
import { Button, type ComboOption, Combobox, FuzzyText, Tooltip } from "@open-planner/ui"

import { createTag, patchTask } from "../lib/api"
import { useDetailAction } from "../lib/detail-actions"
import { runtime } from "../lib/runtime"
import { tagMatches, tagsWith, tagsWithout, tagSpelled, useTags } from "../lib/tags"
import { Blocked } from "./blocked"

type Write = Effect.Effect<unknown, unknown, HttpClient.HttpClient>

// Frontmatter that did not parse at all takes the tags down with it, so it is the same refusal to
// edit as a `tags:` line that did not: neither shows the reader the set a write would replace.
const unreadable = (metadata: Metadata): string | undefined => {
  if ("kind" in metadata) return metadata.message
  const failure = fieldFailure(metadata.tags)
  return failure === undefined ? undefined : fieldMessage(failure)
}

const sameSet = (a: ReadonlyArray<string>, b: ReadonlyArray<string>) => {
  if (a.length !== b.length) return false
  const held = new Set(a)
  return b.every((name) => held.has(name))
}

// What tells the gate that a write has come all the way back: the set the task has to be carrying
// before another edit can be built on it. An assignment knows that set outright; registering a tag
// does not, because the registry names it — but it does know which names survive the prune, and that
// exactly one joins them.
type Landed = (names: ReadonlyArray<string>) => boolean

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
  // A tags write replaces the whole set, so a second one built from the set on screen would undo the
  // first. The write's own answer is not the signal to reopen: it only says the server took the
  // write, and the chips are still the ones from before until the re-read it triggers lands.
  const [writing, setWriting] = useState(false)
  const landed = useRef<Landed | null>(null)
  const broken = unreadable(metadata)
  const { mutate } = useMutation({
    mutationFn: (effect: Write) => runtime.runPromise(effect),
    meta: { project },
  })

  useDetailAction("edit-tags", () => {
    if (blocked === undefined && broken === undefined && !writing) setAdding(true)
  })
  // A refresh can take the write target away while the picker stands open — another worktree took
  // the branch, or a merge started there. Close it rather than leave a control that cannot land.
  useEffect(() => {
    if (blocked !== undefined) setAdding(false)
  }, [blocked])

  useEffect(() => {
    if (landed.current !== null && landed.current(names)) {
      landed.current = null
      setWriting(false)
    }
  }, [names])

  const start = useCallback((reached: Landed) => {
    landed.current = reached
    setWriting(true)
  }, [])
  const stop = useCallback(() => {
    landed.current = null
    setWriting(false)
  }, [])
  const write = useCallback(
    (reached: Landed, effect: Write) => {
      start(reached)
      mutate(effect, { onError: stop })
    },
    [mutate, start, stop],
  )
  const register = useCallback(
    (reached: Landed, effect: Write) => {
      start(reached)
      mutate(effect, { onError: stop })
    },
    [mutate, start, stop],
  )
  const close = useCallback(() => setAdding(false), [])

  const editable = blocked === undefined && broken === undefined && tags !== undefined && !writing
  return (
    <TaskTags
      metadata={metadata}
      tags={tags}
      branch={registry}
      onRemove={
        editable
          ? (name) => {
              const next = tagsWithout(names, tags, name)
              write((seen) => sameSet(seen, next), patchTask(project, id, { tags: next }, branch))
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
            write={write}
            register={register}
            onClose={close}
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
  write,
  register,
  onClose,
}: {
  project: string
  id: string
  names: ReadonlyArray<string>
  tags: ReadonlyMap<string, TagView>
  branch: string | undefined
  write: (landed: Landed, effect: Write) => void
  register: (landed: Landed, effect: Write) => void
  onClose: () => void
}) {
  const all = useMemo(() => [...tags.values()], [tags])
  const assigned = useMemo(() => new Set(names), [names])
  // What a write keeps of the current set: the dangling names go, whatever else the edit does.
  const kept = useMemo(() => names.filter((name) => tags.has(name)), [names, tags])

  const buildOptions = useCallback(
    (query: string): ReadonlyArray<ComboOption> => {
      const options: ComboOption[] = tagMatches(all, query, assigned).map(({ tag, indices }) => ({
        key: tag.name,
        content: <TagOption tag={tag} indices={indices} />,
        onSelect: () => {
          const next = tagsWith(names, tags, tag.name)
          write((seen) => sameSet(seen, next), patchTask(project, id, { tags: next }, branch))
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
            register(
              (seen) => seen.length === kept.length + 1 && kept.every((name) => seen.includes(name)),
              Effect.flatMap(createTag(project, { name: query }, branch), (tag) =>
                patchTask(project, id, { tags: tagsWith(names, tags, tag.name) }, branch),
              ),
            ),
        })
      }
      return options
    },
    [all, assigned, kept, names, tags, project, id, branch, write, register, onClose],
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

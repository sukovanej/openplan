import { Effect } from "effect"
import { Plus } from "lucide-react"
import { useCallback, useEffect, useMemo, useState } from "react"

import type { Metadata, TagView } from "@open-planner/api-client"
import { ColorDot, TagChip, tagsOf } from "@open-planner/task-ui"
import { Button, cn, type ComboOption, Combobox, FuzzyText } from "@open-planner/ui"

import { createTag, patchTask } from "../lib/api"
import { useDetailAction } from "../lib/detail-actions"
import { runMutation } from "../lib/store"
import { spellsTag, tagMatches, tagsWith, tagsWithout, useTags } from "../lib/tags"
import { Blocked } from "./blocked"

// The tags of a task, editable in place: a chip per name, a picker that adds one, and an `×` that
// takes one off. `blocked` is the reason no worktree can take the write, or `undefined` when one can.
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
  const tags = useTags(project)
  const [adding, setAdding] = useState(false)
  useDetailAction("edit-tags", () => {
    if (blocked === undefined) setAdding(true)
  })
  // A refresh can take the write target away while the picker stands open — another worktree took
  // the branch, or a merge started there. Close it rather than leave a control that cannot land.
  useEffect(() => {
    if (blocked !== undefined) setAdding(false)
  }, [blocked])

  const editable = blocked === undefined && tags !== undefined
  return (
    <div className={cn("flex flex-wrap items-center gap-1", className)}>
      {/* Until the registry arrives a name the branch does hold reads exactly like one it does not,
          so the chips wait rather than every one of them claiming to be dangling. */}
      {tags !== undefined &&
        names.map((name) => (
          <TagChip
            key={name}
            name={name}
            tag={tags.get(name)}
            onRemove={
              editable
                ? () =>
                    void runMutation(project, patchTask(project, id, { tags: tagsWithout(names, tags, name) }, branch))
                : undefined
            }
          />
        ))}
      {blocked !== undefined ? (
        <Blocked reason={blocked} />
      ) : adding && tags !== undefined ? (
        <TagPicker
          project={project}
          id={id}
          names={names}
          tags={tags}
          branch={branch}
          onClose={() => setAdding(false)}
        />
      ) : (
        <Button
          variant="accent"
          onClick={() => setAdding(true)}
          aria-label="Add tag"
          className={names.length > 0 ? "px-1.5" : undefined}
        >
          <Plus className="size-3.5" />
          {names.length === 0 && "Add tag"}
        </Button>
      )}
    </div>
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
  onClose,
}: {
  project: string
  id: string
  names: ReadonlyArray<string>
  tags: ReadonlyMap<string, TagView>
  branch: string | undefined
  onClose: () => void
}) {
  const all = useMemo(() => [...tags.values()], [tags])
  const assigned = useMemo(() => new Set(names), [names])

  const buildOptions = useCallback(
    (query: string): ReadonlyArray<ComboOption> => {
      const options: ComboOption[] = tagMatches(all, query, assigned).map(({ tag, indices }) => ({
        key: tag.name,
        content: <TagOption tag={tag} indices={indices} />,
        onSelect: () =>
          void runMutation(project, patchTask(project, id, { tags: tagsWith(names, tags, tag.name) }, branch)),
      }))
      if (query !== "" && !spellsTag(all, query)) {
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
            void runMutation(
              project,
              Effect.flatMap(createTag(project, { name: query }, branch), (tag) =>
                patchTask(project, id, { tags: tagsWith(names, tags, tag.name) }, branch),
              ),
            ),
        })
      }
      return options
    },
    [all, assigned, names, tags, project, id, branch],
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

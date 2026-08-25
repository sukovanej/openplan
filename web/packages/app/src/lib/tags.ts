import { useMemo } from "react"

import type { TagView } from "@open-planner/api-client"
import { fuzzyMatch } from "@open-planner/ui"

import { tagsQuery, useQuery } from "./store"

// `undefined` until the registry has been read, which a reader must not mistake for a registry that
// holds nothing: every name would then read as dangling.
export function useTags(project: string): ReadonlyMap<string, TagView> | undefined {
  const state = useQuery(tagsQuery(project))
  return useMemo(
    () => (state._tag === "success" ? new Map(state.value.map((tag) => [tag.name, tag])) : undefined),
    [state],
  )
}

// A tags write is validated as a whole set, so one name this branch's registry does not hold refuses
// the whole edit — including a name the task already carried. Every write therefore drops the
// dangling names, which is what the dangling chip's tooltip forewarns.
const kept = (names: ReadonlyArray<string>, tags: ReadonlyMap<string, TagView>, dropped: string) =>
  names.filter((name) => name !== dropped && tags.has(name))

export function tagsWith(
  names: ReadonlyArray<string>,
  tags: ReadonlyMap<string, TagView>,
  added: string,
): ReadonlyArray<string> {
  return [...kept(names, tags, added), added]
}

export function tagsWithout(
  names: ReadonlyArray<string>,
  tags: ReadonlyMap<string, TagView>,
  removed: string,
): ReadonlyArray<string> {
  return kept(names, tags, removed)
}

export interface TagMatch {
  readonly tag: TagView
  readonly indices: ReadonlyArray<number>
}

// Ranked matches over the registry, minus the names the task already carries. A tag is searched by
// the name it is displayed under, which is the one the reader sees on the chips.
export function tagMatches(
  tags: ReadonlyArray<TagView>,
  query: string,
  assigned: ReadonlySet<string>,
): ReadonlyArray<TagMatch> {
  const scored: Array<{ tag: TagView; score: number; indices: ReadonlyArray<number> }> = []
  for (const tag of tags) {
    if (assigned.has(tag.name)) continue
    const match = fuzzyMatch(query, tag.display)
    if (match !== null) scored.push({ tag, score: match.score, indices: match.indices })
  }
  scored.sort((a, b) => a.score - b.score || a.tag.name.localeCompare(b.tag.name))
  return scored.map(({ tag, indices }) => ({ tag, indices }))
}

// Not the normalization rule — that lives in the store and is the only thing that decides identity.
// This only keeps the picker from offering to create a name the list right above it already shows; a
// spelling that slips past it is refused as a duplicate.
export function spellsTag(tags: ReadonlyArray<TagView>, query: string): boolean {
  const wanted = query.toLowerCase()
  return tags.some((tag) => tag.name === wanted || tag.display.toLowerCase() === wanted)
}

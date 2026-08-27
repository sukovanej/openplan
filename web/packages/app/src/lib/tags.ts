import { useQuery } from "@tanstack/react-query"
import { useMemo } from "react"

import type { TagView } from "@open-planner/api-client"
import { fuzzyMatch } from "@open-planner/ui"

import { listTags } from "./api"
import { tagsKey } from "./query-client"
import { runtime } from "./runtime"

export interface Registry {
  // `undefined` until the registry has been read, which a reader must not mistake for a registry
  // that holds nothing: every name would then read as dangling.
  readonly byName: ReadonlyMap<string, TagView> | undefined
  // A registry that cannot be read is not one that is still loading. Both leave the names
  // unresolvable, but only one of them is ever going to resolve, and a surface that treats them
  // alike shows a task with tags as a task with none and says nothing about why.
  readonly failed: boolean
}

// `branch` must be the branch a tags write would land on, so that what the chips call dangling is
// what the write would refuse.
export function useTags(project: string, branch?: string): Registry {
  const query = useQuery({
    queryKey: tagsKey(project, branch),
    queryFn: () => runtime.runPromise(listTags(project, branch)),
  })
  return useMemo(
    () => ({
      byName: query.data === undefined ? undefined : new Map(query.data.map((tag) => [tag.name, tag])),
      failed: query.isError,
    }),
    [query.data, query.isError],
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

// A tag is searched by the name it is displayed under, which is the one the chips show.
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
// This only keeps the picker from offering to register a name the registry already holds; a spelling
// that slips past it is refused as a duplicate.
export function tagSpelled(tags: ReadonlyArray<TagView>, query: string): TagView | undefined {
  const wanted = query.toLowerCase()
  return tags.find((tag) => tag.name === wanted || tag.display.toLowerCase() === wanted)
}

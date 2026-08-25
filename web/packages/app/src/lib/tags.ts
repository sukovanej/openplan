import { useMemo } from "react"

import type { TagView } from "@open-planner/api-client"

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

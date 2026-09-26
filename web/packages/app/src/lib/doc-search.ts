import type { DocListItem } from "@openplan/api-client"
import { fuzzyMatch } from "@openplan/ui"

const MAX_MATCHES = 8

export interface DocMatch {
  readonly doc: DocListItem
  readonly indices: ReadonlyArray<number>
}

export const docLabel = (doc: DocListItem): string => doc.title || doc.name

export function docMatches(docs: ReadonlyArray<DocListItem>, query: string, excluded: Set<string>): DocMatch[] {
  const scored: Array<{ doc: DocListItem; score: number; indices: ReadonlyArray<number> }> = []
  for (const doc of docs) {
    if (excluded.has(doc.name)) continue
    const match = fuzzyMatch(query, docLabel(doc))
    if (match !== null) scored.push({ doc, score: match.score, indices: match.indices })
  }
  scored.sort((a, b) => a.score - b.score || docLabel(a.doc).localeCompare(docLabel(b.doc)))
  return scored.slice(0, MAX_MATCHES).map(({ doc, indices }) => ({ doc, indices }))
}

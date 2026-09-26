import type { DocListItem } from "@openplan/api-client"
import { docParentOf } from "@openplan/task-ui"

export interface DocTreeRow {
  readonly doc: DocListItem
  readonly depth: number
}

const coordinate = (project: string, name: string) => `${project}\u0000${name}`

// The docs in tree order, each with its depth. A parent names a doc in the same store, so the tree
// is built per project even when the list spans several. A doc whose parent is not in the list
// stands at the top level: the parent is gone, and hiding the child would lose it.
export function docTree(docs: ReadonlyArray<DocListItem>): ReadonlyArray<DocTreeRow> {
  const children = new Map<string, Array<DocListItem>>()
  const held = new Set(docs.map((doc) => coordinate(doc.project, doc.name)))
  const roots: Array<DocListItem> = []
  for (const doc of docs) {
    const named = docParentOf(doc.metadata)
    const parent = named === undefined ? undefined : coordinate(doc.project, named)
    if (parent === undefined || !held.has(parent)) {
      roots.push(doc)
      continue
    }
    const bucket = children.get(parent)
    if (bucket === undefined) children.set(parent, [doc])
    else bucket.push(doc)
  }

  const rows: Array<DocTreeRow> = []
  // A cycle the files carry would otherwise walk forever, and it can only reach a doc already out.
  const seen = new Set<string>()
  const walk = (doc: DocListItem, depth: number) => {
    const key = coordinate(doc.project, doc.name)
    if (seen.has(key)) return
    seen.add(key)
    rows.push({ doc, depth })
    for (const child of children.get(key) ?? []) walk(child, depth + 1)
  }
  for (const root of roots) walk(root, 0)
  // A cycle among the files has no root, so nothing above reaches it. Those docs are still docs, and
  // the list is the only place a reader can see them to break the cycle.
  for (const doc of docs) walk(doc, 0)
  return rows
}

import type { DocChange, DocumentChangeKind, FieldChange, PullRequestView, TagChange } from "@openplan/api-client"

import { statusLabel } from "./status"

const count = (n: number, noun: string): string => `${n} ${noun}${n === 1 ? "" : "s"}`

// Tags and dependencies are sets to a reader, so the change is what joined and what left.
export function setDifference(
  from: ReadonlyArray<string>,
  to: ReadonlyArray<string>,
): { added: ReadonlyArray<string>; removed: ReadonlyArray<string> } {
  return { added: to.filter((item) => !from.includes(item)), removed: from.filter((item) => !to.includes(item)) }
}

function setChange(name: string, from: ReadonlyArray<string>, to: ReadonlyArray<string>): string {
  const { added, removed } = setDifference(from, to)
  const moved = [...added.map((item) => `+${item}`), ...removed.map((item) => `−${item}`)]
  return moved.length === 0 ? `${name} reordered` : `${name}: ${moved.join(" ")}`
}

function linkChange(from: ReadonlyArray<PullRequestView>, to: ReadonlyArray<PullRequestView>): string {
  const short = (list: ReadonlyArray<PullRequestView>) => list.map((each) => each.short)
  const { added, removed } = setDifference(short(from), short(to))
  const moved = [...added.map((item) => `linked ${item}`), ...removed.map((item) => `unlinked ${item}`)].join(", ")
  return moved === "" ? "Pull requests" : moved.charAt(0).toUpperCase() + moved.slice(1)
}

function fromTo(name: string, from: string | undefined, to: string | undefined): string {
  if (to === undefined) return `${name} removed`
  return from === undefined ? `${name}: ${to}` : `${name}: ${from} → ${to}`
}

export function fieldChangeText(change: FieldChange): string {
  switch (change.field) {
    case "number":
      return `Moved from ${change.from}`
    case "status":
      return `Status: ${statusLabel(change.from)} → ${statusLabel(change.to)}`
    case "parent":
      return fromTo("Parent", change.from, change.to)
    case "order":
      return "Order"
    case "dependencies":
      return setChange("Dependencies", change.from, change.to)
    case "tags":
      return setChange("Tags", change.from, change.to)
    case "pull_requests":
      return linkChange(change.from, change.to)
    case "title":
      return "Title"
    case "description":
      return "Description"
    case "comments":
      return change.removed === 0 ? count(change.added, "comment") : "Comments edited"
    case "conflicts":
      return change.to === 0 ? "Conflicts resolved" : count(change.to, "conflict")
    case "other":
      return `Field ${change.name}`
    case "frontmatter":
      return "Unreadable frontmatter"
  }
}

const kindText: Record<DocumentChangeKind, string> = { added: "Created", modified: "Edited", removed: "Deleted" }

export const documentChangeText = (kind: DocumentChangeKind): string => kindText[kind]

export function renameText(change: TagChange | DocChange): string {
  return change.renamed_from === undefined ? documentChangeText(change.kind) : `Renamed from ${change.renamed_from}`
}

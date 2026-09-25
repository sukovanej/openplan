import type { FieldChange, TagChange, TaskChange } from "@openplan/api-client"

import { statusLabel } from "./status"

const quoted = (text: string): string => `“${text}”`

const count = (n: number, noun: string): string => `${n} ${noun}${n === 1 ? "" : "s"}`

// Tags and dependencies are sets to a reader, so the change is what joined and what left.
function setChange(name: string, from: ReadonlyArray<string>, to: ReadonlyArray<string>): string {
  const added = to.filter((item) => !from.includes(item)).map((item) => `+${item}`)
  const removed = from.filter((item) => !to.includes(item)).map((item) => `−${item}`)
  const moved = [...added, ...removed]
  return moved.length === 0 ? `${name} reordered` : `${name}: ${moved.join(" ")}`
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
    case "title":
      return fromTo(
        "Title",
        change.from === undefined ? undefined : quoted(change.from),
        change.to === undefined ? undefined : quoted(change.to),
      )
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

export function taskChangeText(change: TaskChange): string {
  switch (change.kind) {
    case "added":
      return "Created"
    case "removed":
      return change.title === undefined ? "Deleted" : `Deleted ${quoted(change.title)}`
    case "modified":
      return change.fields === undefined || change.fields.length === 0
        ? "Edited"
        : change.fields.map(fieldChangeText).join(" · ")
  }
}

export function tagChangeText(change: TagChange): string {
  if (change.renamed_from !== undefined) return `Tag ${change.renamed_from} renamed to ${change.tag}`
  const verb = { added: "created", modified: "edited", removed: "deleted" }[change.kind]
  return `Tag ${change.tag} ${verb}`
}

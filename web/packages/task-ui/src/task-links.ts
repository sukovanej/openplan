import { docPath, taskPath, taskReference } from "./task-path"

type Text = { type: "text"; value: string }
type Link = { type: "link"; url: string; title: null; children: Text[] }
type Node = { type: string; value?: string; children?: Node[] }

// The API names a task by its key. A task file names it by its path, whose leading digits are the
// number the store allocated (`op_task::reference`). Nothing else is a task reference, so ordinary
// `[[...]]` bracket text, a bare number, and another store's key all stay literal.
const TASK_REF = /\[\[([^[\]\n]+)\]\]/g
const TASK_FILE = /^(?:\.\/|\.\.\/tasks\/)?([0-9]+)(?:-[^/]*)?\.md$/
const NUMBER = /^(?:0|[1-9][0-9]*)$/
// A doc is named by its file stem, which is the whole id. A `[[…]]` this store does not spell as a
// task and that reads as a doc name is a doc reference; anything else stays literal. A doc name is
// what the daemon's normalizer leaves as it is, so two hyphens in a row name no doc. Bare digits name
// neither: the store refuses to write a body that spells a task that way.
const DOC_NAME = /^[a-z0-9]+(?:-[a-z0-9]+)*-?$/
const DIGITS = /^[0-9]+$/
// A comment keeps the file form, where a task file names a doc by its path.
const DOC_FILE = /^\.\.\/docs\/([^/]+)\.md$/

// A body's references name tasks of the store the body lives in, so they resolve in that project.
export interface TaskLinkSource {
  readonly project: string
  readonly abbreviation: string | undefined
}

function refKey(target: string, abbreviation: string): string | null {
  const file = TASK_FILE.exec(target)
  if (file !== null) return `${abbreviation}-${Number(file[1])}`
  const prefix = `${abbreviation}-`
  return target.startsWith(prefix) && NUMBER.test(target.slice(prefix.length)) ? target : null
}

function text(value: string): Text {
  return { type: "text", value }
}

function link(url: string, label: string): Link {
  return { type: "link", url, title: null, children: [text(label)] }
}

export type Referenced =
  | { readonly kind: "task"; readonly id: string; readonly section: string | undefined }
  | { readonly kind: "doc"; readonly name: string; readonly section: string | undefined }

// The text between `[[` and `]]`, resolved in the store `abbreviation` names: this store's key names
// a task, and a doc name names a doc.
export function referenced(inner: string, abbreviation: string): Referenced | null {
  const { id: target, section } = taskReference(inner.trim())
  const id = refKey(target, abbreviation)
  if (id !== null) return { kind: "task", id, section }
  const name = DOC_FILE.exec(target)?.[1] ?? target
  return DOC_NAME.test(name) && !DIGITS.test(name) ? { kind: "doc", name, section } : null
}

export function referencePath(project: string, reference: Referenced): string {
  return reference.kind === "task"
    ? taskPath(project, reference.id, reference.section)
    : docPath(project, reference.name, reference.section)
}

export function taskRefMatches(value: string): Iterable<RegExpExecArray> {
  return value.matchAll(TASK_REF)
}

// Without the store's abbreviation nothing can be told from another store's spelling, so every
// reference stays literal until the config arrives — and re-renders once it has.
export function splitTaskRefs(value: string, source: TaskLinkSource): Array<Text | Link> | null {
  const { project, abbreviation } = source
  if (abbreviation === undefined) return null
  const nodes: Array<Text | Link> = []
  let last = 0
  for (const match of taskRefMatches(value)) {
    const inner = match[1].trim()
    const reference = referenced(inner, abbreviation)
    if (reference === null) continue
    const url = referencePath(project, reference)
    const start = match.index
    if (start > last) nodes.push(text(value.slice(last, start)))
    nodes.push(link(url, inner))
    last = start + match[0].length
  }
  if (nodes.length === 0) return null
  if (last < value.length) nodes.push(text(value.slice(last)))
  return nodes
}

// Text nested in existing links or code must stay literal: a link inside a link is invalid, and a
// `[[id]]` inside code is quoted source, not a reference.
const OPAQUE = new Set(["link", "linkReference", "inlineCode", "code"])

function walk(node: Node, source: TaskLinkSource): void {
  const children = node.children
  if (children === undefined) return
  for (let i = 0; i < children.length; i++) {
    const child = children[i]
    if (child.type === "text") {
      const replaced = splitTaskRefs(child.value ?? "", source)
      if (replaced !== null) {
        children.splice(i, 1, ...replaced)
        i += replaced.length - 1
      }
    } else if (!OPAQUE.has(child.type)) {
      walk(child, source)
    }
  }
}

// A remark attacher: unified calls it with the options from the `[plugin, options]` entry to get the
// transformer, so the source reaches `walk` without the plugin reading a store of its own.
export function remarkTaskLinks(source: TaskLinkSource) {
  return (tree: Node): void => walk(tree, source)
}

export function taskLinkPlugins(source: TaskLinkSource): [typeof remarkTaskLinks, TaskLinkSource] {
  return [remarkTaskLinks, source]
}

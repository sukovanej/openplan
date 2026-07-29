type Text = { type: "text"; value: string }
type Link = { type: "link"; url: string; title: null; children: Text[] }
type Node = { type: string; value?: string; children?: Node[] }

// A task file names another by its path, and the leading digits of that file name are the id
// (`op_task::ref_id`). A bare id is accepted the same way the Rust side accepts it, and nothing else
// is, so ordinary `[[...]]` bracket text is not mistaken for a task reference.
const TASK_REF = /\[\[([^[\]\n]+)\]\]/g
const TASK_FILE = /^(?:.*\/)?([0-9]+)(?:-[^/]*)?\.md$/
const TASK_ID = /^(?:0|[1-9][0-9]*)$/

function refId(target: string): string | null {
  const file = TASK_FILE.exec(target)
  if (file !== null) return String(Number(file[1]))
  return TASK_ID.test(target) ? target : null
}

function text(value: string): Text {
  return { type: "text", value }
}

function link(url: string, label: string): Link {
  return { type: "link", url, title: null, children: [text(label)] }
}

export function splitTaskRefs(value: string): Array<Text | Link> | null {
  const nodes: Array<Text | Link> = []
  let last = 0
  for (const match of value.matchAll(TASK_REF)) {
    const inner = match[1].trim()
    const hash = inner.indexOf("#")
    const section = hash === -1 ? "" : inner.slice(hash + 1)
    const id = refId(hash === -1 ? inner : inner.slice(0, hash))
    if (id === null) continue
    const start = match.index
    if (start > last) nodes.push(text(value.slice(last, start)))
    const url = section ? `/task/${id}#${encodeURIComponent(section)}` : `/task/${id}`
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

function walk(node: Node): void {
  const children = node.children
  if (children === undefined) return
  for (let i = 0; i < children.length; i++) {
    const child = children[i]
    if (child.type === "text") {
      const replaced = splitTaskRefs(child.value ?? "")
      if (replaced !== null) {
        children.splice(i, 1, ...replaced)
        i += replaced.length - 1
      }
    } else if (!OPAQUE.has(child.type)) {
      walk(child)
    }
  }
}

export function remarkTaskLinks() {
  return (tree: Node): void => walk(tree)
}

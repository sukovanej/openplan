type Text = { type: "text"; value: string }
type Link = { type: "link"; url: string; title: null; children: Text[] }
type Node = { type: string; value?: string; children?: Node[] }

// A task file names another by its path, whose leading digits are the number the store allocated,
// and a human may write this store's key instead (`op_task::body_ref_id`). Nothing else is a
// reference — so ordinary `[[...]]` bracket text, a bare number, and another store's key all stay
// literal. Either spelling resolves to the key, which is the id above the store.
const TASK_REF = /\[\[([^[\]\n]+)\]\]/g
const TASK_FILE = /^(?:.*\/)?([0-9]+)(?:-[^/]*)?\.md$/
const NUMBER = /^(?:0|[1-9][0-9]*)$/

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

// Without the store's abbreviation nothing can be told from another store's spelling, so every
// reference stays literal until the config arrives — and re-renders once it has.
export function splitTaskRefs(value: string, abbreviation: string | undefined): Array<Text | Link> | null {
  if (abbreviation === undefined) return null
  const nodes: Array<Text | Link> = []
  let last = 0
  for (const match of value.matchAll(TASK_REF)) {
    const inner = match[1].trim()
    const hash = inner.indexOf("#")
    const section = hash === -1 ? "" : inner.slice(hash + 1)
    const id = refKey(hash === -1 ? inner : inner.slice(0, hash), abbreviation)
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

function walk(node: Node, abbreviation: string | undefined): void {
  const children = node.children
  if (children === undefined) return
  for (let i = 0; i < children.length; i++) {
    const child = children[i]
    if (child.type === "text") {
      const replaced = splitTaskRefs(child.value ?? "", abbreviation)
      if (replaced !== null) {
        children.splice(i, 1, ...replaced)
        i += replaced.length - 1
      }
    } else if (!OPAQUE.has(child.type)) {
      walk(child, abbreviation)
    }
  }
}

// A remark attacher: unified calls it with the options from the `[plugin, options]` entry to get the
// transformer, so the abbreviation reaches `walk` without the plugin reading a store of its own.
export function remarkTaskLinks(abbreviation: string | undefined) {
  return (tree: Node): void => walk(tree, abbreviation)
}

export function taskLinkPlugins(abbreviation: string | undefined): [typeof remarkTaskLinks, string | undefined] {
  return [remarkTaskLinks, abbreviation]
}

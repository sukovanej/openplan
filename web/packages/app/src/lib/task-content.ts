// A body is the `# ` title line and the description under it. The page edits the two apart, and
// every byte it did not change goes back as it came, so an untouched body is equal to the one read.
export interface TaskContent {
  readonly head: string
  readonly title: string | undefined
  readonly titleEnd: string
  readonly description: string
}

const TITLE = "# "

export function splitBody(body: string): TaskContent {
  let offset = 0
  for (const line of body.match(/[^\n]*\n|[^\n]+$/g) ?? []) {
    if (line.trim() === "") {
      offset += line.length
      continue
    }
    if (!line.startsWith(TITLE)) break
    const titleEnd = line.endsWith("\n") ? "\n" : ""
    return {
      head: body.slice(0, offset),
      title: line.slice(TITLE.length, line.length - titleEnd.length),
      titleEnd,
      description: body.slice(offset + line.length),
    }
  }
  return { head: "", title: undefined, titleEnd: "", description: body }
}

// `title` is `undefined` while the reader has not touched the title of a body that has no title line.
export function joinBody(content: TaskContent, title: string | undefined, description: string): string {
  if (title === undefined) return `${content.head}${description}`
  const adding = content.title === undefined || content.description === ""
  const separator = adding && description !== "" && !description.startsWith("\n") ? "\n" : ""
  const end = description === "" ? content.titleEnd : "\n"
  return `${content.head}${TITLE}${title}${end}${separator}${description}`
}

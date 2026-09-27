const TASK_SEGMENT = "task"
const TAGS_SEGMENT = "tags"
const ACTIVITY_SEGMENT = "activity"
const DOC_SEGMENT = "doc"
const DOCS_SEGMENT = "docs"
const FLOW_SEGMENT = "flow"

export const REVISION_PARAM = "revision"

// The flow is not a read of one project: one query can name several, so it sits above them all and
// carries its whole selection in the query string.
export const FLOW_ROUTE = `/${FLOW_SEGMENT}`

// A page with no project in its path is the page of every project: `/` and `/docs` hold what
// `/:project` and `/:project/docs` hold for one.
export const BOARD_ROUTE = "/:project"
export const TASK_ROUTE = `${BOARD_ROUTE}/${TASK_SEGMENT}/:id`
export const DOC_ROUTE = `${BOARD_ROUTE}/${DOC_SEGMENT}/:name`
export const DOCS_ROUTE = `/${DOCS_SEGMENT}`
export const PROJECT_DOCS_ROUTE = `${BOARD_ROUTE}/${DOCS_SEGMENT}`
export const TAGS_ROUTE = `/${TAGS_SEGMENT}`
export const PROJECT_TAGS_ROUTE = `${BOARD_ROUTE}/${TAGS_SEGMENT}`
export const ACTIVITY_ROUTE = `/${ACTIVITY_SEGMENT}`
export const PROJECT_ACTIVITY_ROUTE = `${BOARD_ROUTE}/${ACTIVITY_SEGMENT}`

// The registry refuses these as project names, so no project hides behind one.
const ABOVE_PROJECTS: ReadonlySet<string> = new Set([DOCS_SEGMENT, FLOW_SEGMENT, TAGS_SEGMENT, ACTIVITY_SEGMENT])

// Two stores can commit the same abbreviation, so a key names a task only inside its project. Every
// task URL therefore carries the project, and every helper here takes it.
export interface TaskRoute {
  readonly project: string
  readonly id: string
}

export function boardPath(project?: string): string {
  return project === undefined ? "/" : `/${encodeURIComponent(project)}`
}

function pagePath(segment: string, project: string | undefined): string {
  return project === undefined ? `/${segment}` : `${boardPath(project)}/${segment}`
}

export const docsPath = (project?: string) => pagePath(DOCS_SEGMENT, project)
export const tagsPath = (project?: string) => pagePath(TAGS_SEGMENT, project)
export const activityPath = (project?: string) => pagePath(ACTIVITY_SEGMENT, project)

// A doc's name is its whole id, so its URL carries the name the way a task URL carries its key.
export function docPath(project: string, name: string, section?: string): string {
  const path = `${boardPath(project)}/${DOC_SEGMENT}/${encodeURIComponent(name)}`
  return section === undefined ? path : `${path}#${encodeURIComponent(section)}`
}

export function docRouteOf(path: string): { project: string; name: string } | undefined {
  const [, project, segment, rest] = path.split("/")
  if (project === undefined || project === "" || segment !== DOC_SEGMENT || rest === undefined) return undefined
  const name = rest.split(/[#?]/, 1)[0]
  return name === "" ? undefined : { project: decodeURIComponent(project), name: decodeURIComponent(name) }
}

export function taskPath(project: string, id: string, section?: string): string {
  const path = `${boardPath(project)}/${TASK_SEGMENT}/${id}`
  return section === undefined ? path : `${path}#${encodeURIComponent(section)}`
}

export function docRevisionPath(project: string, name: string, revision: string): string {
  return `${docPath(project, name)}?${REVISION_PARAM}=${encodeURIComponent(revision)}`
}

export function revisionPath(project: string, id: string, revision: string): string {
  return `${taskPath(project, id)}?${REVISION_PARAM}=${encodeURIComponent(revision)}`
}

// A reference may aim at a section (`OPP-42#Design`). The task it names is the part before the `#`.
export function taskReference(reference: string): { id: string; section: string | undefined } {
  const hash = reference.indexOf("#")
  return hash === -1
    ? { id: reference, section: undefined }
    : { id: reference.slice(0, hash), section: reference.slice(hash + 1) || undefined }
}

export function projectOfPath(path: string): string | undefined {
  const [, project] = path.split("/")
  if (project === undefined || project === "" || ABOVE_PROJECTS.has(project)) return undefined
  return decodeURIComponent(project)
}

function isPage(path: string, segment: string): boolean {
  const parts = path.split("/").filter((part) => part !== "")
  return parts.length === 1 ? parts[0] === segment : parts.length === 2 && parts[1] === segment
}

export const isDocsPath = (path: string) => isPage(path, DOCS_SEGMENT) || docRouteOf(path) !== undefined
export const isTagsPath = (path: string) => isPage(path, TAGS_SEGMENT)
export const isActivityPath = (path: string) => isPage(path, ACTIVITY_SEGMENT)

export function taskRouteOf(path: string): TaskRoute | undefined {
  const [, project, segment, rest] = path.split("/")
  if (project === undefined || project === "" || segment !== TASK_SEGMENT || rest === undefined) return undefined
  const id = rest.split(/[#?]/, 1)[0]
  return id === "" ? undefined : { project: decodeURIComponent(project), id }
}

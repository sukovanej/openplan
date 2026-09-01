const TASK_SEGMENT = "task"
const TAGS_SEGMENT = "tags"

// The flow is not a read of one project: one query can name several, so it sits above them all and
// carries its whole selection in the query string.
export const FLOW_ROUTE = "/flow"

export const BOARD_ROUTE = "/:project"
export const TASK_ROUTE = `${BOARD_ROUTE}/${TASK_SEGMENT}/:id`
export const TAGS_ROUTE = `${BOARD_ROUTE}/${TAGS_SEGMENT}`

// Two stores can commit the same abbreviation, so a key names a task only inside its project. Every
// task URL therefore carries the project, and every helper here takes it.
export interface TaskRoute {
  readonly project: string
  readonly id: string
}

export function boardPath(project: string): string {
  return `/${encodeURIComponent(project)}`
}

export function tagsPath(project: string): string {
  return `${boardPath(project)}/${TAGS_SEGMENT}`
}

export function taskPath(project: string, id: string, section?: string): string {
  const path = `${boardPath(project)}/${TASK_SEGMENT}/${id}`
  return section === undefined ? path : `${path}#${encodeURIComponent(section)}`
}

// A reference may aim at a section (`OPP-42#Design`). The task it names is the part before the `#`.
export function taskReference(reference: string): { id: string; section: string | undefined } {
  const hash = reference.indexOf("#")
  return hash === -1
    ? { id: reference, section: undefined }
    : { id: reference.slice(0, hash), section: reference.slice(hash + 1) || undefined }
}

export function taskRouteOf(path: string): TaskRoute | undefined {
  const [, project, segment, rest] = path.split("/")
  if (project === undefined || project === "" || segment !== TASK_SEGMENT || rest === undefined) return undefined
  const id = rest.split(/[#?]/, 1)[0]
  return id === "" ? undefined : { project: decodeURIComponent(project), id }
}

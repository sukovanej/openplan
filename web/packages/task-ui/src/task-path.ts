const TASK_SEGMENT = "task"
const TAGS_SEGMENT = "tags"
const AGENT_SEGMENT = "agent"
const SESSION_PARAM = "session"

// The flow is not a read of one project: one query can name several, so it sits above them all and
// carries its whole selection in the query string.
export const FLOW_ROUTE = "/flow"

export const BOARD_ROUTE = "/:project"
export const TASK_ROUTE = `${BOARD_ROUTE}/${TASK_SEGMENT}/:id`
export const TAGS_ROUTE = `${BOARD_ROUTE}/${TAGS_SEGMENT}`
// The agent page without a task drafts one; with a task it edits that one. `?session=` names the
// daemon session the page is attached to, so a reload comes back to the same chat.
export const AGENT_ROUTE = `${BOARD_ROUTE}/${AGENT_SEGMENT}`
export const AGENT_TASK_ROUTE = `${AGENT_ROUTE}/:id`

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

export function agentPath(project: string, id?: string, session?: string): string {
  const base = `${boardPath(project)}/${AGENT_SEGMENT}`
  const path = id === undefined ? base : `${base}/${id}`
  return session === undefined ? path : `${path}?${SESSION_PARAM}=${encodeURIComponent(session)}`
}

export interface AgentRoute {
  readonly project: string
  readonly id: string | undefined
}

export function agentRouteOf(path: string): AgentRoute | undefined {
  const [, project, segment, rest] = path.split("/")
  if (project === undefined || project === "" || segment !== AGENT_SEGMENT) return undefined
  const id = rest === undefined ? "" : rest.split(/[#?]/, 1)[0]
  return { project: decodeURIComponent(project), id: id === "" ? undefined : id }
}

// The project a board, tags, or agent route names: the first segment. The flow sits above every
// project, and the merged board names none.
export function projectRouteOf(path: string): string | undefined {
  const [pathname] = path.split(/[#?]/, 1)
  if (pathname === FLOW_ROUTE) return undefined
  const [, project] = pathname.split("/")
  return project === undefined || project === "" ? undefined : decodeURIComponent(project)
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

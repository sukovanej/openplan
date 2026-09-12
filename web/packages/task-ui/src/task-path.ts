const TASK_SEGMENT = "task"
const TAGS_SEGMENT = "tags"
const AGENT_SEGMENT = "agent"
const SESSION_PARAM = "session"
const PROJECT_PARAM = "project"

// The flow is not a read of one project: one query can name several, so it sits above them all and
// carries its whole selection in the query string.
export const FLOW_ROUTE = "/flow"

export const BOARD_ROUTE = "/:project"
export const TASK_ROUTE = `${BOARD_ROUTE}/${TASK_SEGMENT}/:id`
export const TAGS_ROUTE = `${BOARD_ROUTE}/${TAGS_SEGMENT}`
// The agent page drafts a task. It sits above the projects like the flow does, because the project
// is chosen on the page; `?project=` preselects one. Editing a task is the task's own page with a
// session on it: `?session=` names the daemon session, so a reload comes back to the same chat.
export const AGENT_ROUTE = `/${AGENT_SEGMENT}`

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

export function taskSessionPath(project: string, id: string, session: string): string {
  return `${taskPath(project, id)}?${SESSION_PARAM}=${encodeURIComponent(session)}`
}

export function agentPath(project?: string, session?: string): string {
  const params = new URLSearchParams()
  if (project !== undefined) params.set(PROJECT_PARAM, project)
  if (session !== undefined) params.set(SESSION_PARAM, session)
  const query = params.toString()
  return query === "" ? AGENT_ROUTE : `${AGENT_ROUTE}?${query}`
}

export function isAgentRoute(path: string): boolean {
  const [pathname] = path.split(/[#?]/, 1)
  return pathname === AGENT_ROUTE || pathname === `${AGENT_ROUTE}/`
}

// The project a board, tags, or task route names: the first segment. The flow and the agent page
// sit above every project, and the merged board names none.
export function projectRouteOf(path: string): string | undefined {
  const [pathname] = path.split(/[#?]/, 1)
  if (pathname === FLOW_ROUTE || isAgentRoute(pathname)) return undefined
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

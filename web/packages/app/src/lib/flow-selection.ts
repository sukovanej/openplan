import { FLOW_ROUTE, statusText } from "@open-planner/task-ui"

// Each field is a list, because the endpoint takes each name more than once: the values of one name
// are alternatives, and two names narrow each other. The whole selection lives in the URL, so a
// reader can bookmark a flow and share it.
export interface FlowSelection {
  readonly projects: ReadonlyArray<string>
  readonly statuses: ReadonlyArray<string>
  readonly tasks: ReadonlyArray<string>
  readonly tags: ReadonlyArray<string>
}

export const EVERY_TASK: FlowSelection = { projects: [], statuses: [], tasks: [], tags: [] }

export function readSelection(params: URLSearchParams): FlowSelection {
  return {
    projects: params.getAll("project"),
    statuses: params.getAll("status"),
    tasks: params.getAll("task"),
    tags: params.getAll("tag"),
  }
}

export function selectionParams(selection: FlowSelection): URLSearchParams {
  const params = new URLSearchParams()
  for (const project of selection.projects) params.append("project", project)
  for (const status of selection.statuses) params.append("status", status)
  for (const task of selection.tasks) params.append("task", task)
  for (const tag of selection.tags) params.append("tag", tag)
  return params
}

// A key needs its project, because two stores can commit the same abbreviation.
export function taskFlowPath(project: string, id: string): string {
  return `${FLOW_ROUTE}?${selectionParams({ ...EVERY_TASK, projects: [project], tasks: [id] })}`
}

export function selectsEveryTask(selection: FlowSelection): boolean {
  return [selection.projects, selection.statuses, selection.tasks, selection.tags].every((named) => named.length === 0)
}

export function describeSelection(selection: FlowSelection): string {
  const parts = [
    ...selection.tasks,
    ...selection.projects,
    ...selection.tags.map((tag) => `#${tag}`),
    ...selection.statuses.map(statusText),
  ]
  return parts.length === 0 ? "Every task that is not finished" : parts.join(" · ")
}

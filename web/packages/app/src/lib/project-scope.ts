import {
  activityPath,
  boardPath,
  docsPath,
  FLOW_ROUTE,
  isActivityPath,
  isDocsPath,
  isTagsPath,
  projectOfPath,
  tagsPath,
} from "@openplan/task-ui"

import { readSelection, selectionParams } from "./flow-selection"

export type ListView = "tasks" | "docs"

export function listPath(view: ListView, project: string | undefined): string {
  return view === "tasks" ? boardPath(project) : docsPath(project)
}

// Empty is every project. Only the flow can name several.
export function selectedProjects(pathname: string, search: string): ReadonlyArray<string> {
  if (pathname === FLOW_ROUTE) return readSelection(new URLSearchParams(search)).projects
  const project = projectOfPath(pathname)
  return project === undefined ? [] : [project]
}

export function selectedProject(pathname: string, search: string): string | undefined {
  const projects = selectedProjects(pathname, search)
  return projects.length === 1 ? projects[0] : undefined
}

// Another project has no task or doc of this one, so a page of one of them gives way to its list.
// The flow drops the tasks it names for the same reason.
export function switchProjectPath(pathname: string, search: string, project: string | undefined): string {
  if (pathname === FLOW_ROUTE) {
    const selection = readSelection(new URLSearchParams(search))
    const query = selectionParams({ ...selection, projects: project === undefined ? [] : [project], tasks: [] })
    return query.toString() === "" ? FLOW_ROUTE : `${FLOW_ROUTE}?${query}`
  }
  if (isDocsPath(pathname)) return docsPath(project)
  if (isTagsPath(pathname)) return tagsPath(project)
  if (isActivityPath(pathname)) return activityPath(project)
  return boardPath(project)
}

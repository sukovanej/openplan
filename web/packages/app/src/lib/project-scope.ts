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

import { EVERY_TASK, readSelection, selectionParams } from "./flow-selection"

// The pages every project has, and all projects together.
export type Page = "tasks" | "docs" | "activity" | "tags" | "flow"

export function pagePath(page: Page, project: string | undefined): string {
  switch (page) {
    case "tasks":
      return boardPath(project)
    case "docs":
      return docsPath(project)
    case "activity":
      return activityPath(project)
    case "tags":
      return tagsPath(project)
    case "flow":
      return project === undefined
        ? FLOW_ROUTE
        : `${FLOW_ROUTE}?${selectionParams({ ...EVERY_TASK, projects: [project] })}`
  }
}

// A task belongs to the tasks and a doc to the docs.
export function pageOf(pathname: string): Page {
  if (pathname === FLOW_ROUTE) return "flow"
  if (isDocsPath(pathname)) return "docs"
  if (isTagsPath(pathname)) return "tags"
  if (isActivityPath(pathname)) return "activity"
  return "tasks"
}

// Empty is every project. Only the flow can name several.
export function selectedProjects(pathname: string, search: string): ReadonlyArray<string> {
  if (pathname === FLOW_ROUTE) return readSelection(new URLSearchParams(search)).projects
  const project = projectOfPath(pathname)
  return project === undefined ? [] : [project]
}

export function selects(selected: ReadonlyArray<string>, project: string | undefined): boolean {
  return project === undefined ? selected.length === 0 : selected.length === 1 && selected[0] === project
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
  return pagePath(pageOf(pathname), project)
}

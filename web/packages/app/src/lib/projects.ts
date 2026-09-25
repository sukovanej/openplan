import { useQuery } from "@tanstack/react-query"

import type { ProjectView } from "@openplan/api-client"

import { listProjects } from "./api"
import { projectsKey } from "./query-client"
import { abortable } from "./runtime"

export function useProjects(): ReadonlyArray<ProjectView> | undefined {
  return useQuery({
    queryKey: projectsKey,
    queryFn: abortable(listProjects),
  }).data
}

export function useProject(name: string): ProjectView | undefined {
  return useProjects()?.find((project) => project.name === name)
}

export function useAbbreviation(project: string): string | undefined {
  return useProject(project)?.abbreviation
}

export function demotedReason(project: ProjectView | undefined): string | undefined {
  return project?.status.state === "error" ? project.status.reason : undefined
}

import { useQuery } from "@tanstack/react-query"
import { useMemo } from "react"

import type { ProjectView } from "@openplan/api-client"

import { listProjects } from "./api"
import { demotedReason, useFaults } from "./faults"
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

// A demoted project cannot take a write.
export function useWritableProjects(): ReadonlyArray<ProjectView> | undefined {
  const projects = useProjects()
  const faults = useFaults()
  return useMemo(
    () => projects?.filter((project) => demotedReason(faults, project.name) === undefined),
    [projects, faults],
  )
}

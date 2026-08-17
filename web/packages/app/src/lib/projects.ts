import { useSyncExternalStore } from "react"

import type { ProjectView } from "@open-planner/api-client"

// Every project the daemon serves, with the abbreviation each one spells its keys with. Held apart
// from the task queries because it is not task data: it decides how every id already on screen
// reads, and which projects the switcher offers. `undefined` is "not read yet", which an empty list
// — a daemon with nothing registered — must not be mistaken for.
class ProjectsStore {
  private value: ReadonlyArray<ProjectView> | undefined
  private readonly listeners = new Set<() => void>()

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  readonly getSnapshot = (): ReadonlyArray<ProjectView> | undefined => this.value

  readonly set = (next: ReadonlyArray<ProjectView>): void => {
    this.value = next
    for (const listener of this.listeners) listener()
  }
}

export const projectsStore = new ProjectsStore()

export function useProjects(): ReadonlyArray<ProjectView> | undefined {
  return useSyncExternalStore(projectsStore.subscribe, projectsStore.getSnapshot)
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

export const TASK_ROUTE = "/task/"

export function taskPath(id: string, section?: string): string {
  return section === undefined ? `${TASK_ROUTE}${id}` : `${TASK_ROUTE}${id}#${encodeURIComponent(section)}`
}

export function taskIdOf(path: string): string | undefined {
  if (!path.startsWith(TASK_ROUTE)) return undefined
  const id = path.slice(TASK_ROUTE.length).split(/[/#]/, 1)[0]
  return id === "" ? undefined : id
}

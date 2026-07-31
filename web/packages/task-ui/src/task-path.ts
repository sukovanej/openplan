export const TASK_ROUTE = "/task/"

export function taskPath(id: string, section?: string): string {
  return section === undefined || section === ""
    ? `${TASK_ROUTE}${id}`
    : `${TASK_ROUTE}${id}#${encodeURIComponent(section)}`
}

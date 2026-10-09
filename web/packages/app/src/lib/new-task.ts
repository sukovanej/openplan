// The project on screen comes first, because that is the board the new task shows up on. Without one,
// the project of the last task created keeps a run of new tasks in one place.
export function defaultProject(
  writable: ReadonlyArray<string>,
  onScreen: string | undefined,
  last: string | undefined,
): string | undefined {
  if (onScreen !== undefined && writable.includes(onScreen)) return onScreen
  if (last !== undefined && writable.includes(last)) return last
  return writable[0]
}

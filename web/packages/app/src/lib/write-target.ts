import type { TaskDetail, TaskListItem } from "@openplan/api-client"

// Where an edit of the shown version lands, and what to say when it can land nowhere. The daemon
// resolves the branch and reports whether a live worktree can take the write, so a view names that
// branch rather than guessing at one, and offers only the actions that can succeed.
export interface WriteHere {
  readonly branch: string | undefined
  readonly blocked: string | undefined
}

export function writeHere(task: TaskDetail | TaskListItem): WriteHere {
  const target = task.write_target
  if (target === undefined) {
    return { branch: undefined, blocked: "This repository has no branch to write to." }
  }
  return {
    branch: target.branch,
    blocked: target.writable ? undefined : `No writable worktree holds ${target.branch}, so this task cannot change.`,
  }
}

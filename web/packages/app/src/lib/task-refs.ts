import { useQuery } from "@tanstack/react-query"

import type { Board, FieldError, TaskRef } from "@openplan/api-client"
import { statusField } from "@openplan/task-ui"

import { getBoard } from "./api"
import { boardKey } from "./query-client"
import { abortable } from "./runtime"

const UNREAD: FieldError = { kind: "missing" }

// The revisions name tasks by key alone, and the board holds each one's title and status. Two
// projects can hold the same key, so each project has a map of its own.
export const refsByProject = (board: Board): ReadonlyMap<string, ReadonlyMap<string, TaskRef>> => {
  const refs = new Map<string, Map<string, TaskRef>>()
  for (const { task } of board.groups.flatMap((group) => group.rows)) {
    const held = refs.get(task.project) ?? new Map<string, TaskRef>()
    held.set(task.id, { id: task.id, title: task.title, status: statusField(task.metadata) ?? UNREAD })
    refs.set(task.project, held)
  }
  return refs
}

// `undefined` until the board is read.
export function useTaskRefs(project: string): ReadonlyMap<string, TaskRef> | undefined {
  return useQuery({
    queryKey: boardKey(project),
    queryFn: abortable(getBoard(project)),
    select: (board: Board) => refsByProject(board).get(project) ?? new Map<string, TaskRef>(),
  }).data
}

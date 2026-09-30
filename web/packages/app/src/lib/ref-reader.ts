import { useQuery } from "@tanstack/react-query"

import { type ReadRefs, statusField } from "@openplan/task-ui"

import { getDoc, getTask, TaskNotFound } from "./api"
import { docKey, taskKey } from "./query-client"
import { abortable } from "./runtime"

// A chip reads its task or doc with the query of the page that shows it, so the chips and the page
// share one read, and a change to the task or the doc reaches every chip.
export const refReader: ReadRefs = {
  useTask: (project, id) => {
    const task = useQuery({ queryKey: taskKey(project, id), queryFn: abortable(getTask(project, id)) })
    if (task.error instanceof TaskNotFound) return "gone"
    return task.data === undefined ? "reading" : { status: statusField(task.data.metadata) }
  },
  useDoc: (project, name) => {
    const doc = useQuery({ queryKey: docKey(project, name), queryFn: abortable(getDoc(project, name)) })
    if (doc.error instanceof TaskNotFound) return "gone"
    return doc.data === undefined ? "reading" : "found"
  },
}

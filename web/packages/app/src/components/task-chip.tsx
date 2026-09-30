import { useQuery } from "@tanstack/react-query"

import type { FieldError } from "@openplan/api-client"
import { statusField, taskPath, TaskRefChip } from "@openplan/task-ui"

import { getTask, TaskNotFound } from "../lib/api"
import { taskKey } from "../lib/query-client"
import { abortable } from "../lib/runtime"

const UNREAD: FieldError = { kind: "missing" }

// The chip reads its task as the task page does, so the two share one read, and a change to the
// task reaches every chip that shows it.
export function TaskChip({ project, id, sign }: { project: string; id: string; sign?: "+" | "−" }) {
  const task = useQuery({ queryKey: taskKey(project, id), queryFn: abortable(getTask(project, id)) })
  const gone = task.error instanceof TaskNotFound
  return (
    <TaskRefChip
      to={taskPath(project, id)}
      id={id}
      task={
        gone || task.data === undefined
          ? undefined
          : { id, title: task.data.title, status: statusField(task.data.metadata) ?? UNREAD }
      }
      loading={task.isPending}
      sign={sign}
    />
  )
}

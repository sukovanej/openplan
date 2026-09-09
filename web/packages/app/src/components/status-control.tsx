import { useRef, useState } from "react"

import type { Field_Status, Status } from "@openplan/api-client"
import { fieldValue, StatusMark, StatusMenu, statusLabel } from "@openplan/task-ui"
import { Tooltip, useDismissOnOutsideClick } from "@openplan/ui"

import { patchTask } from "../lib/api"
import { useProjectMutation } from "../lib/query-client"
import { useStatusRequest } from "../lib/status-requests"

const UNREADABLE = "Status could not be read"

const named = (status: Status | undefined): string => (status === undefined ? UNREADABLE : statusLabel(status))

// The status mark, and the menu that writes another status in its place. It sits inside rows that
// open their task on a click, so every click of its own stops there rather than travelling on.
export function StatusControl({
  project,
  id,
  at,
  status,
  branch,
  blocked,
  className,
}: {
  project: string
  id: string
  at: number
  status: Field_Status | undefined
  branch?: string
  // Why this task cannot change, when it cannot: the mark stands on its own and says so.
  blocked?: string
  className?: string
}) {
  const [open, setOpen] = useState(false)
  const root = useRef<HTMLDivElement>(null)
  const { mutate } = useProjectMutation(project)
  const current = status === undefined ? undefined : fieldValue(status)

  useDismissOnOutsideClick(root, open ? () => setOpen(false) : undefined)
  useStatusRequest({ project, id, at }, () => {
    if (blocked === undefined) setOpen(true)
  })

  if (blocked !== undefined) {
    return (
      <Tooltip content={`${named(current)} — ${blocked}`}>
        <StatusMark status={status} className={className} />
      </Tooltip>
    )
  }

  const pick = (next: Status) => {
    setOpen(false)
    if (next !== current) mutate(patchTask(project, id, { status: next }, branch))
  }

  return (
    <div
      ref={root}
      className="relative"
      onClick={(event) => {
        event.preventDefault()
        event.stopPropagation()
      }}
    >
      <Tooltip content={`${named(current)} — press s to change`}>
        <button
          type="button"
          aria-label="Change status"
          aria-haspopup="listbox"
          aria-expanded={open}
          onClick={() => setOpen(!open)}
          // The padded hit area is pulled back into the mark's own box, so the mark stays exactly
          // where it sits without one — the list's tree guides are drawn to its middle.
          className="hover:bg-muted focus-visible:ring-ring -m-0.5 flex rounded-md p-0.5 transition-colors focus-visible:ring-2 focus-visible:outline-none"
        >
          <StatusMark status={status} className={className} />
        </button>
      </Tooltip>
      {open && (
        <StatusMenu
          current={current}
          onPick={pick}
          onClose={() => setOpen(false)}
          className="absolute top-full left-0 z-30 mt-1"
        />
      )}
    </div>
  )
}

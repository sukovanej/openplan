import { TriangleAlert } from "lucide-react"
import { useRef, useState } from "react"
import { Link } from "react-router-dom"

import { boardPath } from "@openplan/task-ui"
import { Button, CountPill, Tooltip, useDismissOnOutsideClick } from "@openplan/ui"

import { useFaults } from "../lib/faults"

export const FAULTS_LABEL = "Faults"

// Every fault the daemon cannot fix by itself, in one place. The list stays empty until one starts,
// and the control is absent while it is.
export function FaultStatus() {
  const faults = useFaults()
  const [open, setOpen] = useState(false)
  const root = useRef<HTMLDivElement>(null)
  useDismissOnOutsideClick(root, open ? () => setOpen(false) : undefined)

  if (faults.length === 0) return null
  return (
    <div
      ref={root}
      className="relative"
      onKeyDown={(event) => {
        if (event.key === "Escape") setOpen(false)
      }}
    >
      <Tooltip
        content={faults.length === 1 ? "A fault needs your action." : `${faults.length} faults need your action.`}
      >
        <Button
          aria-label={FAULTS_LABEL}
          aria-expanded={open}
          onClick={() => setOpen(!open)}
          className="text-warning gap-1.5 px-1.5 py-1.5"
        >
          <TriangleAlert className="size-4" aria-hidden />
          <CountPill count={faults.length} className="text-warning bg-muted" />
        </Button>
      </Tooltip>
      {open && (
        <ul className="bg-popover absolute top-full right-0 z-30 mt-1.5 flex w-[28rem] max-w-[calc(100vw-1rem)] max-md:fixed max-md:inset-x-2 max-md:top-14 max-md:w-auto flex-col gap-2 rounded-md border p-2 shadow-md">
          {faults.map((fault) => (
            <li
              key={`${fault.project} ${fault.kind}`}
              aria-label={fault.project}
              className="flex flex-col gap-0.5 pl-1 text-xs"
            >
              <Link
                to={boardPath(fault.project)}
                onClick={() => setOpen(false)}
                className="hover:text-foreground font-medium"
              >
                {fault.project}
              </Link>
              <span className="text-muted-foreground break-words">{fault.message}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

import { Check, TriangleAlert } from "lucide-react"
import { useRef, useState } from "react"
import { useLocation, useNavigate } from "react-router-dom"

import type { ProjectView } from "@openplan/api-client"
import { cn, Kbd, Menu, type MenuItem, MenuTrigger, Tooltip, useDismissOnOutsideClick } from "@openplan/ui"

import { demotedReason, useFaults } from "../lib/faults"
import { useProjectMenuRequest } from "../lib/project-menu"
import { selectedProjects, selects, switchProjectPath } from "../lib/project-scope"
import { useProjects } from "../lib/projects"

const EVERY_PROJECT = "All projects"
const SHORTCUT = "o"

// A demoted project keeps its entry: it is registered, it is listed, and hiding it would leave the
// user looking for a project the daemon is still holding. The mark says it cannot answer, and the
// tooltip says why.
export function ProjectSelect({ className }: { className?: string }) {
  const projects = useProjects()
  const faults = useFaults()
  const { pathname, search } = useLocation()
  const navigate = useNavigate()
  const [open, setOpen] = useState(false)
  const root = useRef<HTMLDivElement>(null)

  useDismissOnOutsideClick(root, open ? () => setOpen(false) : undefined)
  useProjectMenuRequest(() => setOpen(true))

  if (projects === undefined || projects.length === 0) return null

  const selected = selectedProjects(pathname, search)
  const choices: ReadonlyArray<ProjectView | undefined> = [undefined, ...projects]
  const isSelected = (choice: ProjectView | undefined) => selects(selected, choice?.name)
  const reason = selected.length === 1 ? demotedReason(faults, selected[0]) : undefined

  const pick = (index: number) => {
    setOpen(false)
    const choice = choices[index]
    if (!isSelected(choice)) navigate(switchProjectPath(pathname, search, choice?.name))
  }

  const items: ReadonlyArray<MenuItem> = choices.map((choice, index) => {
    const digit = index < DIGITS.length ? DIGITS[index] : undefined
    return {
      key: choice?.name ?? "",
      shortcut: digit,
      content: <Choice project={choice} selected={isSelected(choice)} digit={digit} />,
    }
  })
  const indexOfShortcut = (key: string) => {
    const index = DIGITS.indexOf(key)
    return index === -1 || index >= choices.length ? undefined : index
  }

  return (
    <div ref={root} className={cn("relative flex min-w-10", className)}>
      <Tooltip
        className="min-w-0"
        content={
          <span className="inline-flex items-center gap-2">
            Change the project <Kbd token={SHORTCUT} className="h-5 px-1" />
          </span>
        }
      >
        <MenuTrigger open={open} onClick={() => setOpen(!open)}>
          {reason !== undefined && <TriangleAlert className="text-warning size-3.5 shrink-0" aria-hidden />}
          <span className="truncate">{label(selected)}</span>
        </MenuTrigger>
      </Tooltip>
      {open && (
        <Menu
          label="Projects"
          items={items}
          initial={choices.findIndex(isSelected)}
          onPick={pick}
          onClose={() => setOpen(false)}
          indexOfShortcut={indexOfShortcut}
          className="absolute top-full left-0 z-30 mt-1 max-h-80 w-56 overflow-y-auto"
        />
      )}
    </div>
  )
}

// "All projects" is 0, and the first nine projects take 1 to 9.
const DIGITS = "0123456789"

function label(selected: ReadonlyArray<string>): string {
  if (selected.length === 0) return EVERY_PROJECT
  return selected.length === 1 ? selected[0] : `${selected.length} projects`
}

function Choice({
  project,
  selected,
  digit,
}: {
  project: ProjectView | undefined
  selected: boolean
  digit: string | undefined
}) {
  const faults = useFaults()
  const reason = project === undefined ? undefined : demotedReason(faults, project.name)
  const name = (
    <span className="flex min-w-0 grow items-center gap-1.5">
      {reason !== undefined && <TriangleAlert className="text-warning size-3.5 shrink-0" aria-hidden />}
      <span className="truncate">{project?.name ?? EVERY_PROJECT}</span>
    </span>
  )
  return (
    <>
      {reason === undefined ? (
        name
      ) : (
        <Tooltip content={reason} className="min-w-0 grow">
          {name}
        </Tooltip>
      )}
      {selected && <Check className="text-muted-foreground size-3.5 shrink-0" aria-label="Current" />}
      {digit !== undefined && <Kbd token={digit} className="h-5 min-w-5 px-1" />}
    </>
  )
}

import { Check, ChevronsUpDown, TriangleAlert } from "lucide-react"
import { useRef, useState } from "react"
import { useLocation, useNavigate } from "react-router-dom"

import type { ProjectView } from "@openplan/api-client"
import { Kbd, Menu, type MenuItem, Tooltip, useDismissOnOutsideClick } from "@openplan/ui"

import { useProjectMenuRequest } from "../lib/project-menu"
import { selectedProjects, switchProjectPath } from "../lib/project-scope"
import { demotedReason, useProjects } from "../lib/projects"

const EVERY_PROJECT = "All projects"
const SHORTCUT = "mod+p"

// A demoted project keeps its entry: it is registered, it is listed, and hiding it would leave the
// user looking for a project the daemon is still holding. The mark says it cannot answer, and the
// tooltip says why.
export function ProjectSelect() {
  const projects = useProjects()
  const { pathname, search } = useLocation()
  const navigate = useNavigate()
  const [open, setOpen] = useState(false)
  const root = useRef<HTMLDivElement>(null)

  useDismissOnOutsideClick(root, open ? () => setOpen(false) : undefined)
  useProjectMenuRequest(() => setOpen(true))

  if (projects === undefined || projects.length === 0) return null

  const selected = selectedProjects(pathname, search)
  const choices: ReadonlyArray<ProjectView | undefined> = [undefined, ...projects]
  const isSelected = (choice: ProjectView | undefined) =>
    choice === undefined ? selected.length === 0 : selected.length === 1 && selected[0] === choice.name
  const reason =
    selected.length === 1 ? demotedReason(projects.find((project) => project.name === selected[0])) : undefined

  const pick = (index: number) => {
    setOpen(false)
    const choice = choices[index]
    if (!isSelected(choice)) navigate(switchProjectPath(pathname, search, choice?.name))
  }

  const items: ReadonlyArray<MenuItem> = choices.map((choice) => ({
    key: choice?.name ?? "",
    content: <Choice project={choice} selected={isSelected(choice)} />,
  }))

  return (
    <div ref={root} className="relative flex min-w-10">
      <Tooltip
        className="min-w-0"
        content={
          <span className="inline-flex items-center gap-2">
            Change the project <Kbd token={SHORTCUT} className="h-5 px-1" />
          </span>
        }
      >
        <button
          type="button"
          aria-haspopup="listbox"
          aria-expanded={open}
          onClick={() => setOpen(!open)}
          className="hover:bg-muted focus-visible:ring-ring inline-flex min-w-0 items-center gap-1.5 rounded-md border px-2 py-1 text-sm transition-colors focus-visible:ring-2 focus-visible:outline-none"
        >
          {reason !== undefined && <TriangleAlert className="text-warning size-3.5 shrink-0" aria-hidden />}
          <span className="truncate">{label(selected)}</span>
          <ChevronsUpDown className="text-muted-foreground size-3.5 shrink-0" aria-hidden />
        </button>
      </Tooltip>
      {open && (
        <Menu
          label="Projects"
          items={items}
          initial={choices.findIndex(isSelected)}
          onPick={pick}
          onClose={() => setOpen(false)}
          className="absolute top-full left-0 z-30 mt-1 max-h-80 w-56 overflow-y-auto"
        />
      )}
    </div>
  )
}

function label(selected: ReadonlyArray<string>): string {
  if (selected.length === 0) return EVERY_PROJECT
  return selected.length === 1 ? selected[0] : `${selected.length} projects`
}

function Choice({ project, selected }: { project: ProjectView | undefined; selected: boolean }) {
  const reason = demotedReason(project)
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
    </>
  )
}

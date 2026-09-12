import { Pencil } from "lucide-react"
import { useCallback, useState } from "react"
import { useNavigate, useSearchParams } from "react-router-dom"

import { taskSessionPath } from "@openplan/task-ui"
import {
  Button,
  Combobox,
  type ComboOption,
  EmptyState,
  fuzzyMatch,
  FuzzyText,
  Panel,
  PanelBody,
  PanelHeader,
  PanelTitle,
  Skeleton,
} from "@openplan/ui"

import { AgentDock } from "../components/agent-dock"
import type { SessionView } from "../lib/agent-events"
import { useProjects } from "../lib/projects"

const SESSION_PARAM = "session"
const PROJECT_PARAM = "project"

// The page that drafts a task: the task box before there is a task, with the chat at its foot as
// on a task's page. The project is picked in the header, since the page belongs to none; once the
// agent writes the task, the page hands over to the task's own, with the session on it.
export function AgentRoute() {
  const navigate = useNavigate()
  const [params, setParams] = useSearchParams()
  const projects = useProjects()
  const chosen = params.get(PROJECT_PARAM) ?? undefined
  const session = params.get(SESSION_PARAM) ?? undefined
  const project = projects?.find((known) => known.name === chosen)?.name ?? projects?.[0]?.name

  const setSession = useCallback(
    (next: string | undefined) =>
      setParams(
        (current) => {
          const out = new URLSearchParams(current)
          if (next === undefined) out.delete(SESSION_PARAM)
          else out.set(SESSION_PARAM, next)
          return out
        },
        { replace: true },
      ),
    [setParams],
  )
  const setProject = (next: string) => setParams({ [PROJECT_PARAM]: next }, { replace: true })

  const [writing, setWriting] = useState(false)
  const onView = useCallback(
    (view: SessionView) => {
      setWriting(view.status.kind === "running")
      if (view.task !== null && project !== undefined && session !== undefined) {
        navigate(taskSessionPath(project, view.task, session), { replace: true })
      }
    },
    [project, session, navigate],
  )

  if (projects === undefined) return <Skeleton className="h-40 w-full" />
  if (project === undefined) {
    return <EmptyState title="No projects yet" detail="Register a repository with `openplan project add`." />
  }
  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto lg:flex-row lg:overflow-hidden">
      <Panel className="h-auto min-w-0 lg:h-full lg:w-[59rem]">
        <PanelHeader className="gap-3">
          <PanelTitle>New task</PanelTitle>
          <ProjectPicker
            projects={projects.map((known) => known.name)}
            value={project}
            fixed={session !== undefined}
            onChange={setProject}
          />
        </PanelHeader>
        <PanelBody className="p-6">
          <NoTaskYet writing={writing} />
        </PanelBody>
        <AgentDock
          key={project}
          project={project}
          task={undefined}
          session={session}
          onSession={setSession}
          onView={onView}
        />
      </Panel>
    </div>
  )
}

// The project the draft goes to, shown as a name and changed through the same search box the
// parent picker uses. A session runs in one project, so once it starts the name stays.
function ProjectPicker({
  projects,
  value,
  fixed,
  onChange,
}: {
  projects: ReadonlyArray<string>
  value: string
  fixed: boolean
  onChange: (next: string) => void
}) {
  const [editing, setEditing] = useState(false)
  const buildOptions = useCallback(
    (query: string): ReadonlyArray<ComboOption> =>
      projects
        .flatMap((name) => {
          const match = fuzzyMatch(query, name)
          return match === null ? [] : [{ name, match }]
        })
        .sort((a, b) => a.match.score - b.match.score)
        .map(({ name, match }) => ({
          key: name,
          content: <FuzzyText text={name} indices={match.indices} />,
          onSelect: () => onChange(name),
        })),
    [projects, onChange],
  )
  if (editing) {
    return (
      <Combobox
        placeholder="Project…"
        buildOptions={buildOptions}
        onClose={() => setEditing(false)}
        emptyLabel="No matching project"
        className="w-56"
      />
    )
  }
  return (
    <span className="flex min-w-0 items-center gap-1 text-xs normal-case">
      <span className="text-muted-foreground truncate">{value}</span>
      {!fixed && (
        <Button onClick={() => setEditing(true)} aria-label="Change project" className="px-1.5">
          <Pencil className="size-3.5" />
        </Button>
      )}
    </span>
  )
}

function NoTaskYet({ writing }: { writing: boolean }) {
  return (
    <div className="space-y-4">
      <EmptyState
        title="The agent has not written a task yet"
        detail={writing ? "The preview mounts as soon as it does." : "Describe what you want in the chat."}
      />
      {writing && (
        <div className="space-y-4">
          <Skeleton className="h-8 w-2/3" />
          <Skeleton className="h-5 w-24" />
          <Skeleton className="h-40 w-full" />
        </div>
      )}
    </div>
  )
}

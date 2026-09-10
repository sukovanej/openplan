import { useCallback, useState } from "react"
import { useLocation, useParams, useSearchParams } from "react-router-dom"

import { AgentPanel } from "../components/agent-panel"
import type { SessionView } from "../lib/agent-events"
import { detailActions, useDetailAction } from "../lib/detail-actions"
import { TaskView } from "./task-view"

const BRANCH_PARAM = "branch"
const SESSION_PARAM = "session"

export function DetailRoute() {
  const { project = "", id = "" } = useParams()
  return <DetailPage key={`${project}:${id}`} project={project} id={id} />
}

// The way `e` on a board row asks the task page to open with the chat already up.
export interface DetailState {
  readonly agent?: boolean
}

function DetailPage({ project, id }: { project: string; id: string }) {
  // The selected branch lives in the URL (`?branch=`), so it is shareable and resets to the
  // headline on navigation without a render lag; absent means the headline (current-worktree)
  // version, or the branch the agent writes on while a session is open. `?session=` names that
  // session, so a reload comes back to the same chat.
  const [params, setParams] = useSearchParams()
  const pinned = params.get(BRANCH_PARAM)
  const session = params.get(SESSION_PARAM) ?? undefined
  const state = useLocation().state as DetailState | null
  const [opened, setOpened] = useState(state?.agent === true)
  const [agentBranch, setAgentBranch] = useState<string>()

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
  const onView = useCallback((view: SessionView) => setAgentBranch(view.branch ?? undefined), [])
  useDetailAction("open-agent", () => {
    setOpened(true)
    detailActions.emit("focus-prompt")
  })

  const open = opened || session !== undefined
  // While the agent writes on a branch the headline is a choice, so choosing it writes `branch=`
  // (empty) rather than dropping the parameter.
  const branch = pinned === null ? agentBranch : pinned === "" ? undefined : pinned
  const onSelect = (next: string | undefined) =>
    setParams(
      (current) => {
        const out = new URLSearchParams(current)
        if (next !== undefined) out.set(BRANCH_PARAM, next)
        else if (agentBranch !== undefined) out.set(BRANCH_PARAM, "")
        else out.delete(BRANCH_PARAM)
        return out
      },
      { replace: true },
    )

  return (
    <TaskView
      project={project}
      id={id}
      branch={branch}
      onSelect={onSelect}
      agent={
        open ? (
          <AgentPanel
            project={project}
            task={id}
            session={session}
            onSession={setSession}
            onView={onView}
            className="h-[70vh] shrink-0 lg:h-auto lg:min-h-80 lg:flex-1"
          />
        ) : undefined
      }
    />
  )
}

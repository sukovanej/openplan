import { GitPullRequest } from "lucide-react"

import type { PullRequestView } from "@openplan/api-client"
import { cn, Tooltip } from "@openplan/ui"

import { newestPullRequestState, pullRequestTint } from "./pull-request-state"

const count = (n: number): string => (n === 1 ? "1 pull request" : `${n} pull requests`)

export function PullRequestsMark({ pullRequests = [] }: { pullRequests?: ReadonlyArray<PullRequestView> }) {
  if (pullRequests.length === 0) return null
  const newest = newestPullRequestState(pullRequests)
  return (
    <Tooltip
      content={
        <ul>
          {pullRequests.map((each) => (
            <li key={each.url}>{each.status === undefined ? each.short : `${each.short} · ${each.status.state}`}</li>
          ))}
        </ul>
      }
      className="shrink-0"
    >
      <GitPullRequest
        role="img"
        aria-label={
          newest === undefined ? count(pullRequests.length) : `${count(pullRequests.length)}, newest ${newest}`
        }
        className={cn("size-3.5", newest === undefined ? "opacity-70" : pullRequestTint(newest))}
      />
    </Tooltip>
  )
}

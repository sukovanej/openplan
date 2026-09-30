import { GitPullRequest } from "lucide-react"

import type { PullRequestView } from "@openplan/api-client"
import { Tooltip } from "@openplan/ui"

const count = (n: number): string => (n === 1 ? "1 pull request" : `${n} pull requests`)

export function PullRequestsMark({ pullRequests = [] }: { pullRequests?: ReadonlyArray<PullRequestView> }) {
  if (pullRequests.length === 0) return null
  return (
    <Tooltip
      content={
        <ul>
          {pullRequests.map((each) => (
            <li key={each.url}>{each.short}</li>
          ))}
        </ul>
      }
      className="shrink-0"
    >
      <GitPullRequest role="img" aria-label={count(pullRequests.length)} className="size-3.5 opacity-70" />
    </Tooltip>
  )
}

import type { PullRequestState, PullRequestView } from "@openplan/api-client"
import { cn, Tag } from "@openplan/ui"

// The hues that GitHub and GitLab use, so a state reads the same here as on the forge.
const tints: Record<PullRequestState, string> = {
  open: "text-success border-success/40",
  draft: "text-muted-foreground border-border",
  merged: "text-tag-violet border-tag-violet/40",
  closed: "text-danger border-danger/40",
}

export const pullRequestTint = (state: PullRequestState): string => tints[state]

// The pull request linked last is the newest, because a link appends to the list.
export const newestPullRequestState = (pullRequests: ReadonlyArray<PullRequestView>): PullRequestState | undefined =>
  pullRequests.findLast((each) => each.status !== undefined)?.status?.state

export const allMerged = (pullRequests: ReadonlyArray<PullRequestView>): boolean =>
  pullRequests.length > 0 && pullRequests.every((each) => each.status?.state === "merged")

export function PullRequestStateChip({ state, className }: { state: PullRequestState; className?: string }) {
  return <Tag className={cn("px-1.5 py-0.5", pullRequestTint(state), className)}>{state}</Tag>
}

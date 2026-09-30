import { Plus, X } from "lucide-react"
import { useState } from "react"

import type { Metadata, PullRequestView } from "@openplan/api-client"
import { ForgeLink } from "@openplan/task-ui"
import { Button, Section, TextInput } from "@openplan/ui"

import { patchTask } from "../lib/api"
import { errorText } from "../lib/format"
import { useProjectMutation } from "../lib/query-client"
import { FieldConflictControl } from "./field-conflict"

export function PullRequestsSection({
  project,
  id,
  metadata,
  pullRequests = [],
}: {
  project: string
  id: string
  metadata: Metadata
  pullRequests?: ReadonlyArray<PullRequestView>
}) {
  const [adding, setAdding] = useState(false)
  const unlink = useProjectMutation(project)
  return (
    <Section
      title="Pull requests"
      count={pullRequests.length}
      action={
        <div className="flex items-center gap-1.5">
          <FieldConflictControl project={project} id={id} metadata={metadata} field="pull_requests" align="end" />
          <Button variant="accent" onClick={() => setAdding((open) => !open)} aria-label="Add pull request">
            <Plus className="size-3.5" />
          </Button>
        </div>
      }
    >
      {adding && <AddPullRequest project={project} id={id} onClose={() => setAdding(false)} />}
      {pullRequests.length === 0 ? (
        <p className="text-muted-foreground text-sm">No pull requests yet.</p>
      ) : (
        <ul className="space-y-0.5">
          {pullRequests.map((each) => (
            <li
              key={each.url}
              className="group/row hover:bg-muted/50 relative flex items-center gap-2 rounded-md px-2 py-1.5 text-sm"
            >
              {/* The link covers the row, so a click anywhere on the row opens the pull request. */}
              <ForgeLink
                url={each.url}
                forge={each.forge}
                short={each.short}
                className="after:absolute after:inset-0"
              />
              <button
                type="button"
                aria-label={`Unlink ${each.short}`}
                disabled={unlink.isPending}
                onClick={() => unlink.mutate(patchTask(project, id, { remove_pull_requests: [each.url] }))}
                className="text-muted-foreground hover:text-foreground relative ml-auto shrink-0 cursor-pointer opacity-0 transition-opacity group-hover/row:opacity-100 focus-visible:opacity-100 max-md:opacity-100"
              >
                <X className="size-3.5" />
              </button>
            </li>
          ))}
        </ul>
      )}
    </Section>
  )
}

function AddPullRequest({ project, id, onClose }: { project: string; id: string; onClose: () => void }) {
  const [entry, setEntry] = useState("")
  const link = useProjectMutation(project, "inline")
  const submit = () => {
    const pullRequest = entry.trim()
    if (pullRequest === "" || link.isPending) return
    link.mutate(patchTask(project, id, { add_pull_requests: [pullRequest] }), { onSuccess: onClose })
  }
  return (
    <div className="mb-3">
      <TextInput
        autoFocus
        value={entry}
        placeholder="Address or number…"
        aria-label="Pull request address or number"
        aria-invalid={link.isError}
        onChange={(event) => {
          setEntry(event.target.value)
          link.reset()
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter") submit()
          if (event.key === "Escape") onClose()
        }}
        className="w-full"
      />
      {link.isError && (
        <p role="alert" className="text-danger mt-1 text-xs">
          {errorText(link.error)}
        </p>
      )}
    </div>
  )
}

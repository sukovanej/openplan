import { Link } from "react-router-dom"

import { ChangeMark, RevisionMeta, revisionPath, revisionSummary, shortRevision } from "@openplan/task-ui"
import { Row, Section, SkeletonList } from "@openplan/ui"

import { errorText } from "../lib/format"
import { revisionChanges, useTaskHistory } from "../lib/history"
import { OlderRevisions } from "./older-revisions"

export function TaskHistory({ project, id, selected }: { project: string; id: string; selected: string | undefined }) {
  const history = useTaskHistory(project, id)
  return (
    <Section title="History">
      {history.isPending ? (
        <SkeletonList count={3} className="h-11 w-full" />
      ) : history.isError ? (
        <p className="text-muted-foreground text-sm">The history could not be read: {errorText(history.error)}</p>
      ) : history.data.length === 0 ? (
        <p className="text-muted-foreground text-sm">No revision holds this task.</p>
      ) : (
        <>
          <ol className="space-y-0.5">
            {history.data.map((entry) => {
              const revision = entry.revision
              const kind = revisionChanges(entry).tasks.find((change) => change.id === id)?.kind
              const current = revision.id === selected
              return (
                <li key={revision.id}>
                  <Row
                    as={Link}
                    variant="option"
                    active={current}
                    hoverable
                    aria-current={current ? "page" : undefined}
                    to={revisionPath(project, id, revision.id)}
                    className="flex-col items-stretch gap-1"
                  >
                    <span className="flex min-w-0 items-baseline gap-2">
                      <span className="min-w-0 flex-1 truncate">
                        {revisionSummary(revision.message) || shortRevision(revision.id)}
                      </span>
                      {kind !== undefined && kind !== "modified" && <ChangeMark kind={kind} />}
                    </span>
                    <RevisionMeta revision={revision} />
                  </Row>
                </li>
              )
            })}
          </ol>
          <OlderRevisions history={history} className="mt-2" />
        </>
      )}
    </Section>
  )
}

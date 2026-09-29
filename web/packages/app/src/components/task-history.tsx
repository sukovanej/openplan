import { Section, SkeletonList } from "@openplan/ui"

import { errorText } from "../lib/format"
import { useTaskHistory } from "../lib/history"
import { OlderRevisions } from "./older-revisions"
import { RevisionList } from "./revision-list"

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
          <RevisionList project={project} entries={history.data} task={id} selected={selected} />
          <OlderRevisions history={history} className="mt-2" />
        </>
      )}
    </Section>
  )
}

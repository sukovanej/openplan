import { Section, SkeletonList } from "@openplan/ui"

import { errorText } from "../lib/format"
import { useDocHistory } from "../lib/history"
import { OlderRevisions } from "./older-revisions"
import { RevisionList } from "./revision-list"

// The history of a doc runs back to its last rename: before it, the doc had another name and file.
export function DocHistory({
  project,
  name,
  selected,
}: {
  project: string
  name: string
  selected: string | undefined
}) {
  const history = useDocHistory(project, name)
  return (
    <Section title="History">
      {history.isPending ? (
        <SkeletonList count={3} className="h-11 w-full" />
      ) : history.isError ? (
        <p className="text-muted-foreground text-sm">The history could not be read: {errorText(history.error)}</p>
      ) : history.data.length === 0 ? (
        <p className="text-muted-foreground text-sm">No revision holds this doc.</p>
      ) : (
        <>
          <RevisionList project={project} entries={history.data} doc={name} selected={selected} />
          <OlderRevisions history={history} className="mt-2" />
        </>
      )}
    </Section>
  )
}

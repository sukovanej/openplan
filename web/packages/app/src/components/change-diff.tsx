import { lazy, Suspense } from "react"

import { Skeleton } from "@openplan/ui"

import type { DiffTarget } from "../lib/api"
import { errorText } from "../lib/format"
import { useChangeDiff } from "../lib/history"

// Only an open card needs the diff code, so the activity route loads without it.
const DiffView = lazy(() => import("@openplan/ui/diff-view").then((module) => ({ default: module.DiffView })))

const note = "text-muted-foreground px-3 py-2 text-xs"

export function ChangeDiff({ project, revision, target }: { project: string; revision: string; target: DiffTarget }) {
  const diff = useChangeDiff(project, revision, target)
  return (
    <>
      <p className="bg-popover text-muted-foreground sticky top-0 truncate border-b px-3 py-1.5 font-mono text-xs">
        {target.from === undefined ? target.path : `${target.from} → ${target.path}`}
      </p>
      {diff.isPending ? (
        <DiffSkeleton />
      ) : diff.isError ? (
        <p className={note}>Could not load the diff: {errorText(diff.error)}</p>
      ) : diff.data.kind === "binary" ? (
        <p className={note}>The file is binary. It has no lines to compare.</p>
      ) : (
        <Suspense fallback={<DiffSkeleton />}>
          <DiffView diff={diff.data.diff} className="py-1" />
          {diff.data.truncated && <p className={`${note} border-t`}>The diff is too long to show all of it.</p>}
        </Suspense>
      )}
    </>
  )
}

function DiffSkeleton() {
  return (
    <div aria-label="Loading the diff" className="space-y-2 px-3 py-3">
      <Skeleton className="h-3 w-3/4" />
      <Skeleton className="h-3 w-1/2" />
      <Skeleton className="h-3 w-2/3" />
    </div>
  )
}

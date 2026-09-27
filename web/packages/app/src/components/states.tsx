import { Panel, PanelBody, Skeleton, SkeletonList } from "@openplan/ui"

import { useRowCursor } from "../lib/row-cursor"

const NO_ROWS: ReadonlyArray<string> = []

// While the list loads, the cursor keeps the rows of the page it left, and `j` then Enter would open
// one of them.
export function ListSkeleton() {
  useRowCursor(NO_ROWS)
  return (
    <Panel>
      <PanelBody className="p-6">
        <SkeletonList count={3} className="h-10 w-full" />
      </PanelBody>
    </Panel>
  )
}

export function DetailSkeleton() {
  return (
    <div className="space-y-4">
      <Skeleton className="h-8 w-2/3" />
      <Skeleton className="h-5 w-24" />
      <Skeleton className="h-40 w-full" />
    </div>
  )
}

export function BodySkeleton() {
  return <Skeleton className="h-40 w-full" />
}

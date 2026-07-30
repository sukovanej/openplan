import { Skeleton, SkeletonList } from "@open-planner/ui"

export function ListSkeleton() {
  return <SkeletonList count={5} className="h-14 w-full" />
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

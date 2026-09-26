import { Skeleton } from "@openplan/ui"

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

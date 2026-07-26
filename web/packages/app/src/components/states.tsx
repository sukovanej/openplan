import { Skeleton } from "@/components/ui/skeleton"

export function ListSkeleton() {
  return (
    <div className="space-y-3">
      {[0, 1, 2, 3, 4].map((i) => (
        <Skeleton key={i} className="h-14 w-full" />
      ))}
    </div>
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

export function Message({ title, detail }: { title: string; detail?: string }) {
  return (
    <div className="rounded-lg border border-dashed p-8 text-center">
      <p className="font-medium">{title}</p>
      {detail !== undefined && <p className="text-muted-foreground mt-1 text-sm">{detail}</p>}
    </div>
  )
}

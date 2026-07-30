export function EmptyState({ title, detail }: { title: string; detail?: string }) {
  return (
    <div className="rounded-lg border border-dashed p-8 text-center">
      <p className="font-medium">{title}</p>
      {detail !== undefined && <p className="text-muted-foreground mt-1 text-sm">{detail}</p>}
    </div>
  )
}

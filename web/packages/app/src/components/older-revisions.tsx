import { Button } from "@openplan/ui"

export function OlderRevisions({
  history,
  className,
}: {
  history: { hasNextPage: boolean; isFetchingNextPage: boolean; fetchNextPage: () => Promise<unknown> }
  className?: string
}) {
  if (!history.hasNextPage) return null
  return (
    <Button onClick={() => void history.fetchNextPage()} disabled={history.isFetchingNextPage} className={className}>
      {history.isFetchingNextPage ? "Loading" : "Show older revisions"}
    </Button>
  )
}

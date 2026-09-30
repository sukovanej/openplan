import { useEffect, useRef, useState } from "react"

import { Button, SkeletonList } from "@openplan/ui"

import { errorText } from "../lib/format"

// The list scrolls inside its panel, and the viewport would show the sentinel only once the panel does,
// too late to read ahead.
function scroller(element: Element): Element | null {
  for (let at = element.parentElement; at !== null; at = at.parentElement) {
    if (/auto|scroll/.test(getComputedStyle(at).overflowY)) return at
  }
  return null
}

export function OlderRevisionsOnScroll({
  history,
  className,
}: {
  history: {
    hasNextPage: boolean
    isFetching: boolean
    isFetchingNextPage: boolean
    isFetchNextPageError: boolean
    error: unknown
    fetchNextPage: (options?: { cancelRefetch?: boolean }) => Promise<unknown>
  }
  className?: string
}) {
  const sentinel = useRef<HTMLDivElement>(null)
  const [near, setNear] = useState(false)
  useEffect(() => {
    const element = sentinel.current
    if (element === null) return
    const observer = new IntersectionObserver(([entry]) => setNear(entry.isIntersecting), {
      root: scroller(element),
      rootMargin: "0px 0px 100% 0px",
    })
    observer.observe(element)
    return () => observer.disconnect()
  }, [])

  // The observer reports only a change, so each render asks again while the sentinel stays in reach,
  // until the list fills the panel. A read can start and end within one render, so no dependency
  // would change between them. A failed read waits for the reader, or a dead daemon would be asked in
  // a loop.
  const { fetchNextPage } = history
  const wanted = near && history.hasNextPage && !history.isFetching && !history.isFetchNextPageError
  useEffect(() => {
    if (wanted) void fetchNextPage({ cancelRefetch: false })
  })

  return (
    <div ref={sentinel} className={className}>
      {history.isFetchingNextPage ? (
        <SkeletonList count={3} className="h-16 w-full" />
      ) : history.isFetchNextPageError ? (
        <div className="flex items-center gap-3 text-sm">
          <p role="alert" className="text-danger">
            {errorText(history.error)}
          </p>
          <Button onClick={() => void fetchNextPage()}>Retry</Button>
        </div>
      ) : null}
    </div>
  )
}

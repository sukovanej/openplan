import { useFlash } from "../lib/flash"
import { cn } from "../lib/utils"

export function Flash() {
  const message = useFlash()
  // The region outlives any one message: a live region inserted together with its text is announced
  // unreliably, and an empty one is invisible anyway.
  return (
    <div role="status" aria-live="polite" className="pointer-events-none fixed right-7 bottom-7 z-50">
      {message !== undefined && (
        <span
          className={cn(
            "inline-block rounded-full border px-3 py-1.5 text-xs font-medium shadow-lg",
            message.tone === "ok"
              ? "bg-background text-foreground/90"
              : "border-red-500/30 bg-red-50 text-red-900 dark:bg-red-950 dark:text-red-100",
          )}
        >
          {message.text}
        </span>
      )}
    </div>
  )
}

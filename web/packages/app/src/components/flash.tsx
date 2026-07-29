import { useFlash } from "../lib/flash"
import { cn } from "../lib/utils"

export function Flash() {
  const message = useFlash()
  if (message === undefined) return null
  return (
    <div
      role="status"
      className={cn(
        "fixed right-4 bottom-4 z-50 rounded-full border px-3 py-1.5 text-xs font-medium shadow-lg",
        message.tone === "ok"
          ? "bg-background text-foreground/90"
          : "border-red-500/30 bg-red-50 text-red-900 dark:bg-red-950 dark:text-red-100",
      )}
    >
      {message.text}
    </div>
  )
}

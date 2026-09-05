import { type RefObject, useEffect } from "react"

// `onDismiss` undefined means the caller is closed, or does not dismiss at all.
export function useDismissOnOutsideClick(root: RefObject<HTMLElement | null>, onDismiss: (() => void) | undefined) {
  useEffect(() => {
    if (onDismiss === undefined) return
    const onDown = (event: MouseEvent) => {
      if (root.current !== null && !root.current.contains(event.target as Node)) onDismiss()
    }
    document.addEventListener("mousedown", onDown)
    return () => document.removeEventListener("mousedown", onDown)
  }, [root, onDismiss])
}

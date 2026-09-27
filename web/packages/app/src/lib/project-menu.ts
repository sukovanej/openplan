import { useEffect, useEffectEvent } from "react"

const listeners = new Set<() => void>()

export function openProjectMenu(): void {
  for (const listener of listeners) listener()
}

export function useProjectMenuRequest(open: () => void): void {
  const onAsked = useEffectEvent(open)
  useEffect(() => {
    const listener = () => onAsked()
    listeners.add(listener)
    return () => {
      listeners.delete(listener)
    }
  }, [])
}

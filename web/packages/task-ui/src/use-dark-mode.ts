import { useSyncExternalStore } from "react"

import type { DiagramTheme } from "./diagram"

const DARK_CLASS = "dark"

function subscribe(listener: () => void): () => void {
  const observer = new MutationObserver(listener)
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] })
  return () => observer.disconnect()
}

function currentTheme(): DiagramTheme {
  return document.documentElement.classList.contains(DARK_CLASS) ? "dark" : "light"
}

// The app flips `.dark` on <html>; reading that class keeps this package free of the app's store.
export function useResolvedTheme(): DiagramTheme {
  return useSyncExternalStore(subscribe, currentTheme, () => "light")
}

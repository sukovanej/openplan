import { useSyncExternalStore } from "react"

export type ThemePreference = "light" | "dark" | "system"
export type ResolvedTheme = "light" | "dark"

export const THEME_STORAGE_KEY = "openplan.theme"

const DARK_QUERY = "(prefers-color-scheme: dark)"
const PREFERENCES: ReadonlyArray<ThemePreference> = ["light", "dark", "system"]

export function isThemePreference(value: unknown): value is ThemePreference {
  return PREFERENCES.includes(value as ThemePreference)
}

export function readPreference(): ThemePreference {
  let stored: string | null = null
  try {
    stored = window.localStorage.getItem(THEME_STORAGE_KEY)
  } catch {
    stored = null
  }
  return isThemePreference(stored) ? stored : "system"
}

export function writePreference(preference: ThemePreference): void {
  try {
    window.localStorage.setItem(THEME_STORAGE_KEY, preference)
  } catch {
    // A blocked or unavailable localStorage still drives the in-memory store below.
  }
}

export function systemTheme(): ResolvedTheme {
  if (typeof window.matchMedia !== "function") return "light"
  return window.matchMedia(DARK_QUERY).matches ? "dark" : "light"
}

export function resolve(preference: ThemePreference): ResolvedTheme {
  return preference === "system" ? systemTheme() : preference
}

export function apply(theme: ResolvedTheme): void {
  const root = document.documentElement
  root.classList.toggle("dark", theme === "dark")
  root.style.colorScheme = theme
}

export interface ThemeState {
  readonly preference: ThemePreference
  readonly resolved: ResolvedTheme
}

export class ThemeStore {
  private state: ThemeState
  private readonly listeners = new Set<() => void>()
  private media: MediaQueryList | null = null

  constructor() {
    const preference = readPreference()
    this.state = { preference, resolved: resolve(preference) }
  }

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    if (this.listeners.size === 1) this.connect()
    return () => {
      this.listeners.delete(listener)
      if (this.listeners.size === 0) this.disconnect()
    }
  }

  readonly getSnapshot = (): ThemeState => this.state

  readonly setPreference = (preference: ThemePreference): void => {
    writePreference(preference)
    this.update(preference)
  }

  private connect(): void {
    apply(this.state.resolved)
    window.addEventListener("storage", this.onStorage)
    this.watchSystem()
  }

  private disconnect(): void {
    window.removeEventListener("storage", this.onStorage)
    this.unwatchSystem()
  }

  private readonly onStorage = (event: StorageEvent): void => {
    if (event.key !== null && event.key !== THEME_STORAGE_KEY) return
    this.update(readPreference())
  }

  private readonly onSystemChange = (): void => {
    if (this.state.preference === "system") this.update("system")
  }

  private watchSystem(): void {
    if (this.media !== null || this.state.preference !== "system") return
    if (typeof window.matchMedia !== "function") return
    this.media = window.matchMedia(DARK_QUERY)
    this.media.addEventListener("change", this.onSystemChange)
  }

  private unwatchSystem(): void {
    if (this.media === null) return
    this.media.removeEventListener("change", this.onSystemChange)
    this.media = null
  }

  private update(preference: ThemePreference): void {
    const resolved = resolve(preference)
    if (preference === this.state.preference && resolved === this.state.resolved) return
    this.state = { preference, resolved }
    apply(resolved)
    if (this.listeners.size > 0) {
      if (preference === "system") this.watchSystem()
      else this.unwatchSystem()
    }
    for (const listener of this.listeners) listener()
  }
}

export const themeStore = new ThemeStore()

export interface ThemeControls extends ThemeState {
  readonly setPreference: (preference: ThemePreference) => void
}

export function useTheme(): ThemeControls {
  const state = useSyncExternalStore(themeStore.subscribe, themeStore.getSnapshot)
  return { ...state, setPreference: themeStore.setPreference }
}

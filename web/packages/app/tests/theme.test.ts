// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest"

import {
  apply,
  readPreference,
  resolve,
  THEME_STORAGE_KEY,
  ThemeStore,
  themeStore,
  writePreference,
} from "../src/lib/theme"

function define(target: object, key: string, value: unknown) {
  Object.defineProperty(target, key, { value, configurable: true, writable: true })
}

function installLocalStorage() {
  const map = new Map<string, string>()
  const storage = {
    get length() {
      return map.size
    },
    clear: () => map.clear(),
    getItem: (key: string) => (map.has(key) ? map.get(key)! : null),
    key: (index: number) => [...map.keys()][index] ?? null,
    removeItem: (key: string) => void map.delete(key),
    setItem: (key: string, value: string) => void map.set(key, String(value)),
  }
  define(window, "localStorage", storage as Storage)
}

function installMatchMedia(initial = false) {
  let matches = initial
  const listeners = new Set<(event: MediaQueryListEvent) => void>()
  const mql = {
    get matches() {
      return matches
    },
    media: "(prefers-color-scheme: dark)",
    addEventListener(_type: "change", cb: (event: MediaQueryListEvent) => void) {
      listeners.add(cb)
    },
    removeEventListener(_type: "change", cb: (event: MediaQueryListEvent) => void) {
      listeners.delete(cb)
    },
  }
  define(window, "matchMedia", () => mql as unknown as MediaQueryList)
  return {
    flip(next: boolean) {
      matches = next
      for (const cb of listeners) cb({ matches } as MediaQueryListEvent)
    },
    get listenerCount() {
      return listeners.size
    },
  }
}

beforeEach(() => {
  installLocalStorage()
  define(window, "matchMedia", undefined)
  document.documentElement.className = ""
  document.documentElement.style.colorScheme = ""
})

describe("preference persistence", () => {
  it("round-trips through localStorage", () => {
    writePreference("dark")
    expect(readPreference()).toBe("dark")
    expect(window.localStorage.getItem(THEME_STORAGE_KEY)).toBe("dark")
  })

  it("falls back to system when unset", () => {
    expect(readPreference()).toBe("system")
  })

  it("falls back to system when the stored value is unrecognized", () => {
    window.localStorage.setItem(THEME_STORAGE_KEY, "banana")
    expect(readPreference()).toBe("system")
  })
})

describe("resolve", () => {
  it("follows matchMedia in system mode", () => {
    const media = installMatchMedia(true)
    expect(resolve("system")).toBe("dark")
    media.flip(false)
    expect(resolve("system")).toBe("light")
  })

  it("ignores matchMedia for light and dark", () => {
    installMatchMedia(true)
    expect(resolve("light")).toBe("light")
    expect(resolve("dark")).toBe("dark")
  })
})

describe("apply", () => {
  it("toggles the .dark class and color-scheme on documentElement", () => {
    const root = document.documentElement
    apply("dark")
    expect(root.classList.contains("dark")).toBe(true)
    expect(root.style.colorScheme).toBe("dark")
    apply("light")
    expect(root.classList.contains("dark")).toBe(false)
    expect(root.style.colorScheme).toBe("light")
  })
})

describe("live system tracking", () => {
  it("flips the resolved theme when the OS changes in system mode", () => {
    const media = installMatchMedia(false)
    window.localStorage.setItem(THEME_STORAGE_KEY, "system")
    const store = new ThemeStore()
    const unsubscribe = store.subscribe(() => {})
    expect(store.getSnapshot().resolved).toBe("light")
    media.flip(true)
    expect(store.getSnapshot().resolved).toBe("dark")
    unsubscribe()
    expect(media.listenerCount).toBe(0)
  })

  it("ignores OS changes when a fixed theme is selected", () => {
    const media = installMatchMedia(false)
    window.localStorage.setItem(THEME_STORAGE_KEY, "dark")
    const store = new ThemeStore()
    const unsubscribe = store.subscribe(() => {})
    expect(media.listenerCount).toBe(0)
    media.flip(true)
    expect(store.getSnapshot().resolved).toBe("dark")
    unsubscribe()
  })

  it("attaches the media listener only after switching to system", () => {
    const media = installMatchMedia(true)
    window.localStorage.setItem(THEME_STORAGE_KEY, "light")
    const store = new ThemeStore()
    const unsubscribe = store.subscribe(() => {})
    expect(media.listenerCount).toBe(0)
    store.setPreference("system")
    expect(store.getSnapshot().resolved).toBe("dark")
    expect(media.listenerCount).toBe(1)
    unsubscribe()
  })
})

describe("cross-tab sync", () => {
  it("adopts a preference written by another tab", () => {
    installMatchMedia(false)
    window.localStorage.setItem(THEME_STORAGE_KEY, "light")
    const store = new ThemeStore()
    const unsubscribe = store.subscribe(() => {})
    expect(store.getSnapshot().preference).toBe("light")

    window.localStorage.setItem(THEME_STORAGE_KEY, "dark")
    window.dispatchEvent(new StorageEvent("storage", { key: THEME_STORAGE_KEY, newValue: "dark" }))

    expect(store.getSnapshot().preference).toBe("dark")
    expect(store.getSnapshot().resolved).toBe("dark")
    unsubscribe()
  })

  it("ignores storage events for unrelated keys", () => {
    installMatchMedia(false)
    window.localStorage.setItem(THEME_STORAGE_KEY, "light")
    const store = new ThemeStore()
    const unsubscribe = store.subscribe(() => {})

    window.dispatchEvent(new StorageEvent("storage", { key: "other", newValue: "dark" }))

    expect(store.getSnapshot().preference).toBe("light")
    unsubscribe()
  })
})

describe("exported singleton", () => {
  it("exposes a store whose snapshot has a preference and a resolved theme", () => {
    const snapshot = themeStore.getSnapshot()
    expect(["light", "dark", "system"]).toContain(snapshot.preference)
    expect(["light", "dark"]).toContain(snapshot.resolved)
  })
})

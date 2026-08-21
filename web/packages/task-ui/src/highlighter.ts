import type { HighlighterCore, ShikiTransformer } from "@shikijs/core"
import type { Root } from "hast"
import { useSyncExternalStore } from "react"

const LIGHT_THEME = "github-light"
const DARK_THEME = "github-dark"

const GRAMMARS = {
  css: () => import("@shikijs/langs/css"),
  diff: () => import("@shikijs/langs/diff"),
  go: () => import("@shikijs/langs/go"),
  html: () => import("@shikijs/langs/html"),
  javascript: () => import("@shikijs/langs/javascript"),
  json: () => import("@shikijs/langs/json"),
  jsx: () => import("@shikijs/langs/jsx"),
  markdown: () => import("@shikijs/langs/markdown"),
  python: () => import("@shikijs/langs/python"),
  rust: () => import("@shikijs/langs/rust"),
  shellscript: () => import("@shikijs/langs/shellscript"),
  sql: () => import("@shikijs/langs/sql"),
  toml: () => import("@shikijs/langs/toml"),
  tsx: () => import("@shikijs/langs/tsx"),
  typescript: () => import("@shikijs/langs/typescript"),
  yaml: () => import("@shikijs/langs/yaml"),
} as const

export type CodeLanguage = keyof typeof GRAMMARS

const TAGS: Readonly<Record<string, CodeLanguage>> = {
  bash: "shellscript",
  css: "css",
  diff: "diff",
  go: "go",
  golang: "go",
  html: "html",
  javascript: "javascript",
  js: "javascript",
  json: "json",
  jsx: "jsx",
  markdown: "markdown",
  md: "markdown",
  patch: "diff",
  py: "python",
  python: "python",
  rs: "rust",
  rust: "rust",
  sh: "shellscript",
  shell: "shellscript",
  sql: "sql",
  toml: "toml",
  ts: "typescript",
  tsx: "tsx",
  typescript: "typescript",
  yaml: "yaml",
  yml: "yaml",
  zsh: "shellscript",
}

export function resolveLang(tag: string | undefined): CodeLanguage | null {
  if (tag === undefined) return null
  return TAGS[tag.trim().toLowerCase()] ?? null
}

let highlighter: HighlighterCore | null = null
let build: Promise<void> | null = null
const listeners = new Set<() => void>()

async function createHighlighter(): Promise<void> {
  try {
    const [{ createHighlighterCore }, { createJavaScriptRegexEngine }] = await Promise.all([
      import("@shikijs/core"),
      import("@shikijs/engine-javascript"),
    ])
    highlighter = await createHighlighterCore({
      themes: [() => import("@shikijs/themes/github-light"), () => import("@shikijs/themes/github-dark")],
      langs: Object.values(GRAMMARS),
      // The JavaScript engine is what makes `codeToHast` synchronous, which react-markdown's
      // synchronous pipeline needs; `forgiving` keeps a pattern it cannot compile from throwing.
      engine: createJavaScriptRegexEngine({ forgiving: true }),
    })
  } catch (error) {
    console.error("Syntax highlighting is unavailable", error)
  }
  for (const listener of listeners) listener()
}

export function ensureHighlighter(): Promise<void> {
  build ??= createHighlighter()
  return build
}

export function highlightToHast(code: string, lang: CodeLanguage, transformers?: Array<ShikiTransformer>): Root | null {
  if (highlighter === null) return null
  try {
    return highlighter.codeToHast(code, {
      lang,
      themes: { light: LIGHT_THEME, dark: DARK_THEME },
      defaultColor: false,
      transformers,
    })
  } catch (error) {
    console.error(`Cannot highlight a ${lang} code block`, error)
    return null
  }
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener)
  void ensureHighlighter()
  return () => {
    listeners.delete(listener)
  }
}

function isReady(): boolean {
  return highlighter !== null
}

export function useHighlighterReady(): boolean {
  return useSyncExternalStore(subscribe, isReady)
}

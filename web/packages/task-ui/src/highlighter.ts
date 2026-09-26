import type { HighlighterCore, ShikiTransformer } from "@shikijs/core"
import type { Root } from "hast"
import { useEffect, useSyncExternalStore } from "react"

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

let building: Promise<HighlighterCore | null> | null = null
let built: HighlighterCore | null = null
const grammars = new Map<CodeLanguage, Promise<void>>()
const ready = new Set<CodeLanguage>()
const listeners = new Set<() => void>()

async function createHighlighter(): Promise<HighlighterCore | null> {
  try {
    const [{ createHighlighterCore }, { createJavaScriptRegexEngine }] = await Promise.all([
      import("@shikijs/core"),
      import("@shikijs/engine-javascript"),
    ])
    built = await createHighlighterCore({
      themes: [() => import("@shikijs/themes/github-light"), () => import("@shikijs/themes/github-dark")],
      langs: [],
      // The JavaScript engine is what makes `codeToHast` synchronous, which react-markdown's
      // synchronous pipeline needs; `forgiving` keeps a pattern it cannot compile from throwing.
      engine: createJavaScriptRegexEngine({ forgiving: true }),
    })
  } catch (error) {
    console.error("Syntax highlighting is unavailable", error)
  }
  return built
}

async function loadGrammar(lang: CodeLanguage): Promise<void> {
  building ??= createHighlighter()
  const core = await building
  if (core === null) return
  try {
    await core.loadLanguage(GRAMMARS[lang])
    ready.add(lang)
  } catch (error) {
    console.error(`The ${lang} grammar is unavailable`, error)
  }
  for (const listener of listeners) listener()
}

// Each grammar is a chunk of its own, and a task body seldom holds more than one or two languages, so
// a grammar loads when the first block of its language needs it.
export function ensureHighlighter(lang: CodeLanguage): Promise<void> {
  let pending = grammars.get(lang)
  if (pending === undefined) {
    pending = loadGrammar(lang)
    grammars.set(lang, pending)
  }
  return pending
}

export function highlightToHast(code: string, lang: CodeLanguage, transformers?: Array<ShikiTransformer>): Root | null {
  if (built === null || !ready.has(lang)) return null
  try {
    return built.codeToHast(code, {
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

export interface CodeToken {
  readonly offset: number
  readonly length: number
  readonly style: string
}

// The same two palettes as `highlightToHast`, as spans an editor can lay over text it does not own.
export function highlightTokens(code: string, lang: CodeLanguage): ReadonlyArray<CodeToken> | null {
  if (built === null || !ready.has(lang)) return null
  try {
    const { tokens } = built.codeToTokens(code, {
      lang,
      themes: { light: LIGHT_THEME, dark: DARK_THEME },
      defaultColor: false,
    })
    return tokens.flat().flatMap((token) => {
      const style = token.htmlStyle
      if (style === undefined) return []
      const css = Object.entries(style)
        .map(([name, value]) => `${name}:${value}`)
        .join(";")
      return [{ offset: token.offset, length: token.content.length, style: css }]
    })
  } catch (error) {
    console.error(`Cannot highlight a ${lang} code block`, error)
    return null
  }
}

export function watchHighlighter(listener: () => void): () => void {
  return subscribe(listener)
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

export function useHighlighterReady(lang: CodeLanguage): boolean {
  useEffect(() => {
    void ensureHighlighter(lang)
  }, [lang])
  return useSyncExternalStore(subscribe, () => ready.has(lang))
}

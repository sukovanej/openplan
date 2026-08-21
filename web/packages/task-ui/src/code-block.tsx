import type { ShikiTransformer } from "@shikijs/core"
import type { Element } from "hast"
import { toJsxRuntime } from "hast-util-to-jsx-runtime"
import { type ComponentProps, type ReactNode, useMemo } from "react"
import { Fragment, jsx, jsxs } from "react/jsx-runtime"

import { type CodeLanguage, highlightToHast, resolveLang, useHighlighterReady } from "./highlighter"

// Shiki writes both palettes as `--shiki-light` / `--shiki-dark` on every token, so the app theme
// picks one in CSS and a theme change never re-highlights.
const tokenColours = "[&_span]:text-[var(--shiki-light)] dark:[&_span]:text-[var(--shiki-dark)]"

const paintTokens: ShikiTransformer = {
  name: "task-ui:token-colours",
  pre(node) {
    this.addClassToHast(node, tokenColours)
  },
}

const LANGUAGE_CLASS = "language-"

function languageTag(className: unknown): string | undefined {
  if (!Array.isArray(className)) return undefined
  for (const name of className) {
    if (typeof name === "string" && name.startsWith(LANGUAGE_CLASS)) return name.slice(LANGUAGE_CLASS.length)
  }
  return undefined
}

function fencedCode(node: Element | undefined): { source: string; lang: CodeLanguage } | null {
  if (node === undefined || node.children.length !== 1) return null
  const code = node.children[0]
  if (code?.type !== "element" || code.tagName !== "code" || code.children.length !== 1) return null
  const text = code.children[0]
  if (text?.type !== "text") return null
  const lang = resolveLang(languageTag(code.properties.className))
  // The source keeps its trailing newline, so the highlighted block has the same line count as the
  // plain one below it and the swap moves nothing.
  return lang === null ? null : { source: text.value, lang }
}

// Only a block Shiki can colour mounts this, so a fence with no language never pulls the chunk in.
function HighlightedCode({ source, lang, children }: { source: string; lang: CodeLanguage; children: ReactNode }) {
  const ready = useHighlighterReady()
  const tree = useMemo(() => (ready ? highlightToHast(source, lang, [paintTokens]) : null), [ready, source, lang])
  return tree === null ? children : toJsxRuntime(tree, { Fragment, jsx, jsxs })
}

export function CodeBlock({ node, children, ...props }: ComponentProps<"pre"> & { node?: Element }) {
  const plain = <pre {...props}>{children}</pre>
  const fence = fencedCode(node)
  if (fence === null) return plain
  return (
    <HighlightedCode source={fence.source} lang={fence.lang}>
      {plain}
    </HighlightedCode>
  )
}

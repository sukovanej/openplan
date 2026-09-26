import { syntaxTree } from "@codemirror/language"
import type { EditorState } from "@codemirror/state"
import type { SyntaxNode } from "@lezer/common"

const CODE = new Set(["InlineCode", "FencedCode", "CodeBlock", "CodeText", "HTMLBlock", "HTMLTag", "Comment"])

// Text inside code or HTML is quoted source: no reference, no command, no mark lives there.
export function isInCode(state: EditorState, at: number, side: -1 | 1): boolean {
  for (let node: SyntaxNode | null = syntaxTree(state).resolveInner(at, side); node !== null; node = node.parent) {
    if (CODE.has(node.name)) return true
  }
  return false
}

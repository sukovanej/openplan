import { EditorSelection, type Extension, Prec } from "@codemirror/state"
import { type Command, EditorView, keymap } from "@codemirror/view"

import { taskRouteOf } from "@openplan/task-ui"

export interface EditorActions {
  readonly project: string
  readonly save: () => void
  readonly navigate: (path: string) => void
}

// Wraps each selection in `marker`, or takes the marker away where it already wraps it. With no
// selection, a caret before a closing marker steps past it, so a second press ends the run.
const toggleMarker =
  (marker: string): Command =>
  (view) => {
    const size = marker.length
    view.dispatch(
      view.state.changeByRange((range) => {
        const before = view.state.sliceDoc(range.from - size, range.from)
        const after = view.state.sliceDoc(range.to, range.to + size)
        if (before === marker && after === marker) {
          return {
            changes: [
              { from: range.from - size, to: range.from, insert: "" },
              { from: range.to, to: range.to + size, insert: "" },
            ],
            range: EditorSelection.range(range.from - size, range.to - size),
          }
        }
        if (range.empty && after === marker) {
          return { range: EditorSelection.cursor(range.to + size) }
        }
        return {
          changes: [
            { from: range.from, insert: marker },
            { from: range.to, insert: marker },
          ],
          range: EditorSelection.range(range.from + size, range.to + size),
        }
      }),
    )
    return true
  }

// `[text](url)` with the address selected, ready to paste over.
const insertLink: Command = (view) => {
  view.dispatch(
    view.state.changeByRange((range) => {
      const text = view.state.sliceDoc(range.from, range.to)
      const insert = `[${text}](url)`
      const url = range.from + text.length + 3
      return { changes: { from: range.from, to: range.to, insert }, range: EditorSelection.range(url, url + 3) }
    }),
  )
  return true
}

const leave: Command = (view) => {
  view.contentDOM.blur()
  return true
}

const isUrl = (text: string) => /^https?:\/\/\S+$/.test(text)

// A link pasted over a selection links the selection; a link to a task of this project becomes a
// reference to it.
const pasteLink = (project: string) =>
  EditorView.domEventHandlers({
    paste: (event, view) => {
      const text = event.clipboardData?.getData("text/plain").trim() ?? ""
      if (!isUrl(text)) return false
      const url = new URL(text)
      const route = url.origin === window.location.origin ? taskRouteOf(url.pathname) : undefined
      const range = view.state.selection.main
      if (route?.project === project) {
        view.dispatch(view.state.replaceSelection(`[[${route.id}]]`))
      } else if (!range.empty) {
        view.dispatch(view.state.replaceSelection(`[${view.state.sliceDoc(range.from, range.to)}](${text})`))
      } else {
        return false
      }
      event.preventDefault()
      return true
    },
  })

// A rendered link opens on a click, as it would in the read view. The source shows when the caret
// reaches it from the keyboard.
const openLinks = (navigate: (path: string) => void) =>
  EditorView.domEventHandlers({
    mousedown: (event) => {
      const href = (event.target as HTMLElement).closest<HTMLElement>("[data-href]")?.dataset.href
      if (href === undefined || event.button !== 0) return false
      event.preventDefault()
      if (href.startsWith("/") && !event.metaKey && !event.ctrlKey) navigate(href)
      else window.open(href, "_blank", "noreferrer")
      return true
    },
  })

export function editorKeys({ project, save, navigate }: EditorActions): Extension {
  return [
    Prec.high(
      keymap.of([
        { key: "Mod-b", run: toggleMarker("**") },
        { key: "Mod-i", run: toggleMarker("*") },
        { key: "Mod-e", run: toggleMarker("`") },
        { key: "Mod-Shift-x", run: toggleMarker("~~") },
        { key: "Mod-k", run: insertLink },
        {
          key: "Mod-s",
          run: () => {
            save()
            return true
          },
          preventDefault: true,
        },
        { key: "Escape", run: leave },
      ]),
    ),
    pasteLink(project),
    openLinks(navigate),
  ]
}

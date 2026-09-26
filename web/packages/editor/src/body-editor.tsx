import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands"
import { markdown as markdownSupport, markdownLanguage } from "@codemirror/lang-markdown"
import { Annotation, EditorSelection, EditorState, Transaction } from "@codemirror/state"
import { EditorView, keymap, placeholder } from "@codemirror/view"
import {
  type Ref,
  useEffect,
  useEffectEvent,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react"
import { createPortal } from "react-dom"

import type { DocRef, TaskRef } from "@openplan/api-client"
import { Prose } from "@openplan/ui"

import { completions, type RefOption } from "./completion"
import { editorKeys } from "./keys"
import { livePreview } from "./live-preview"
import { EditorScopeContext, Portals, portals } from "./portals"

export interface BodyEditorHandle {
  readonly focus: (at: "start" | "end") => void
}

export interface BodyEditorProps {
  readonly project: string
  readonly abbreviation: string
  readonly refs: ReadonlyArray<TaskRef> | undefined
  readonly docRefs: ReadonlyArray<DocRef> | undefined
  // The text to show. The editor takes it in whenever it differs from what the editor holds, so the
  // owner passes a new one only when it means to replace the reader's text.
  readonly markdown: string
  readonly onChange: (markdown: string) => void
  readonly onSave: () => void
  readonly searchRefs: (query: string) => ReadonlyArray<RefOption>
  readonly label: string
  readonly placeholder: string
  readonly navigate: (path: string) => void
  readonly ref?: Ref<BodyEditorHandle>
}

const fromOwner = Annotation.define<boolean>()

// Only the part that differs is replaced, so the caret and the history keep their place around it.
function difference(current: string, next: string) {
  let start = 0
  while (start < current.length && start < next.length && current[start] === next[start]) start++
  let end = 0
  while (
    end < current.length - start &&
    end < next.length - start &&
    current[current.length - 1 - end] === next[next.length - 1 - end]
  ) {
    end++
  }
  return { from: start, to: current.length - end, insert: next.slice(start, next.length - end) }
}

export function BodyEditor({
  project,
  abbreviation,
  refs,
  docRefs,
  markdown,
  onChange,
  onSave,
  searchRefs,
  label,
  placeholder: empty,
  navigate,
  ref,
}: BodyEditorProps) {
  const host = useRef<HTMLDivElement>(null)
  const view = useRef<EditorView>(null)
  const [initial] = useState(() => ({ markdown, project, abbreviation, label, empty }))
  const [portalHost] = useState(() => new Portals())
  const [pickedTasks, setPickedTasks] = useState<ReadonlyArray<TaskRef>>([])
  const [pickedDocs, setPickedDocs] = useState<ReadonlyArray<DocRef>>([])
  const mounted = useSyncExternalStore(portalHost.subscribe, portalHost.getSnapshot)

  // The view lives as long as the component, and its listeners reach the props of the latest render.
  const changed = useEffectEvent((text: string) => onChange(text))
  const saved = useEffectEvent(() => onSave())
  const went = useEffectEvent((path: string) => navigate(path))
  const found = useEffectEvent((query: string) => searchRefs(query))
  const picked = (option: RefOption) => {
    if (option.kind === "task") {
      setPickedTasks((current) => [...current.filter((one) => one.id !== option.task.id), option.task])
    } else {
      setPickedDocs((current) => [...current.filter((one) => one.name !== option.doc.name), option.doc])
    }
  }

  useEffect(() => {
    const parent = host.current
    if (parent === null) return
    const created = new EditorView({
      parent,
      state: EditorState.create({
        doc: initial.markdown,
        extensions: [
          portals.of(portalHost),
          markdownSupport({ base: markdownLanguage }),
          history(),
          keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
          EditorView.lineWrapping,
          EditorView.contentAttributes.of({ "aria-label": initial.label, spellcheck: "true" }),
          placeholder(initial.empty),
          livePreview(initial.abbreviation),
          completions({
            search: (query) => found(query),
            picked,
          }),
          editorKeys({ project: initial.project, save: () => saved(), navigate: (path) => went(path) }),
          EditorView.updateListener.of((update) => {
            if (update.docChanged && !update.transactions.some((tr) => tr.annotation(fromOwner) === true)) {
              changed(update.state.doc.toString())
            }
          }),
        ],
      }),
    })
    view.current = created
    return () => {
      view.current = null
      created.destroy()
    }
  }, [initial, portalHost])

  useEffect(() => {
    const current = view.current
    if (current === null) return
    const text = current.state.doc.toString()
    if (text === markdown) return
    current.dispatch({
      changes: difference(text, markdown),
      annotations: [fromOwner.of(true), Transaction.addToHistory.of(false)],
    })
  }, [markdown])

  useImperativeHandle(
    ref,
    () => ({
      focus: (at) => {
        const current = view.current
        if (current === null) return
        current.focus()
        const anchor = at === "start" ? 0 : current.state.doc.length
        current.dispatch({ selection: EditorSelection.cursor(anchor), scrollIntoView: true })
      },
    }),
    [],
  )

  // A task or a doc picked in this visit has no entry in `refs` until the save comes back with one.
  const scope = useMemo(
    () => ({
      project,
      abbreviation,
      refs: [...pickedTasks, ...(refs ?? [])],
      docRefs: [...pickedDocs, ...(docRefs ?? [])],
    }),
    [project, abbreviation, refs, docRefs, pickedTasks, pickedDocs],
  )

  return (
    <EditorScopeContext.Provider value={scope}>
      <Prose className="op-editor">
        <div ref={host} />
      </Prose>
      {mounted.map(({ key, element, node }) => createPortal(node, element, String(key)))}
    </EditorScopeContext.Provider>
  )
}

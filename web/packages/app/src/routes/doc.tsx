import { useQuery, useQueryClient } from "@tanstack/react-query"
import { Check, FileText, Pencil, Plus, Trash2, Undo2, X } from "lucide-react"
import { type MouseEvent, type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react"
import { Link, useLocation, useNavigate, useParams, useSearchParams } from "react-router-dom"

import type { DocChild, DocDetail, DocListItem, DocSnapshot } from "@openplan/api-client"
import {
  bodySegments,
  ProblemBanner,
  REVISION_PARAM,
  RevisionMeta,
  shortRevision,
  TaskAuthor,
  TaskBodyWithConflicts,
  docCreatedOf,
  DocParentLink,
  docParentOf,
  docPath,
  docProblems,
  docsPath,
  TaskTimes,
} from "@openplan/task-ui"
import {
  Button,
  type ComboOption,
  Combobox,
  EmptyState,
  FuzzyText,
  MetaLine,
  Panel,
  PanelBody,
  PanelHeader,
  PanelTitle,
  Row,
  Section,
  SkeletonList,
  Tooltip,
} from "@openplan/ui"

import { DetailColumns } from "../components/detail-columns"
import { DocHistory } from "../components/doc-history"
import { DocConflictBanner, DocFieldConflictControl } from "../components/field-conflict"
import { BodySkeleton } from "../components/states"
import { DocContent } from "../components/task-content"
import { createDoc, deleteDoc, getDoc, listAllDocs, patchDoc, TaskNotFound } from "../lib/api"
import { useDetailAction } from "../lib/detail-actions"
import { docLabel, docMatches } from "../lib/doc-search"
import { errorText } from "../lib/format"
import { useDocHistory, useDocRevision } from "../lib/history"
import { useAbbreviation } from "../lib/projects"
import { allDocsKey, docKey, useProjectMutation } from "../lib/query-client"
import { isOverLiveTask } from "../lib/revision-navigation"
import { detailCursor, useDetailCursor } from "../lib/row-cursor"
import { hoveredRow } from "../lib/row-target"
import { abortable } from "../lib/runtime"

const NO_DOCS: ReadonlyArray<DocListItem> = []
const NO_ROWS: ReadonlyArray<string> = []

export function DocRoute() {
  const { project = "", name = "" } = useParams()
  const [params] = useSearchParams()
  const revision = params.get(REVISION_PARAM)
  return revision === null ? (
    <Doc key={`${project}:${name}`} project={project} name={name} />
  ) : (
    <DocAtRevision key={`${project}:${name}@${revision}`} project={project} name={name} revision={revision} />
  )
}

function Doc({ project, name }: { project: string; name: string }) {
  const doc = useQuery({
    queryKey: docKey(project, name),
    queryFn: abortable(getDoc(project, name)),
  })

  if (doc.isError) {
    return doc.error instanceof TaskNotFound ? (
      <EmptyState title="No such doc" detail={`${project} has no doc ${name}`} />
    ) : (
      <EmptyState title="Could not load doc" detail={errorText(doc.error)} />
    )
  }
  if (doc.data === undefined) {
    return (
      <Panel>
        <PanelBody className="p-6">
          <SkeletonList count={4} className="h-5 w-full" />
        </PanelBody>
      </Panel>
    )
  }
  return <DocView doc={doc.data} project={project} />
}

function DocView({ doc, project }: { doc: DocDetail; project: string }) {
  const abbreviation = useAbbreviation(project)

  const rows = useMemo(() => (doc.children ?? []).map((child) => docPath(project, child.name)), [doc, project])
  const meta = (saveNote: ReactNode) => (
    <div className="mb-4 flex min-h-4 items-center justify-between gap-4">
      <MetaLine className="h-4">
        <TaskAuthor author={doc.author} withAgent />
        <TaskTimes created={docCreatedOf(doc.metadata)} updated={doc.updated} problems={docProblems(doc.metadata)} />
        <DocFieldConflictControl project={project} name={doc.name} metadata={doc.metadata} field="created" />
        {saveNote}
      </MetaLine>
      <div className="flex shrink-0 items-center gap-1">
        <DeleteControl project={project} doc={doc} />
      </div>
    </div>
  )
  const { index } = useDetailCursor(`${project}:${doc.name}`, rows)

  return (
    <DetailColumns
      main={
        <>
          <PanelHeader className="gap-2">
            <PanelTitle>{doc.title || doc.name}</PanelTitle>
            <Link
              to={docsPath(project)}
              className="text-muted-foreground hover:text-foreground ml-auto shrink-0 text-xs"
            >
              ← Docs
            </Link>
            <div className="flex min-w-0 items-center gap-1.5">
              <DocFieldConflictControl
                project={project}
                name={doc.name}
                metadata={doc.metadata}
                field="parent"
                trigger="Parent conflict"
                align="end"
              />
              <HeaderParent project={project} doc={doc} />
            </div>
          </PanelHeader>
          <PanelBody className="p-6">
            <DocConflictBanner project={project} name={doc.name} metadata={doc.metadata} count={doc.conflicts} />
            {abbreviation === undefined ? (
              <>
                <ProblemBanner problems={doc.problems} />
                <BodySkeleton />
              </>
            ) : (
              <DocContent project={project} doc={doc} abbreviation={abbreviation} meta={meta} />
            )}
          </PanelBody>
        </>
      }
      aside={
        <>
          <NestedSection project={project} doc={doc} rows={rows} cursor={index} />
          <DocHistory project={project} name={doc.name} selected={undefined} />
        </>
      }
    />
  )
}

// A revision never changes, so nothing here can be edited.
function DocAtRevision({ project, name, revision }: { project: string; name: string; revision: string }) {
  const snapshot = useDocRevision(project, name, revision)
  const entry = useDocHistory(project, name).data?.find((one) => one.revision.id === revision)
  const doc = snapshot.data?.doc
  useDetailCursor(`${docPath(project, name)}@${revision}`, NO_ROWS)
  return (
    <DetailColumns
      main={
        <>
          <PanelHeader className="gap-2">
            <PanelTitle>{doc?.title || name}</PanelTitle>
            <CurrentVersionLink project={project} name={name} />
          </PanelHeader>
          <PanelBody className="p-6">
            <div
              role="note"
              className="border-info/40 bg-info/5 mb-5 flex flex-col gap-1 rounded-md border px-3 py-2 text-xs"
            >
              <p className="text-info">
                This is the doc as revision <span className="font-mono">{shortRevision(revision)}</span> left it. You
                cannot change it here.
              </p>
              {entry !== undefined && <RevisionMeta revision={entry.revision} />}
            </div>
            {snapshot.isPending ? (
              <BodySkeleton />
            ) : snapshot.isError ? (
              <EmptyState title="Could not load this revision" detail={errorText(snapshot.error)} />
            ) : doc === undefined ? (
              <EmptyState title="The doc did not exist at this revision" detail={name} />
            ) : (
              <DocSnapshotView project={project} name={name} doc={doc} at={entry?.revision.at} />
            )}
          </PanelBody>
        </>
      }
      aside={<DocHistory project={project} name={name} selected={revision} />}
    />
  )
}

// The revision names tasks and docs alone. The live doc has them resolved to titles, which is the
// best a chip can show.
function DocSnapshotView({
  project,
  name,
  doc,
  at,
}: {
  project: string
  name: string
  doc: DocSnapshot
  at: string | undefined
}) {
  const abbreviation = useAbbreviation(project)
  const live = useQueryClient().getQueryData<DocDetail>(docKey(project, name))
  const segments = useMemo(() => bodySegments(doc.body), [doc.body])
  return (
    <>
      <h1 className="mb-1.5 text-2xl font-semibold tracking-tight">{doc.title}</h1>
      <MetaLine className="mb-4 h-4">
        <TaskTimes created={docCreatedOf(doc.metadata)} updated={at} problems={docProblems(doc.metadata)} />
      </MetaLine>
      <TaskBodyWithConflicts
        segments={segments}
        project={project}
        refs={live?.refs}
        docRefs={live?.doc_refs}
        abbreviation={abbreviation}
        data-keys-ignore
      />
    </>
  )
}

function CurrentVersionLink({ project, name }: { project: string; name: string }) {
  const navigate = useNavigate()
  const { state } = useLocation()
  const back = (event: MouseEvent<HTMLAnchorElement>) => {
    if (!isOverLiveTask(state) || event.button !== 0) return
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
    event.preventDefault()
    navigate(-1)
  }
  return (
    <Link
      to={docPath(project, name)}
      replace
      onClick={back}
      className="text-muted-foreground hover:text-foreground ml-auto inline-flex shrink-0 items-center gap-1.5 text-xs"
    >
      <Undo2 className="size-3.5" />
      Current version
    </Link>
  )
}

// The parent sits at the right of the header and changes in place, as a task's parent does. A parent
// the store cannot resolve reads as missing, as a task's does, and names the doc it looked for.
function HeaderParent({ project, doc }: { project: string; doc: DocDetail }) {
  const navigate = useNavigate()
  const [editing, setEditing] = useState(false)
  const parent = docParentOf(doc.metadata)
  const parentTitle = doc.parent_title
  useDetailAction("edit-parent", () => setEditing(true))
  useDetailAction("go-parent", () => {
    if (parent !== undefined && parentTitle !== undefined) navigate(docPath(project, parent))
  })

  if (editing) {
    return <ParentPicker project={project} doc={doc} onClose={() => setEditing(false)} />
  }
  return (
    <div className="flex min-w-0 items-center gap-1">
      {parent !== undefined && parentTitle !== undefined ? (
        <MetaLine>
          <DocParentLink project={project} name={parent} title={parentTitle} />
        </MetaLine>
      ) : parent !== undefined ? (
        <Tooltip content={`No doc is named ${parent}`}>
          <span className="text-muted-foreground/70 text-xs italic">parent missing</span>
        </Tooltip>
      ) : null}
      <Button
        onClick={() => setEditing(true)}
        aria-label={parent === undefined ? "Set parent" : "Change parent"}
        className="gap-1 px-1.5"
      >
        {parent === undefined ? (
          <>
            <Plus className="size-3.5" />
            Set parent
          </>
        ) : (
          <Pencil className="size-3.5" />
        )}
      </Button>
    </div>
  )
}

// Opened on demand, so the project's doc list is fetched only when nesting. Nesting `doc` under one
// of its own descendants would close a cycle, which the store refuses, so the picker leaves those
// out along with the doc itself.
function ParentPicker({ project, doc, onClose }: { project: string; doc: DocDetail; onClose: () => void }) {
  const docs = useQuery({
    queryKey: allDocsKey(project),
    queryFn: abortable(listAllDocs([project])),
    refetchOnMount: "always",
  })
  const { mutate } = useProjectMutation(project)
  const all = docs.data ?? NO_DOCS
  const parent = docParentOf(doc.metadata)

  const buildOptions = useCallback(
    (query: string): ReadonlyArray<ComboOption> => {
      const excluded = descendants(all, doc.name)
      excluded.add(doc.name)
      if (parent !== undefined) excluded.add(parent)
      const options: ComboOption[] = []
      if (parent !== undefined) {
        options.push({
          key: " clear",
          content: (
            <span className="text-muted-foreground flex items-center gap-2">
              <X className="size-4" />
              Top level (no parent)
            </span>
          ),
          onSelect: () => mutate(patchDoc(project, doc.name, { parent: null })),
        })
      }
      for (const { doc: entry, indices } of docMatches(all, query, excluded)) {
        options.push({
          key: entry.name,
          content: <ComboDocRow doc={entry} indices={indices} />,
          onSelect: () => mutate(patchDoc(project, doc.name, { parent: entry.name })),
        })
      }
      return options
    },
    [all, project, doc.name, parent, mutate],
  )

  return (
    <Combobox
      placeholder="Change parent…"
      buildOptions={buildOptions}
      onClose={onClose}
      emptyLabel="No matching doc"
      className="w-72"
    />
  )
}

function ancestors(docs: ReadonlyArray<DocListItem>, name: string): Set<string> {
  const parents = new Map(docs.map((doc) => [doc.name, docParentOf(doc.metadata)]))
  const found = new Set<string>()
  let current = parents.get(name)
  while (current !== undefined && !found.has(current)) {
    found.add(current)
    current = parents.get(current)
  }
  return found
}

function descendants(docs: ReadonlyArray<DocListItem>, name: string): Set<string> {
  const found = new Set<string>()
  let frontier = [name]
  while (frontier.length > 0) {
    const next: Array<string> = []
    for (const doc of docs) {
      const named = docParentOf(doc.metadata)
      if (named === undefined || !frontier.includes(named) || found.has(doc.name)) continue
      found.add(doc.name)
      next.push(doc.name)
    }
    frontier = next
  }
  return found
}

// The docs nested directly under this one, plus an inline add box that either nests an existing doc
// here or writes a new one.
function NestedSection({
  project,
  doc,
  rows,
  cursor,
}: {
  project: string
  doc: DocDetail
  rows: ReadonlyArray<string>
  cursor: number
}) {
  const [adding, setAdding] = useState(false)
  const children = doc.children ?? []
  useDetailAction("add-subtask", () => setAdding(true))
  return (
    <Section
      title="Nested docs"
      count={children.length}
      action={
        <Button variant="accent" onClick={() => setAdding((open) => !open)}>
          <Plus className="size-3.5" />
          Add doc
        </Button>
      }
    >
      {adding && (
        <div className="mb-3">
          <NestedPicker project={project} doc={doc} onClose={() => setAdding(false)} />
        </div>
      )}
      {children.length === 0 ? (
        <p className="text-muted-foreground text-sm">No nested docs yet.</p>
      ) : (
        <ChildList rows={rows} cursor={cursor} entries={children} />
      )}
    </Section>
  )
}

function ChildList({
  entries,
  rows,
  cursor,
}: {
  entries: ReadonlyArray<DocChild>
  rows: ReadonlyArray<string>
  cursor: number
}) {
  const activeRow = useRef<HTMLLIElement>(null)
  useEffect(() => {
    activeRow.current?.scrollIntoView({ block: "nearest" })
  }, [cursor])

  return (
    <ul
      className="space-y-0.5"
      onMouseMove={() => {
        if (cursor !== -1) detailCursor.clear()
      }}
      onMouseLeave={hoveredRow.clear}
    >
      {entries.map((child, at) => (
        <li
          key={child.name}
          ref={at === cursor ? activeRow : undefined}
          aria-selected={at === cursor}
          onMouseMove={() => hoveredRow.enter(rows[at], at)}
          onMouseLeave={() => hoveredRow.leave(rows[at], at)}
        >
          <Row
            as={Link}
            variant="option"
            active={at === cursor}
            hoverable
            to={rows[at]}
            onClick={() => detailCursor.focus(at)}
            className="flex min-w-0 items-center gap-2"
          >
            <FileText className="text-muted-foreground size-4 shrink-0" />
            <span className="min-w-0 truncate">{child.title || child.name}</span>
          </Row>
        </li>
      ))}
    </ul>
  )
}

// Opened on demand, so the project's doc list is fetched only when adding. Nesting a doc under `doc`
// closes a cycle only when that doc is an ancestor of `doc`, so the picker leaves those out along
// with the doc itself and the docs already nested here.
function NestedPicker({ project, doc, onClose }: { project: string; doc: DocDetail; onClose: () => void }) {
  const docs = useQuery({
    queryKey: allDocsKey(project),
    queryFn: abortable(listAllDocs([project])),
    refetchOnMount: "always",
  })
  const { mutate } = useProjectMutation(project)
  const all = docs.data ?? NO_DOCS

  const buildOptions = useCallback(
    (query: string): ReadonlyArray<ComboOption> => {
      const excluded = ancestors(all, doc.name)
      excluded.add(doc.name)
      for (const child of doc.children ?? []) excluded.add(child.name)
      const options: ComboOption[] = []
      const title = query.trim()
      if (title !== "") {
        options.push({
          key: " create",
          content: (
            <span className="flex items-center gap-2">
              <Plus className="text-muted-foreground size-4" />
              <span>
                Create <span className="font-medium">“{title}”</span> as a new nested doc
              </span>
            </span>
          ),
          onSelect: () => mutate(createDoc(project, { name: title, parent: doc.name })),
        })
      }
      for (const { doc: entry, indices } of docMatches(all, query, excluded)) {
        options.push({
          key: entry.name,
          content: <ComboDocRow doc={entry} indices={indices} />,
          onSelect: () => mutate(patchDoc(project, entry.name, { parent: doc.name })),
        })
      }
      return options
    },
    [all, project, doc, mutate],
  )

  return (
    <Combobox
      placeholder="Find a doc or type a new doc title…"
      buildOptions={buildOptions}
      onClose={onClose}
      emptyLabel="Type a title to create a doc"
      className="max-w-md"
      inline
    />
  )
}

function ComboDocRow({ doc, indices }: { doc: DocListItem; indices: ReadonlyArray<number> }) {
  return (
    <span className="flex min-w-0 items-center gap-2">
      <FileText className="text-muted-foreground size-4 shrink-0" />
      <span className="truncate">
        <FuzzyText text={docLabel(doc)} indices={indices} />
      </span>
    </span>
  )
}

// The whole markdown below the title heading, replaced in one write. A doc is prose, so it is edited
// as prose rather than field by field.
// The delete moves the docs nested under this one, so the question says where they go.
function nestedNote(doc: DocDetail): string {
  const count = doc.children?.length ?? 0
  const place = doc.parent_title === undefined ? "to the top level" : `up under ${doc.parent_title}`
  if (count === 0) return ""
  return count === 1 ? ` Its nested doc moves ${place}.` : ` Its ${count} nested docs move ${place}.`
}

function DeleteControl({ project, doc }: { project: string; doc: DocDetail }) {
  const [confirming, setConfirming] = useState(false)
  const mutation = useProjectMutation(project)
  const navigate = useNavigate()
  if (!confirming) {
    return (
      <Button
        variant="danger"
        aria-label={`Delete ${doc.title || doc.name}`}
        onClick={() => setConfirming(true)}
        className="text-danger/70 hover:text-danger"
      >
        <Trash2 className="size-3.5" />
      </Button>
    )
  }
  return (
    <>
      <span className="text-muted-foreground text-xs">
        Delete {doc.title || doc.name}?{nestedNote(doc)}
      </span>
      <Button
        variant="danger"
        disabled={mutation.isPending}
        onClick={() =>
          mutation.mutate(deleteDoc(project, doc.name), {
            onSuccess: () => navigate(docsPath(project)),
          })
        }
        className="text-danger disabled:opacity-40"
      >
        <Check className="size-3.5" />
        Delete
      </Button>
      <Button onClick={() => setConfirming(false)}>
        <X className="size-3.5" />
        Cancel
      </Button>
    </>
  )
}

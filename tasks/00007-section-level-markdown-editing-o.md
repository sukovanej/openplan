---
status: backlog
created: 2026-07-14T10:31:28Z
---
# Section-level markdown editing of task bodies (engine → CLI → API → web editor)

## Goal
Ship the full vertical slice for editing a task's markdown **body**: a section-addressable
splice **engine**, its **CLI** surface, an **op-server** mutation **endpoint**, and a
**raw-markdown + live-preview inline editor** in the web UI. A human edits sections in the
browser while agents edit the same store; edits **splice source byte ranges** so files
stay pristine and diffs stay minimal. Builds directly on the read-only viewer from
[[./00005-bootstrap-the-realtime-web-ui-re.md]] and the whole-file/metadata writes from
[[./00003-task-crud-across-the-store-daemo.md]] and [[./00004-support-setting-task-content-on-c.md]].

Scope is **body content only** — frontmatter (`status`/`parent`/`deps`) already has `oplan set`
and stays out.

## Decisions (locked)
- **Vertical slice**: engine + CLI + HTTP mutation endpoint + web editor, one coherent slice.
- **Section-level splice**: compute the target node's source byte range and splice new
  text in; the rest of the file is byte-for-byte untouched. No reparse-and-reserialize.
- **Operations in v1**: edit an existing section, append a new `##` section, edit the `# H1`
  title, and delete / reorder sections.
- **Editor UX**: raw markdown source (CodeMirror) + live preview, reusing the existing
  `react-markdown` + `remark-gfm` renderer for the preview pane.
- **Concurrency**: optimistic, **per-section** base-version guard. A save carries the base it
  edited; the server rejects only if *that section* changed underneath — a concurrent edit to a
  different section of the same file does not block the save.
- **Live collision**: if the section being edited changes on disk while the buffer is dirty,
  keep the buffer, show a non-destructive "changed underneath" flag, and reconcile at save time;
  untouched sections still live-update in the view. Never silently discard in-flight typing.
- **Section handle**: deduped heading-slug path + a content hash of the section as loaded.
  The server re-derives the byte range from the *current* file and checks the hash for the guard
  — no persisted anchors, robust to shifts above the section ("list → act").

## Parser spike
Before building the engine, spike section-splice against all three candidates and pick one:
**pulldown-cmark `OffsetIter`** (splice-friendly byte ranges), **comrak**
(`sourcepos`), **markdown-rs** (mdast + positions). Judge on: fidelity of untouched-region
bytes after a splice, correct heading→next-heading span (a section includes nested subsections),
GFM coverage (tables, `- [ ]` checklists), and speed. Record the choice and why.

## Splice engine (shared core crate)
One engine, called identically by the CLI, the HTTP endpoint, and (where relevant) the
[[merge-driver]]. Capabilities:
- **Address** a section by deduped heading-slug path (`Section`, `Section.Sub`) → source
  byte range (heading through the byte before the next same-or-higher heading).
- **Overwrite** a section: splice new text into its byte range; surrounding bytes unchanged.
- **Append** a new `##` section at end of body.
- **Delete / reorder**: cut a section's byte block and (for reorder) reinsert it elsewhere.
  Reorder/delete inherently touch two regions, so the guarantee is *"untouched text is never
  reflowed or reprinted,"* not "a single hunk changes."
- **Title**: read/overwrite the single `# H1`.
- **List handles**: enumerate addressable sections with their slug paths + content hashes (feeds
  the guard and the editor).
- **Invariants enforced on every write**: exactly one `# H1` survives; sections start at
  `##`; the result reparses; atomic temp-write + rename under the per-file advisory lock.
- **Guard primitive**: given (slug path, base hash), resolve the current range and reject with a
  typed "section changed" error if the hash no longer matches.
- **Edge case**: preamble body between the `# H1` and the first `##` — define its addressing
  (implicit lead region) or explicitly reject editing it in v1; do not corrupt it on other edits.

## CLI surface
Wire the engine into `oplan`, JSON-first for agents:
```
oplan sections <id> [--json]                 # addressable targets: slug path + hash
oplan get      <id> -t 'Section.Sub' [--json]
oplan set      <id> -t 'Section' --body-file f|-   # overwrite one section (splice)
oplan append   <id> --body-file f|-               # new ## section
oplan set      <id> --title '<text>'              # overwrite the H1
oplan delete   <id> -t 'Section'                  # remove a section
oplan move     <id> -t 'Section' --before|--after 'Other'   # reorder
```
Section writes take an optional `--base-hash` so headless agents can opt into the same guard.

## Mutation API (op-server)
Add body-mutation endpoints (the first writes op-server exposes — the bootstrap shipped reads
only). Requests carry `{ slug path, new text, base hash }`; the handler calls the engine, returns
the new section handle + hash, and returns a typed **409-style conflict** when the guard rejects.
Writes target the **server's own worktree/branch** only (reads global, writes local);
multi-worktree write targeting is out of scope here. A successful write flows to all clients via
the existing realtime stream.

## Web editor (`web/packages/app`)
- Per-section inline editor in the task detail route: click a section → CodeMirror raw-markdown
  pane + live preview (reuse the existing renderer). Save via the mutation API with the loaded
  base hash.
- **Append**: an "add section" affordance. **Title**: inline-editable H1. **Delete / reorder**:
  section controls (a drag handle for order; reorder sends `move`).
- **Guard UX**: on a 409, surface the "changed underneath" flag and let the human reload that
  section or retry; the realtime stream marks a dirty section stale in place without clobbering
  the buffer.
- Effect layer: mutations are `Effect` services/atoms consistent with the bootstrap's
  data-layer pattern; realtime drives invalidation (no `useEffect` soup).

## Out of scope
- Block-level (sub-section) addressing — open decision #6, stays deferred.
- Frontmatter editing in the UI (already `oplan set`).
- Cross-worktree write targeting and the merge/conflict *resolution* view (the driver exists
  separately; this task only adds the per-section optimistic guard for live editing).
- WYSIWYG editing.

## Definition of done
- Parser choice recorded with rationale.
- Engine + CLI + API + editor land the four operations end-to-end; a human edits a section in
  the browser and the file on disk changes with a **minimal, pristine diff**.
- Concurrent-edit guard proven: a stale save is rejected per-section, a non-overlapping
  concurrent save succeeds.
- Tests in each crate's `tests/` dir (never in `src/`): engine splice fidelity (untouched bytes
  byte-for-byte, H1 invariant, deduped slugs, span-includes-subsections, guard reject/accept),
  API conflict path, web decode + save/invalidation reducers.
- `cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings` clean; web
  `pnpm -r` build/typecheck/lint/test clean.

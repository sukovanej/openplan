---
status: backlog
created: 2026-07-14T22:12:23Z
---
# Store and embed assets (images, artifacts, files) in tasks

Store binary assets (images, Claude Code design artifacts, PDFs, arbitrary
files) in the `.plan/` store and embed them in task bodies, rendered inline in
the web UI. Assets follow the same principles as tasks: plain-markdown
references, content-addressed, committed to git, branch-scoped, files as source
of truth.

## Model

- **Storage**: `.plan/assets/<hash>.<ext>`, committed to git alongside tasks.
  Content-addressed by hash of the bytes; `<ext>` derived from the original
  filename. Adding identical bytes resolves to the same file (natural dedup,
  no-op). No garbage collection — git history is permanent regardless, and an
  orphaned working-tree file is harmless.
- **Reference**: standard markdown in the task body, using a real relative path
  so it renders everywhere (GitHub, editor previews, the web UI):
  - image → `![login mockup](../assets/9f2a1c.png)`
  - other file → `[report.pdf](../assets/8b3f2d.pdf)`
- **Metadata is derived, not stored**: the original filename is the embed's
  alt / link text; mime and size are sniffed from the blob at render time. No
  sidecar manifest, no new frontmatter — the reference lives entirely in the
  addressable markdown body.
- **Branch-scoped, reads-global / writes-local**: an asset is readable
  from another branch only once its blob is committed; the daemon serves it from
  the object DB by blob at that branch. In the active worktree it renders
  immediately from the uncommitted working-tree file.

## Ingestion

Two entry points for v1, both write into the currently selected worktree and
insert the markdown embed:

- `openplan asset add <file> --task <id> [--section <name>]` — copies the file into
  `.plan/assets/`, computes the hash, appends/inserts the embed into the task
  body (at `--section` if given, else the body end). The path agents and scripts
  use.
- **Web UI paste / drag-drop** — paste a screenshot or drop a file onto a task;
  the UI writes the blob into the selected worktree and inserts the embed.

**Commit timing**: the blob is written to the working tree and folded into the
next commit like any `.md` edit — no auto-commit. This matches how task edits
flow today and keeps `openplan asset add` from injecting surprise commits.

**Size guardrail**: warn and require `--force` (UI: an explicit confirm) above a
soft cap (~5MB); hard-refuse above a large ceiling (~100MB). Keeps the repo sane
without blocking legitimate large design artifacts.

## Web UI rendering

- The markdown renderer rewrites relative `../assets/<hash>.<ext>` URLs to an
  asset-serving HTTP endpoint on the daemon.
- The endpoint serves bytes from the active worktree's working tree when that
  branch is checked out, else from the git object DB by blob for cross-branch
  views (mirrors the matrix's blob-OID model).
- **Images** render inline; **other types** render as a download chip showing
  the filename (from link text) and size (sniffed).

## Out of scope (v1)

- Standalone doc primitive as an asset home (still deferred) — assets
  attach to tasks only.
- `openplan asset gc` / orphan pruning.
- git-lfs.

## Open / to confirm during the spike

- Hash function and truncation length for the on-disk name.
- Exact soft/hard size thresholds.
- Whether the section-aware merge driver needs any awareness of assets
  (embeds are ordinary markdown, so likely not).

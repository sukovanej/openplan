---
status: backlog
---
# Tags: registered labels (name · color · description) assignable to tasks

## Goal
Introduce **tags** as a first-class, *registered* vocabulary that tasks reference. A tag is its
own file in `.plan/tags/`, identified by its normalized name, carrying a palette color and an
optional description. Tasks reference tags by name in a new `tags:` frontmatter field. Full-stack:
`op-store` data model, the `oplan tag …` CLI, daemon HTTP, and web-UI chips + tag management.
**Filtering by tag is explicitly out of scope** (a follow-up).

## Decisions (locked)
- **Registered, not free-form.** A tag must exist before a task can reference it (like `--dep`
  requires an existing id). It is a controlled vocabulary, not ad-hoc strings.
- **Identity = the name.** A tag is identified by its normalized name, which is its filename. Task
  frontmatter reads `tags: [backend, wip]` — human-readable. Two branches that both create
  `backend` converge on the same file (add/add), not two rival ids.
- **Color = fixed palette.** A closed set of ~12 named colors (theme-aware in the UI); free hex is
  rejected. Names, not hexes, cross the wire.
- **Consequences we accept:** names are globally unique; `rename` rewrites references but only on
  the current branch (writes are branch-local, §7.1), so cross-branch refs to the old name are left
  dangling; readers render dangling tag refs gracefully (name only, muted) rather than erroring.

## Data model
- **Tag file** `.plan/tags/<name>.md`, reusing the task file format:
  - filename = normalized name = identity (`[a-z0-9][a-z0-9-]*`; lowercased, kebab; validated).
  - single `# H1` = display name (may differ in case, e.g. `# Backend`).
  - frontmatter `color: <palette-name>` (required; defaulted deterministically from the name when
    omitted — hash → palette index, no RNG).
  - body below the H1 = optional description (free markdown).
- **Task frontmatter** gains `tags: [<name>, …]` — an **unordered set**: sorted + deduped on write,
  **omitted when empty** (mirrors `deps`). Roundtrip fidelity preserved; writing tags must not
  reflow the body.
- **Palette** has a single Rust source of truth (e.g. `op-task::Palette`) enumerating the color
  names; the web package mirrors the same names → theme-aware CSS tokens (light + dark).

## Referential integrity (reads global, writes local — §7.1)
- Assigning a tag validates it exists in the current worktree's registry; unknown → non-zero exit
  with a hint to `oplan tag create`.
- `tag delete` refuses when tasks on the current branch reference it, unless `--force`; it **cannot**
  see or clean references on other branches — those become dangling by design.
- Readers never hard-fail on an unknown tag name: a task may reference a tag not present on this
  branch (created elsewhere, or since deleted). Resolve name → {color, desc} best-effort; fall back
  to a neutral chip showing the raw name.

## CLI surface (this task)
```
oplan tag create "<name>" [--color <c>] [--desc <text>]   # register a tag; prints its name
oplan tag list [--json]                                    # registry: name · color · (desc)
oplan tag show <name> [--json]
oplan tag set  <name> color <c>                            # recolor (validated against palette)
oplan tag rename <name> <new-name>                         # rename file + rewrite refs on THIS branch
oplan tag delete <name> [--force] [--yes]                  # refuse if referenced unless --force
oplan tag colors                                           # print the palette names

# assignment
oplan create "<title>" [--tag <name> ...]                  # each validated against the registry
oplan set <id> tags "<a>, <b>"                             # replace the whole set (empty clears)
oplan tag add    <task-id> <name> ...                      # add to a task's set
oplan tag remove <task-id> <name> ...                      # remove from a task's set
```
- `--tag` / `set tags` / `tag add` validate every name exists; reject with a clear message otherwise.

## Daemon / HTTP (this task)
```
GET    /api/tags            # list TagView
POST   /api/tags            # create → { name }
GET    /api/tags/:name
PATCH  /api/tags/:name      # color / description / rename
DELETE /api/tags/:name      # ?force= overrides the reference check
```
- Extend task DTOs (`TaskView`, `TaskSummary`, `CreateTask`, `TaskPatch`) with `tags: [name]`.
- `TagView { name, display, color, description }`. Map store errors → 404 / 400 (bad color) /
  409 (referenced delete) / 500.
- The existing watcher already fires on `.plan/tags/` writes; a `TagChanged` log line is enough
  here (no index wiring).

## Web UI (this task)
- **Chips on task rows:** each tag a palette-colored chip (name), theme-aware in light + dark; a
  dangling ref renders as a neutral/muted chip.
- **Assign / unassign on a task:** add and remove chips, driven by the task PATCH route.
- **Tag-management surface:** create / recolor / rename / delete against `/api/tags`, with a palette
  picker (the fixed named set).
- **No filtering.** Build chips so a future filter task can hang a click handler on them, but add no
  filter control now.

## Crate changes
- **op-task:** `Tag` / `Palette` types; name normalization + validation; add `tags: Vec<String>` to
  the task model (sorted-set semantics, omit-when-empty, roundtrip-safe).
- **op-store:** `tags_dir`; `create_tag` / `read_tag` / `write_tag` / `rename_tag` / `delete_tag` /
  `list_tags` / `tag_exists`, each per-file-locked + atomic. Task-side: validate tags on assignment;
  `rename_tag` rewrites refs across the branch's task files (each under its own lock); reference scan
  for delete. New `StoreError` variants: `TagNotFound`, `TagExists`, `TagReferenced { count }`,
  `InvalidColor`.
- **op-api:** `TagView`, `CreateTag`, `TagPatch`; extend the task DTOs with `tags`.
- **op-server:** the five `/api/tags` routes + task DTO plumbing; error → HTTP mapping incl. 409.
- **op-cli:** the `tag` subcommand group + `--tag` on `create` + `set … tags` + `tag add/remove`.
- **web:** palette token map (light/dark), chip component, task-row rendering, assign UI, and the
  tag-management surface.

## Acceptance criteria
- [ ] `oplan tag create "Backend"` writes `.plan/tags/backend.md` (`# Backend`, `color:` set) and
      prints `backend`; a second create of the same name is rejected non-zero.
- [ ] `oplan tag create "X" --color notacolor` is rejected (palette validation); `oplan tag colors`
      lists the valid names.
- [ ] `oplan create "Wire parser" --tag backend --tag wip` succeeds only if both tags exist; an
      unknown tag is rejected non-zero with a `tag create` hint. The task frontmatter shows
      `tags: [backend, wip]` (sorted, deduped) and the body is byte-for-byte unchanged.
- [ ] `oplan tag add/remove <id> …` and `oplan set <id> tags "…"` mutate only frontmatter; an empty
      string clears the set and omits the field.
- [ ] `oplan tag rename backend infra` renames the file and rewrites `tags:` in every referencing
      task on this branch; `oplan tag delete infra` then refuses (referenced) without `--force` and
      succeeds with it.
- [ ] A task referencing a non-existent tag name still reads/lists cleanly (no error); the daemon
      and UI render it as a neutral chip.
- [ ] Daemon: `POST/GET/PATCH/DELETE /api/tags[/:name]` round-trip; referenced delete → 409; bad
      color → 400; task routes carry `tags`. Covered by `tower::ServiceExt::oneshot` tests.
- [ ] `op-store` tests: tag CRUD roundtrip; assign validates existence; rename rewrites refs; delete
      honors the reference check; concurrent writes to the same tag file serialize.
- [ ] Web: chips render with palette colors in light + dark; a dangling ref renders muted;
      create / rename / recolor / delete work end-to-end. Web checks (lint + vitest + build) pass.
- [ ] `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings` all pass.

## Out of scope (follow-ups)
- **Filtering:** `oplan list --tag …` and any UI filter control (explicitly deferred).
- **Merge-driver set-union for `tags:`** — the section-aware merge driver (§7.7) must treat `tags`
  as an unordered set so two branches adding different tags auto-merge; that lands with the
  merge-driver task. Model the field as a set now so it is ready.
- **Cross-branch tag dedup / "merge duplicate tags"** tooling, and cross-branch rename propagation
  (impossible under §7.1; left dangling by design).
- Bulk retag, tag groups / namespaces, per-tag task counts in the registry view.

## Notes
- Keep writes minimal-diff: assigning tags re-serializes only the frontmatter block, never the body
  (same discipline as `set status`).
- A tag file reuses the task markdown format but lives in `.plan/tags/`, not `.plan/tasks/`; the two
  never mix. This is the SPEC's first primitive beyond the task since docs were deferred — keep it
  as thin as possible.

---
status: done
created: 2026-07-14T11:16:24Z
---
# Tags: registered labels (name · color · description) assignable to tasks

## Goal
Introduce **tags** as a first-class, *registered* vocabulary that tasks reference. A tag is its
own file in `.plan/tags/`, identified by its normalized name, carrying a palette color and an
optional description. Tasks reference tags by name in a new `tags:` frontmatter field. Full-stack:
`op-store` data model, the `openplan tag …` CLI, daemon HTTP, and web-UI chips + tag management.
**Filtering by tag is explicitly out of scope** (a follow-up).

## Decisions (locked)
- **Registered, not free-form.** A tag must exist before a task can reference it (like `--dep`
  requires an existing id). It is a controlled vocabulary, not ad-hoc strings.
- **Identity = the name.** A tag is identified by its normalized name, which is its filename. Task
  frontmatter reads `tags: [backend, wip]` — human-readable. Two branches that both create
  `backend` converge on the same file (add/add), not two rival ids.
- **Color = fixed palette.** A closed set of ~12 named colors (theme-aware in the UI); free hex is
  rejected. Names, not hexes, cross the wire.
- **Strict whole-set validation.** Every name in an incoming set — `--tag`, `set … tags`, task
  `PATCH` — must exist in the current worktree's registry, including names the task already
  carried. Editing the tags of a task holding a dangling ref means dropping that ref in the same
  write; dangling refs are scrubbed on touch, never silently carried forward.
- **Consequences we accept:** names are globally unique; `rename` rewrites references but only on
  the current branch (writes are branch-local), so cross-branch refs to the old name are left
  dangling; readers render dangling tag refs gracefully (name only, muted) rather than erroring.
  Two branches creating the same name with different color/description converge in identity but
  still content-conflict at merge — a human resolves a two-line file (the section-aware merge
  driver may learn tags later).

## Data model
- **Tag file** `.plan/tags/<name>.md`, reusing the task file format:
  - filename = normalized name = identity. Normalization transforms case and separators only:
    lowercase; spaces/underscores → hyphens; collapse runs. If the result still fails
    `[a-z0-9][a-z0-9-]*`, the name is rejected with the rule in the message — `"Front End"` →
    `front-end`, but `"C++"` is refused, never slugified into something surprising.
  - single `# H1` = display name (may differ in case, e.g. `# Backend`).
  - frontmatter `color: <palette-name>`. `tag create` always materializes it — the given color, or
    one derived deterministically from the name (hash → palette index, no RNG). Readers apply the
    same derivation when a hand-made file omits the field; `op-lint` flags the omission.
  - body below the H1 = optional description (free markdown).
- **Task frontmatter** gains `tags: [<name>, …]` — an **unordered set**: sorted + deduped on write,
  **omitted when empty** (mirrors `deps`). Roundtrip fidelity preserved; writing tags must not
  reflow the body.
- **Palette** has a single Rust source of truth (e.g. `op-task::Palette`), surfaced in `op-api` as
  a closed utoipa enum so the names reach the web through `generate-web-client`; the web adds only
  the name → theme-aware CSS token map (light + dark), never its own name list.

## Referential integrity (reads global, writes local)
- Assignment is strict whole-set: every name in the written set must exist in the current
  worktree's registry; unknown → non-zero exit with a hint to `openplan tag create`. A dangling ref
  the task already carries fails the write too — drop it in the same write.
- `tag delete` refuses when tasks on the current branch reference it, unless `--force`; it **cannot**
  see or clean references on other branches — those become dangling by design.
- `tag rename` onto an existing name refuses (`TagExists`); merging two tags is follow-up tooling.
- Readers never hard-fail on an unknown tag name: a task may reference a tag not present on this
  branch (created elsewhere, or since deleted). Resolve name → {color, desc} best-effort; fall back
  to a neutral chip showing the raw name. The daemon resolves against the served worktree's
  `.plan/tags/` — tasks from other branches resolve best-effort against that same registry.

## CLI surface (this task)
```
openplan tag create "<name>" [--color <c>] [--desc <text>]   # register a tag; prints its name
openplan tag list [--json]                                    # registry: name · color · (desc)
openplan tag show <name> [--json]
openplan tag set  <name> color <c>                            # recolor (validated against palette)
openplan tag set  <name> desc <text>                          # re-describe (parity with PATCH)
openplan tag rename <name> <new-name>                         # rename file + rewrite refs on THIS branch
openplan tag delete <name> [--force] [--yes]                  # refuse if referenced unless --force
openplan tag colors                                           # print the palette names

# assignment — replace-only for v1; the tag namespace stays purely registry ops
openplan create "<title>" [--tag <name> ...]                  # each validated against the registry
openplan set <id> tags "<a>, <b>"                             # replace the whole set (empty clears)
```
- `--tag` / `set tags` validate every name in the set exists; reject with a clear message otherwise.

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
- The watcher does **not** see `.plan/tags/` today: worktree watches cover only `.plan/tasks`, the
  store's `.plan` is watched non-recursively, and `is_relevant` filters tags paths out. Extend
  `op-watch`: watch each worktree's `.plan/tags` recursively, accept tags paths in `is_relevant`,
  carry the tag files' blob oids in the snapshot, and emit one coarse `Change::Tags` when they
  differ — no per-tag diffing. The daemon bridges it to a new `ChangeEvent::TagsChanged` on
  `/api/events` (UIs refetch `GET /api/tags`); the HTTP tag handlers publish `TagsChanged`
  directly on their own writes, like the task routes do.

## Web UI (this task)
- **Chips on task rows:** each tag a palette-colored chip (name), theme-aware in light + dark.
- **Dangling refs:** a gray chip with a warning icon and a tooltip explaining the problem (the tag
  doesn't exist on this branch — created elsewhere, deleted, or renamed) and that any tags edit
  drops it. Its only affordance is remove. Because validation is strict whole-set, the UI prunes
  dangling names from every tags PATCH it sends — the tooltip is what makes that drop non-silent.
- **Assign / unassign on a task:** add and remove chips, driven by the task PATCH route — the same
  uncommitted live-worktree write path status changes use today (no dependency on rolling-updates;
  when [[./00109-rolling-updates-an-ambient-edit-b.md]] reroutes ambient writes, tag writes ride
  along).
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
- **op-api:** `TagView`, `CreateTag`, `TagPatch`, the closed `Color` enum,
  `ChangeEvent::TagsChanged`; extend the task DTOs with `tags`.
- **op-watch:** watch `.plan/tags` per worktree, tags paths in `is_relevant`, tag blobs in the
  snapshot, coarse `Change::Tags`.
- **op-server:** the five `/api/tags` routes + task DTO plumbing; error → HTTP mapping incl. 409;
  bridge `Change::Tags` → `TagsChanged` and publish it from the tag handlers.
- **op-cli:** the `tag` subcommand group + `--tag` on `create` + `set … tags`.
- **web:** palette token map (light/dark), chip component, task-row rendering, assign UI, and the
  tag-management surface.

## Acceptance criteria
- [ ] `openplan tag create "Backend"` writes `.plan/tags/backend.md` (`# Backend`, `color:` always
      materialized) and prints `backend`; `"Front End"` normalizes to `front-end`; `"C++"` is
      rejected with the normalization rule; a second create of an existing name is rejected
      non-zero.
- [ ] `openplan tag create "X" --color notacolor` is rejected (palette validation); `openplan tag colors`
      lists the valid names.
- [ ] `openplan create "Wire parser" --tag backend --tag wip` succeeds only if both tags exist; an
      unknown tag is rejected non-zero with a `tag create` hint. The task frontmatter shows
      `tags: [backend, wip]` (sorted, deduped) and the body is byte-for-byte unchanged.
- [ ] `openplan set <id> tags "…"` mutates only frontmatter; an empty string clears the set and omits
      the field. A set containing a name missing from this branch's registry is rejected even when
      the task already carried it (strict whole-set); resubmitting without it succeeds.
- [ ] `openplan tag rename backend infra` renames the file and rewrites `tags:` in every referencing
      task on this branch; renaming onto an existing name is refused; `openplan tag delete infra` then
      refuses (referenced) without `--force` and succeeds with it.
- [ ] A task referencing a non-existent tag name still reads/lists cleanly (no error); the daemon
      and UI render it as a neutral chip.
- [ ] Daemon: `POST/GET/PATCH/DELETE /api/tags[/:name]` round-trip; referenced delete → 409; bad
      color → 400; task routes carry `tags`. Covered by `tower::ServiceExt::oneshot` tests.
- [ ] A write to a watched worktree's `.plan/tags/` (a CLI `tag create` / recolor while the daemon
      runs) emits `TagsChanged` on `/api/events`; the HTTP tag routes emit it on their own writes.
      Covered by an `op-watch` test and a server test.
- [ ] `op-store` tests: tag CRUD roundtrip; assign validates existence; rename rewrites refs; delete
      honors the reference check; concurrent writes to the same tag file serialize.
- [ ] Web: chips render with palette colors in light + dark; a dangling ref renders muted;
      create / rename / recolor / delete work end-to-end. Web checks (lint + vitest + build) pass.
- [ ] `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings` all pass.

## Out of scope (follow-ups)
- **Filtering:** `openplan list --tag …` and any UI filter control (explicitly deferred).
- **Merge-driver set-union for `tags:`** — the section-aware merge driver must treat `tags`
  as an unordered set so two branches adding different tags auto-merge; that lands with the
  merge-driver task. Model the field as a set now so it is ready.
- **Cross-branch tag dedup / "merge duplicate tags"** tooling, and cross-branch rename propagation
  (impossible under writes-local; left dangling by design).
- **Delta assignment verbs** (`tag add/remove` or `set … tags +a -b`) — replace-only for v1; add
  them if the CLI ergonomics hurt in practice.
- **Per-branch tag resolution** in the daemon — resolving another branch's refs against that
  branch's committed `.plan/tags/` tree instead of the served worktree's registry.
- Bulk retag, tag groups / namespaces, per-tag task counts in the registry view.

## Notes
- Keep writes minimal-diff: assigning tags re-serializes only the frontmatter block, never the body
  (same discipline as `set status`).
- A tag file reuses the task markdown format but lives in `.plan/tags/`, not `.plan/tasks/`; the two
  never mix. This is the first primitive beyond the task since docs were deferred — keep it
  as thin as possible.

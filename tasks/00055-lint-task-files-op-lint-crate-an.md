---
status: done
created: 2026-07-30T10:34:02Z
---
# Lint task files: op-lint crate and the oplan lint command

A new `op-lint` crate plus `oplan lint`, run in CI and locally alike — agents,
humans, git hooks. It checks what the write path deliberately cannot:
`Store::validate` rejects only refs a write **newly introduces**, so anything
arriving by hand edit, by merge, or by a delete that orphans someone else's ref
goes unseen.

Gaps it closes today: dependency cycles are never checked; a body `[[…]]` naming
no task is silently rewritten to a key; two files claiming one number resolve to
the lower path in silence; a `#Section` ref is never matched against the target's
headings.

## Shape

`crates/op-lint` — depends on `op-task`, `op-md`, `op-store`, `op-git`. Never
contacts or starts a daemon, in CI or locally.

- **Snapshot** — every `.plan/tasks/*.md` read once through
  `op_task::parse_partial` (lenient: one broken file must not hide the rest),
  plus an index of paths and headings that the reference rules resolve against.
  Standalone docs (§12) plug into that index later; it is the only seam built
  ahead of need.
- **Rules** — `const TASK_RULES: &[TaskRule]` (per file,
  `fn(&Snapshot, &TaskFile, &mut Sink)`) and `const STORE_RULES: &[StoreRule]`
  (whole graph). A new check is one file and one line. No I/O inside a rule; the
  runner materializes everything first.
- **Diagnostic** — `{ code, path, span, message, help, fixable }`. `--json`
  carries `"severity": "error"` from day one, so agents parsing it keep working
  when a warning tier lands.

## Rules

- Frontmatter: fence present, YAML parses, `status` in the enum, `created` a real
  RFC3339 instant, `parent` a ref and not a section ref, `dependencies` a sequence
  of refs, `rank` base-36.
- References resolve — frontmatter `parent`/`dependencies`, body `[[…]]`, and
  markdown link destinations **including links into source** (`../../crates/…`).
  An `#anchor` matches by **GitHub slug** (lowercase, spaces → `-`, punctuation
  dropped, duplicates `-1`, `-2`): the scheme GitHub, GitLab, and VS Code all
  resolve, so our own links stay clickable outside oplan. `[[42]]` and `[[WEB-7]]`
  are reported, never rewritten — §3.1 refuses those spellings.
- Exactly one non-empty `# ` title.
- Parent cycles and dependency cycles.
- Two files claiming one number.

Every rule is an error; the bar for adding one is "would I block a commit on
this". Slug drift, unknown frontmatter keys, and a non-numbered `.md` in `tasks/`
are warning-tier and so are not rules yet.

## `--fix`

Only where the right output is **derivable**, never guessed:

- **Reference canonicalization** — rewrite each ref to the spelling
  `Store::in_file_form` already produces: the target's current file path. Covers
  `parent: 42`, a missing `./`, a stale slug left by a renamed target, and
  `[[OPP-42]]` in prose. (`parent: OPP-42` is not canonicalizable — the parser
  rejects the key form outright, so that is an invalid field.)
- **`created` backfill** from the first commit that added the file — already the
  remedy `StoreError::MissingCreated` prints. An uncommitted file has no answer
  and stays unfixable.

Fixes are textual splices (`Range<usize>` + replacement) applied by the runner to
a fixpoint, never `Task::to_file_string` — reserializing reflows the whole
frontmatter, the diff blowup §5.3 exists to prevent. They are written under
`Store`'s advisory lock + atomic rename with no daemon: `--fix` allocates no id
and resolves no branch, so it is an out-of-band writer of the kind §6 already
sanctions. SPEC §7.3 gains a line saying so.

Dangling refs, an invalid `status`, and duplicate H1s stay report-only — each has
more than one valid repair, and picking one is a guess.

## CLI

`oplan lint [<path>|<key>…] [--json] [--fix]`, non-zero exit on any diagnostic.
The snapshot is always the whole store — graph rules are wrong on a subset — and
the arguments filter the **output**, so an agent can lint the one task it just
wrote and a pre-commit hook does not fail on breakage in files the commit never
touched. Hooks check; they never fix.

## Prerequisites

- `Store::replace_raw(id, bytes)`, locked and atomic — `with_lock` is already
  public, `atomic_replace` is not.
- `PartialTask` carries its `serde_yaml::Mapping`, so lint can report `path:line`
  and name which `dependencies` entry is bad instead of losing the whole list to
  one bad element.
- `op-git`: the first commit to touch a path.

## Verify

Tests in `crates/op-lint/tests/` over in-memory snapshots: one fixture per rule,
plus a file with unreadable frontmatter that still yields every other file's
diagnostics. `--fix` round-trips — fix → re-lint is clean, fix → fix is
byte-identical. A test that lints this repo's own `.plan/` guards against
regressions; `mise run lint` and CI run the binary.

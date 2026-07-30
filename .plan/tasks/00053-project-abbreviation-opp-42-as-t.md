---
status: done
created: 2026-07-29T16:51:55Z
---
# Project abbreviation: OPP-42 as the task key

Linear-style project keys: `OPP-42` replaces `42` as the id everywhere above the
store. The number keeps allocating tasks and naming files; it stops being the
thing anyone types or reads.

This **contradicts SPEC §3.1** ("The number is the whole id … exactly one string
is the id: `42` is one") — rewriting that paragraph is part of the task, not a
follow-up.

## The model

- **Number = allocation unit + file name.** The daemon's allocator (§7.3), the
  `00042-` prefix, and `./00042-ship-login-page.md` references are untouched.
- **Key = `<ABBR>-<number>` = the id.** It is what the API, the CLI, the URLs,
  and the UI carry. `id` in every payload becomes `"OPP-42"`.
- **Disk and wire deliberately diverge.** A task file stays numeric so a plain
  markdown reader, an editor, and `grep` still follow a reference without oplan
  (§3.1's actual point); the key is a boundary rendering of the same number.

## `.plan/config.toml`

The store's first config file.

```toml
abbreviation = "OPP"
```

- Format `^[A-Z]{3}$` — exactly three uppercase ASCII letters. Hand-edited; no
  `oplan init`, no `config set` (see Non-goals).
- **Required. Missing or invalid is a hard stop**: the daemon exits at startup
  and every CLI command exits non-zero with `error: .plan/config.toml:
  'abbreviation' required` (or `: must be exactly three uppercase letters`).
  This is not the per-field "a read never fails" model of §3.1 — that governs a
  *task's* fields; a store with no abbreviation has no id space at all, so there
  is nothing to degrade into.
- **One store, one abbreviation.** Read from the worktree the daemon serves; a
  different value on another branch's copy of the file is ignored, so one task
  renders as one key across the whole cross-branch matrix. Consistent with
  §7.10 (one repository per daemon).
- The daemon watches the file (op-watch already watches `.plan/`) and applies a
  valid change live — every key re-renders. A change that leaves it missing or
  invalid stops the daemon exactly as startup does, so "no store operates
  without an abbreviation" holds at all times, including after a checkout that
  removes the file.
- Reader belongs in `op-store` (it owns `.plan/`), beside the task CRUD.

## Key parsing — one spelling, no leniency

`format_key` / `parse_key` in `op-task`, next to the existing `parse_id`
(`crates/op-task/src/lib.rs:95`), which keeps its job at the number/file layer.

| input | result |
|---|---|
| `OPP-42` | 42 |
| `42` | refused |
| `opp-42` | refused |
| `OPP-042` | refused |
| `WEB-7` | refused (wrong abbreviation) |
| `OPP-42x`, `OPP42`, `OPP-` | refused |

`id_cmp` (`crates/op-api/src/lib.rs:530`) strips the prefix and keeps comparing
numerically — the prefix is constant within a store, so ordering is unchanged.

## Surfaces to convert

- **HTTP** — path params become keys (`GET /api/tasks/OPP-42`); `id` in
  `TaskSummary` / `TaskDetail` / ref chips / matrix rows / change events becomes
  the key; OpenAPI carries the pattern. Then `mise run generate-web-client`.
- **CLI** — every `<id>` argument and every printed id, `--json` included
  (`crates/op-cli/src/main.rs`). Errors name the expected form.
- **Web** — route `/task/OPP-42` (`src/main.tsx:18`); the id column
  ([[./00051-web-ui-show-a-task-s-id.md]]); Cmd+. copies `OPP-42`
  (`lib/clipboard.ts`, `lib/copy-target.ts`); the search combobox matches keys;
  `[[…]]` chips render keys.
- **op-index** — the matrix keys by task id; internal `u64` unchanged, only the
  API boundary formats.

## References inside task bodies

- The store keeps writing the file form: `parent: ./00042-ship-login-page.md`,
  `[[./00042-ship-login-page.md]]`.
- A human may type `[[OPP-42]]`; it resolves, and the next write through the
  daemon normalizes it to the file form.
- `[[42]]` and a foreign `[[WEB-7]]` are **refused on write**. A hand-edited
  file that already contains one renders it as plain text — it resolves to no
  task, so it gets no chip.

## Migration

- Add `.plan/config.toml` with `abbreviation = "OPP"` **in this change** — the
  repo's own daemon will not start without it.
- No task file rewrites: every one of the 19 ref-carrying bodies uses the file
  form today; no bare `[[42]]` exists on disk.
- Knowingly breaking: bookmarked `/task/42` URLs stop resolving, and any script
  or agent passing a bare number to `oplan` fails loudly. Acceptable pre-1.0 on
  a single-machine tool — a silent numeric fallback would reintroduce the second
  spelling this task exists to remove.

## SPEC.md

- §3.1 — replace "The number is the whole id" with the two-layer model: number
  allocates and names the file, key is the id above the store; state the single
  spelling and the refusals.
- §4 — add `.plan/config.toml` to the storage layout.
- §8 — the CLI sketch's `<id>` placeholders.

## Verify

- `op-task`: key format/parse, every refusal in the table above.
- `op-store`: config present / missing / malformed / wrong shape; branch
  divergence ignored; live reload.
- `op-cli` (`crates/op-cli/tests/cli.rs`): key arguments, key output, non-zero
  exit with no config.
- `op-server`: routing by key, `id` shape in payloads. (A fresh worktree needs
  the gitignored web dist copied in or these 404.)
- Web tests + an interactive pass per the repo's web-UI verify recipe: id
  column, Cmd+. clipboard contents, deep link, search by key.
- `cargo build && cargo test && cargo fmt --check && cargo clippy -- -D warnings`

## Non-goals

- Cross-store resolution of foreign keys via `~/.plan/registry.toml` — that is
  the real payoff of prefixes and a much larger feature (uniqueness enforcement,
  two stores addressable at once). Foreign keys are refused for now.
- No uniqueness check across registered stores, since nothing resolves across
  them yet.
- No CLI for setting the abbreviation.

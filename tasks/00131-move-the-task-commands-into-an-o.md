---
status: done
created: 2026-09-27T13:54:57Z
tags:
- cli
- docs
- feature
---
# Move the task commands into an openplan tasks group

The top level of `openplan --help` mixes the task commands with the project commands. Move each task command into a new `openplan tasks` group, as `openplan doc` holds the doc commands.

| Now | After |
|---|---|
| `openplan create` | `openplan tasks create` |
| `openplan list` | `openplan tasks list` |
| `openplan search` | `openplan tasks search` |
| `openplan get` | `openplan tasks get` |
| `openplan write` | `openplan tasks write` |
| `openplan comment` | `openplan tasks comment` |
| `openplan comments` | `openplan tasks comments` |
| `openplan show` | `openplan tasks show` |
| `openplan tree` | `openplan tasks tree` |
| `openplan move` | `openplan tasks move` |
| `openplan set` | `openplan tasks set` |
| `openplan delete` | `openplan tasks delete` |

These commands stay at the top level: `init`, `migrate`, `setup-skills`, `history`, `sync`, `open`, `url`, `lint`, `doc`, `tag`, `project`, `update`, `server`. Each one works on the whole project, works on tasks and docs together, or is a group already.

Change each text that names an old form:

- the openplan skill in `crates/op-skills/skills/openplan/SKILL.md`, and the installed copies in this repository (run `openplan setup-skills`)
- the help text of the lint findings in `crates/op-cli/src/lint.rs` and `crates/op-cli/src/tag.rs`
- the messages that name `openplan comment` in `crates/op-server/src/tasks.rs` and `crates/op-tracker/src/lib.rs`
- the empty-list text in `web/packages/app/src/routes/list.tsx`
- the README, the CHANGELOG, the CLI tests, and the source comments that name `openplan get`

## Comments

### 2026-09-27T14:11:13Z by Milan Suk via claude-code

> The CHANGELOG keeps the old forms in the released entries, because those versions shipped them. An Unreleased entry names the move.

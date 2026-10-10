---
status: in_progress
created: 2026-10-03T15:38:28Z
tags:
- cli
- daemon
- feature
- ui
---
# Change the project abbreviation through the CLI and show the change in the activity page

## Problem

No command changes the abbreviation of a project. `openplan init --abbreviation XYZ` on a project that has an abbreviation fails with `AlreadyInitialized`. The only way is a manual edit of `config.toml` in the task store.

The activity page also shows a change to `config.toml` badly. `Described::lines` gives `config.toml: modify` for every change after the first, and the web UI shows the file as an "other" document row. A reader cannot see the old abbreviation or the new abbreviation.

## Scope

1. Add a CLI command that changes the abbreviation, for example `openplan project abbreviation XYZ`. The command calls a new daemon endpoint. The tracker writes the new `config.toml` in one revision.
2. Reject a value that is not three uppercase letters. Reject the current abbreviation as a no-op.
3. Show the change in the activity page as one line with the two values, for example `Change the task keys from OPP to XYZ`. Use the same words in the commit message. The describer reads the old value from the parent revision.
4. Show the change as a row of its own in the activity table, not as a generic `config.toml` document row.

## Notes

- Task files and `[[...]]` links keep the number and the path, not the key. A change of abbreviation needs no rewrite of a task or a doc.
- After the change, the old keys (`OPP-42`) name no task. The CLI prints the new key of each task. Keep this behavior.
- The activity page spells the keys of old revisions with the current abbreviation. Keep this behavior, so that each link opens a task that exists.

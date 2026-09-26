---
status: in_review
created: 2026-09-26T02:05:59Z
tags:
- docs
- feature
---
# Make the openplan skill show task keys as links to the web UI

Change the openplan skill (`.claude/skills/openplan/SKILL.md`) so that the agent writes each task key in its replies as a markdown link to the task page in the local web UI. The user can then click `OPP-42` to open the task.

## Requirements

- The agent writes a key as `[OPP-42](<url>)` in replies to the user.
- The URL is the task page that the web UI serves: the daemon base URL plus the path from `taskPath(project, id)` in `web/packages/task-ui/src/task-path.ts`.
- The daemon listens on port 7373 by default. `OPENPLAN_PORT` changes the port. The skill must not hard-code a URL that is wrong when the port changes.
- The agent must know the project name that the path needs.

## Open questions

- How does the agent get the base URL and the project name? One option: a CLI command (for example `openplan url <key>`) that prints the full URL. Another option: `openplan get --json` and `openplan list --json` return a `url` field.
- Does the rule apply only to replies, or also to commit messages and pull requests? A local URL is not useful to other people, so replies only is the likely answer.

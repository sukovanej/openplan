---
status: in_review
created: 2026-09-04T10:55:09Z
---
# Lint that installed agent skills match the binary

The `openplan` binary embeds the agent skills and `openplan setup-skills` writes
them into `.claude/skills/` and `.agents/skills/`. Nothing tells the user when a
written copy no longer matches the binary, so an edited or stale `SKILL.md`
stays in the repository without notice.

Add a `skill` rule to `openplan lint`. For each agent that has a skills
directory, the rule reports a skill file that is missing and a skill file whose
content differs from the binary. `openplan lint --fix` writes the binary content
back.

The repository CI already runs `openplan lint`, so the rule also holds the
checked-in copies of `crates/op-skills/skills/` in sync.


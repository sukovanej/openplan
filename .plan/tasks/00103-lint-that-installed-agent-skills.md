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

## Comments

### 2026-09-04T11:01:24Z by Milan Suk via claude-code

> The rule compares the files against the binary that runs the lint. An installed binary that is older than the repository asks the user to write its own older skills back. The binary is the only content the rule can read, so it stays the reference.

### 2026-09-04T11:01:24Z by Milan Suk via claude-code

> The rule reads only the skills the binary carries, and only for an agent that already has a skills directory. A repository keeps its own skills next to them, and an agent that never ran `setup-skills` gets no report.

### 2026-09-04T11:17:57Z by Milan Suk via claude-code

> A skill the binary drops keeps its installed copy. The rule reads the skills the binary carries now, and nothing on disk says which files `setup-skills` wrote before, so a renamed or deleted skill leaves an orphan that lint never reports and `--fix` never removes. To close it, `setup-skills` must record what it wrote.

---
name: implement-task
description: Build a task in .plan/tasks/ end to end with the implement-task workflow — test-first, reviewed, to a PR. Triggers on: implement OPP-42, build this task, work on OPP-42, take OPP-42, do the task.
---

# Implement a task

Run the `implement-task` workflow. Do not hand-implement a task this skill covers.

```
Workflow({ name: 'implement-task', args: { taskKey: 'OPP-42', maxRounds: 3 } })
```

`args`: `taskKey` (required, `OPP-42` form only), `maxRounds` (default 3), `base` (default `main`).

## Before launching

**Run from the primary checkout.** The workflow creates the task's worktree itself and
copies the gitignored `web/packages/app/dist` in. Launched from inside a worktree, both
paths resolve to a nested worktree and op-server tests 404.

**Confirm the task is ready.** `oplan show <key>` — it needs a `## Verify` section, which
the workflow treats as the acceptance criteria and turns into the first round of tests. A
task without one will halt at `plan-mismatch`.

**Expect 30–45 agents per round.** Over the default workflow size cap; the run is throttled,
not broken, if the cap is left at medium. Verification is batched one agent per
review-dimension × angle, so the count does not grow with the number of findings.

## What it does to the task

Marks it `in_progress` as the first write inside the new worktree, and `in_review` in the
same commit as the PR. Never `done` — that stays a human call.

## Halts

Three states return the gap instead of a PR. Each is a real answer, not a failure to retry
blindly:

- `plan-mismatch` — the task body is ambiguous or names something absent from the codebase
- `stubs-do-not-compile` — no `todo!()` surface, so no meaningful red gate
- `no-clean-red` — the tests never failed for the right reason, so implementing against them
  would prove nothing

A red check suite or unresolved findings are not halts: those open a **draft** PR with the
failures written into the body.

## Resuming

Every run persists its script and returns a `runId`. After fixing a prompt or schema, relaunch
with `{scriptPath, resumeFromRunId}` — the unchanged prefix of agents replays from cache and
only the edited call onward re-runs. Same session only.

## Not for this

Single-file edits, follow-ups on an open PR, and anything without a task file. Just do those.

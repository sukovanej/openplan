---
status: backlog
created: 2026-07-26T13:14:25Z
---
# Writes 404 on a task that lives only on another branch

`GET /api/board` and `GET /api/tasks` aggregate every branch the task set spans,
so the list shows tasks whose file exists only on some other branch. `PATCH` and
`DELETE` resolve the id against the **write** branch — the serve root's checked
out worktree — so those same rows 404 when a user acts on them.

Observed while verifying #37 against a live daemon:

```
GET  /api/board                      -> row: continuous-changes-accumulation-v-0cb0
PATCH /api/tasks/continuous-changes-accumulation-v-0cb0
  -> 404 {"message":"no such task: continuous-changes-accumulation-v-0cb0"}
```

Nothing in the row tells the client the task is unreachable for writes, so the
web UI offers controls (reparent, add-subtask, status) that cannot succeed. The
new mutation-error banner now surfaces the 404 instead of dropping it, which
makes the gap visible but not less confusing: "no such task" is wrong, the task
plainly exists.

## Options

- **Write through to the owning branch.** `PATCH` targets the branch the task
  headlines on when the serve root does not have it. Needs a writable worktree
  for that branch; §7 already refuses writes to a branch with no live worktree,
  so this reduces to picking the branch rather than a new mechanism.
- **Say so in the read model.** Carry a per-row "writable here" flag from the
  index, and have the client disable the controls and explain why. Cheaper, and
  honest, but leaves the action impossible.
- **Improve the error only.** Keep the 404 but say "task <id> lives on <branch>,
  which is not checked out here". Smallest change; still a dead end for the user.

Prefer the first if a worktree for the owning branch is available and the second
as the fallback when it is not — they compose.

## Out of scope

Cross-branch *reads* already work and are not in question here.

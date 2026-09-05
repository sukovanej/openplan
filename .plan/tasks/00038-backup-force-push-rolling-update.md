---
status: backlog
created: 2026-07-26T15:40:55Z
parent: ./00039-continuous-changes-accumulation-v.md
dependencies:
- ./00040-daemon-ambient-writer-accumulate.md
tags:
- feature
- git
---
# Backup: force-push rolling-updates to a mirror remote (durability)

**Phase 6** of the rolling-updates plan
([[./00023-design-a-continuous-changes-accu.md]]). Durability only: un-published
ambient edits survive disk/machine loss. Sole-writer, no distributed concurrency;
multi-machine / collaborative sync stays out of scope.

## Mechanism

`gix` 0.85 has fetch/transport but **no push**, so backup **shells to
`git push --force`** — a legitimate git-op (unlike the merge-driver reconcile,
where shelling was forbidden). 

## Design

**Opt-in config.** Git-config key `openplan.backupRemote` (remote name or URL),
read with `config_snapshot().string`. Unset -> backup disabled, clean no-op.
No new config format.

**`BackupPusher` — separate, non-blocking, coalescing.** NOT on the ref-owner
actor's critical path ([[./00040-daemon-ambient-writer-accumulate.md]]); it must never
delay an ambient ack or a refresh. Notified "tip moved" whenever the actor
advances `rolling-updates` (accumulate flush / refresh / publish),
debounces/coalesces a burst into one push of the latest tip:

```
git push --force <remote> refs/open-plan/rolling-updates:refs/open-plan/rolling-updates
```

Force is safe: sole writer, write-only mirror nobody pulls.

**Best-effort.** A failed push logs `warn` and is NOT propagated to the edit; it
retries on the next tip move. A **startup push** of the current tip covers a
final push that failed as the daemon last exited. Optionally record a
`last_backup` ok/failed timestamp for diagnostics; no user-facing surface in v1.

**No read coupling.** The push only reads the ref and talks to the remote; it
writes no local ref, so it cannot collide with the sole-writer invariant and runs
concurrently.

## Scope

Config + pusher + startup push. No UI (a backup-health indicator is a later
nicety). Refresh / publish / routing untouched.

## Verify

- `openplan.backupRemote` set -> a ref advance pushes `refs/open-plan/rolling-updates`
  to the mirror (assert against a local bare remote); unset -> no push.
- a burst of advances coalesces to one push of the latest tip.
- push failure (unreachable remote) logs `warn`, does not fail the edit, retries
  on the next advance.
- startup push sends the current tip once on boot.

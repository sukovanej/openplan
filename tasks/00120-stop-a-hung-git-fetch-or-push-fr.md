---
status: todo
created: 2026-09-26T00:42:26Z
tags:
- bug
- daemon
- git
---
# Stop a hung git fetch or push from blocking task sync

A network git command in the sync can hang with no end. Then the sync thread
blocks, and every later sync waits behind it. "Sync now" in the web UI does
nothing, because only the blocked thread reads the due time.

## What happened

The daemon started `git fetch ... refs/openplan/tasks`. Git ran
`ssh git@github.com`. The TCP connection stayed ESTABLISHED, but no data came.
This is a dead half-open connection after a sleep or a network change. The
fetch hung for 1 h 42 min, until someone killed the `ssh` process.

```d2
shape: sequence_diagram
ui: Web UI
loop: Sync thread
git: git fetch
ssh: ssh
ui -> loop: sync now (sets due)
loop -> git: Command::output()
git -> ssh: git-upload-pack
ssh -> ssh: waits on a dead connection
ui -> loop: sync now (nobody reads due)
```

## Cause

`git()` in `crates/op-backend-git/src/sync.rs` calls `Command::output()` with
no timeout. `run()` in `crates/op-backend/src/sync_loop.rs` blocks in
`remote.sync()` until git exits.

## Change

- Give each network git call in `git()` (fetch and push) a deadline, for
  example 60 s. Spawn the child, wait with the deadline, and kill the child
  when the deadline passes.
- Return a sync error that says the command timed out. The loop then uses its
  normal backoff.
- Do not set `core.sshCommand` or `GIT_SSH_COMMAND`. The fix must keep the
  SSH setup of the user, and it must work for HTTPS remotes too.

## Test

Point a remote at a command that never answers, and check that `sync()`
returns an error after the deadline.

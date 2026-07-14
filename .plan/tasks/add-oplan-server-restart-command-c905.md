---
status: todo
---
# Add `oplan server restart` command

## Goal

Add a `restart` subcommand to the `oplan server` command group so a running
daemon can be recycled in one step (stop, then start), matching the existing
`server {start,stop,ping}` lifecycle commands.

## Behavior

- `oplan server restart [--port N]` stops the running daemon (gracefully if it
  answers, same path as `server stop`), waits for it to fully exit and its
  runtime files to be cleaned up, then starts a fresh detached daemon on the
  requested port (same path as `server start`).
- If no daemon is running, it behaves like a plain `server start` (no error).
- Like `server start`, reject `--daemon <url>` with a clear message — restart
  operates on the local machine daemon, not a remote one.
- Return success once the new daemon's `/health` is reachable (reuse the
  start/ensure-ready polling), non-zero if it fails to come up.

## Implementation notes

- `crates/op-cli/src/main.rs`: add a `Restart { port }` variant to
  `ServerCommand` and wire it in `server()`.
- `crates/op-cli/src/daemon.rs`: add a `Control::restart(port, root)` that
  composes the existing `stop` + `start`, ensuring the stop completes (pid gone,
  `daemon.json` removed) before the new spawn so the singleton lock is free.

## Tests

Add to `crates/op-cli/tests/daemon.rs`: restart while running rebinds a fresh
daemon (new pid, healthy), and restart while stopped just starts one.

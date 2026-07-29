---
status: done
created: 2026-07-13T23:01:08Z
---

# Run the server as a managed daemon with auto-start

## Goal
Make `oplan`'s server a real background daemon with a managed lifecycle: start it detached,
stop it gracefully, query its status, and — for any command that needs to talk to it —
transparently start it first if it isn't already running. Replace the foreground `serve` and
stub `ping` with an `oplan server {start,stop,ping}` command group.

## Design

### Machine singleton
The daemon is a **per-machine singleton** (§7.3), keyed by a machine-local home dir
(default `~/.plan`, overridable via `OPLAN_HOME` — required so tests never touch the real
home). All lifecycle state lives there:
- `daemon.json` — `{ pid, port, version, started_at }`, written atomically once the port is
  bound (records the *actual* port). This is how clients discover where to connect and report
  status.
- `daemon.lock` — fs2 advisory lock. Held for the daemon's lifetime to enforce the singleton,
  and taken briefly by clients around a spawn to prevent two callers racing to start two
  daemons.
- `daemon.log` — the detached process's stdout+stderr.

### Detaching without `unsafe`
`unsafe_code` is `forbid`ped workspace-wide, so no hand-rolled `fork`/`setsid`/`pre_exec`.
Instead:
- `oplan server start` (default) re-spawns `current_exe()` as `oplan server start --foreground
  --port N`, with `stdin(null)`, stdout/stderr → `daemon.log`, and its own process group via
  the stable, safe `CommandExt::process_group(0)`; the parent does not wait and returns
  immediately.
- The `--foreground` child (the actual daemon) **ignores SIGHUP** (tokio signal handler) so it
  survives terminal hangup, and handles SIGINT/SIGTERM for graceful shutdown. This gives full
  detachment with zero unsafe and no extra crate.
- `--foreground` is also the debugging path — run it to watch the server in your terminal.

### Startup / singleton guard
On `--foreground` boot: take `daemon.lock` (fail fast with a clear message if another daemon
holds it), bind the port, write `daemon.json` atomically, then serve. On shutdown, drop the
lock and remove `daemon.json`.

### Graceful shutdown
Wire `axum::serve(...).with_graceful_shutdown(sig)` where `sig` fires on SIGINT, SIGTERM, or an
admin shutdown request. Add a loopback-only `POST /admin/shutdown` to op-server that triggers
it. `stop` prefers this HTTP path (clean: lets watchers stop and files get removed); if the
daemon is wedged and won't answer, fall back to signalling the pid from `daemon.json` (via the
`nix` crate — keeps unsafe out of our crates), then wait for exit and clean up runtime files.

### Auto-start on demand (`ensure_daemon`)
A single helper used by every command that needs the daemon:
```
ensure_daemon(home, port):
  if health_ok(addr, short_timeout): return Connected(addr)
  with daemon.lock:                       # serialize concurrent starters
    if health_ok(addr): return Connected  # someone else won the race
    spawn detached daemon (as above)
  poll GET /health with backoff until ready or deadline -> Connected | Err
```
- Only commands that genuinely need coordination call `ensure_daemon` (today: none of the
  task-file CRUD, which is local — see task-crud). It's the mechanism future
  claim/presence/aggregated-read commands opt into. `server ping` deliberately does **not**
  auto-start — it only observes.

## CLI surface
Remove `serve` and `ping`. Add:
```
oplan server start [--port N] [--foreground]   # detach by default; --foreground blocks (also internal)
oplan server stop                              # graceful shutdown, then clean up runtime files
oplan server ping                              # status only: prints pid/port/uptime/version, or "not running"
```
- `server start` is idempotent: if a healthy daemon already runs, print `already running (pid N,
  port P)` and exit 0 without spawning a second.
- `server ping` exits non-zero when the daemon is down (script-friendly).
- Reconcile the existing global `--daemon <url>` with `--port`: `--daemon` is the explicit
  connect override; otherwise the address comes from `daemon.json`, falling back to the default
  port.

## Crate changes
- **op-cli**: delete `Serve`/`Ping`; add the nested `server` subcommand group. New `daemon`
  module owning: runtime paths + `OPLAN_HOME`, `daemon.json` read/write (atomic), `daemon.lock`,
  detached spawn, `health` probe, readiness poll, `ensure_daemon`, and `stop`. `serve::run`
  becomes the `--foreground` body: acquire lock → bind → write `daemon.json` → serve with
  graceful shutdown → cleanup.
- **op-server**: add graceful-shutdown wiring to `serve(...)` (take a shutdown signal/future);
  add loopback-only `POST /admin/shutdown`. `/health` already exists and is the readiness probe.
- **workspace deps**: add `nix` (signal fallback for `stop`) if not using HTTP-only shutdown.

## Acceptance criteria
- [x] `oplan server start` returns immediately; the daemon keeps running after the invoking
      shell exits; `daemon.json` records a live pid and the bound port; logs land in
      `daemon.log`.
- [x] A second `oplan server start` while one runs is a no-op reporting the existing pid/port;
      exactly one daemon process exists.
- [x] `oplan server ping` prints running status (pid/port/uptime/version) when up and
      `not running` with a non-zero exit when down — and never starts a daemon.
- [x] `oplan server stop` shuts the daemon down gracefully, waits for exit, and removes
      `daemon.json`; `stop` with nothing running is a clean no-op with a clear message.
- [x] `ensure_daemon` auto-starts a down daemon, waits for `/health`, then proceeds; two
      concurrent callers start exactly one daemon (start-lock + health re-check).
- [x] A crashed daemon (stale `daemon.json`, lock released) is detected via failed health probe
      / free lock and does not block a fresh start; `ping` reports it as not running.
- [x] `--foreground` runs in the terminal and exits cleanly on Ctrl-C, releasing the lock and
      removing `daemon.json`; the daemon survives SIGHUP.
- [x] `serve` and `ping` no longer appear in `--help`; `server --help` lists start/stop/ping.
- [x] Tests (with `OPLAN_HOME` pointed at a tempdir, ephemeral port): start → ping → stop
      roundtrip; singleton (concurrent starts yield one pid); `ensure_daemon` from-down path.
- [x] `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings` pass.

## Out of scope (follow-up tasks)
- **Multi-project registration** — one machine daemon serving N stores (`§7.3` registry,
  `POST /admin/register`). For now the daemon watches the store it was started in; a single
  active project is assumed. This is the immediate next step and blocks nothing here.
- OS service integration (launchd/systemd unit files); crash auto-restart / supervision.
- Everything already deferred by task-crud (section addressing, matrix, presence).

## Notes
- **Supersedes the `ping` change in [[./00003-task-crud-across-the-store-daemo.md]]:** that task's "real `ping`" is now
  `oplan server ping`, delivered here. task-crud's local CRUD does not call `ensure_daemon`.
- Keep timeouts tight: health probe ~250ms; readiness poll a few seconds with backoff, then a
  clear timeout error pointing at `daemon.log`.
- Singleton scope is per `OPLAN_HOME`, not per project — surface this in `server start` output
  so it's never surprising.

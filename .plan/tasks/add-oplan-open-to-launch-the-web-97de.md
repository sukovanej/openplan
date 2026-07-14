---
status: todo
created: 2026-07-14T10:31:08Z
---
# Add `oplan open` to launch the web UI in a browser

Top-level `oplan open` command that launches the realtime web UI in the
user's default browser.

## Behavior

- Ensure a daemon is serving by reusing `Control::ensure_daemon`
  (`crates/op-cli/src/daemon.rs`): auto-start a detached daemon and wait until
  healthy if none is running, otherwise use the running one.
- Resolve the URL from the *actual* bound port recorded in `~/.plan/daemon.json`
  via `Home::read_info` — not `DEFAULT_PORT`, since the daemon may bind port `0`
  (any) or a custom port.
- Honor the global `--daemon <url>` flag: open that URL directly and do **not**
  auto-start.
- Launch the default browser at `http://127.0.0.1:<port>/` (root URL only).
- Decided: if the launcher fails (no browser / headless SSH / CI), exit non-zero
  with a clear error — no URL-print fallback. Known cost: `oplan open` is a
  dead-end in headless environments; revisit if that bites.

## Launcher

- Respect the `$BROWSER` environment variable when set (Linux convention);
  otherwise the platform default (`open` on macOS, `xdg-open` on Linux). Use the
  `open` crate, or hand-roll — the rest of the CLI is already unix-only.
- The `$BROWSER` override doubles as the test seam.

## Out of scope (follow-ups)

- Task deep-linking (`oplan open <id>`): the SPA has no client-side task routes
  yet. Separate task once it does.
- Store-mismatch warning: the daemon serves whichever store it was first started
  in, so `oplan open` from a different project silently shows the daemon's store.
  Detecting this needs the daemon to advertise its root (add to `DaemonInfo` /
  `/health`). Note only; out of scope here.

## Tests (in `tests/`)

- URL resolution: reads the bound port from `daemon.json`; `--daemon <url>`
  overrides; auto-start path is exercised.
- Launch: set `$BROWSER` to a stub command so CI asserts the resolved URL
  without spawning a real browser; assert a non-zero exit when the launcher
  command fails.

## Refs

- SPEC.md:250 (`op serve [--open]`) — superseded by this standalone command.
- Reuse: `crates/op-cli/src/daemon.rs` (`Control`, `Home`),
  `crates/op-server/src/lib.rs` (`static_handler` already serves the embedded SPA).

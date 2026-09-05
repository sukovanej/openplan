---
status: done
created: 2026-09-04T16:27:43Z
---
# Desktop app: a Tauri shell for the web UI

## Purpose

The web UI lives in a browser tab. The tab hides among many other tabs, the browser keeps the
shortcuts the UI wants, and the tab shows the browser's icon. A tab also cannot start the daemon, so
the UI is only there when a person brought the daemon up by hand. The shell fixes that.

## What the shell is

`crates/op-gui` holds the package `openplan-gui`, which builds the binary of the same name. It opens
one Tauri window on the running daemon at `http://127.0.0.1:<port>/`. The page is the SPA that
`op-server` already embeds, so `web/packages/app` needs no change.

Do not bundle a second copy of the SPA and do not load it from `tauri://localhost`. A second copy
needs an absolute API base URL, CORS on the daemon, and a second build to keep in step. The UI a
person sees must be the UI of the daemon that answers it.

## Starting the daemon

`crates/op-daemon` is a new crate. It holds `Home`, which resolves `$OPENPLAN_HOME` or `~/.plan` and
reads `daemon.json`. It holds `Control`, which starts, stops, and probes the daemon. It holds the
serving loop itself. `crates/op-cli/src/daemon.rs` keeps only what it prints.

The recorded port is a claim, so `Control` confirms it with `GET /health` before the window loads.
When nothing answers, `Control` starts a daemon and polls `/health` under a 5 second deadline.

The shell must not search for an `openplan` binary. `Control` starts the daemon by a re-exec of the
running executable with `--serve-daemon <port>`, and both binaries answer that argument before their
own parsing. The shell therefore starts a daemon out of itself, and the app bundle needs no second
binary. The argument is a flag, not an environment variable: the daemon runs
`openplan mergedriver`, which an inherited variable would turn into a second daemon.

When the daemon does not answer, the window shows the reason instead of a white page.

Closing the window must not stop the daemon. The CLI and the merge driver share it.

## Build

`mise run gui` builds and starts the shell. It does not build the SPA: that restarts the daemon the
CLI and the merge driver share, and opening a window is no reason to take it down. `frontendDist`
points at the splash page only, because the daemon serves every other asset.

## Acceptance

- Starting the app with no daemon running starts one and shows the board.
- With `OPENPLAN_PORT=7500` already serving, the app finds that port and starts no second daemon.
- Closing the window leaves `openplan server ping` reporting a running daemon.
- With no `openplan` on `PATH`, the app still starts a daemon and shows the board.
- `cargo build`, `cargo test`, `cargo fmt --check`, and `cargo clippy -- -D warnings` pass.

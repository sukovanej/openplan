---
status: in_review
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
reads `daemon.json`, and it holds `default_port` and `base_url`. `crates/op-cli/src/daemon.rs` keeps
the lifecycle commands and imports the rest from there.

The recorded port is a claim, so the shell confirms it with `GET /health` before the window loads.
When nothing answers, the shell runs `openplan server start` and polls `/health` under a 5 second
deadline.

The shell must not spawn the daemon from its own executable. The daemon respawns itself from the
executable that started it, and a GUI binary serves no HTTP. The shell resolves the CLI in this
order: `$OPENPLAN_BIN`, the app bundle's `Resources/bin/openplan`, `PATH`, then `~/.cargo/bin`. A
macOS app started from the dock inherits a small `PATH`, so `PATH` alone finds nothing for a
developer install.

When no step finds a binary, or the daemon does not answer, the window shows which variable fixes it
instead of a white page.

Closing the window must not stop the daemon. The CLI and the merge driver share it.

## Build

`mise run gui` builds the SPA, then builds and starts the shell. The Tauri config declares no
`frontendDist`, because the daemon serves every asset.

## Acceptance

- Starting the app with no daemon running starts one and shows the board.
- With `OPENPLAN_PORT=7500` already serving, the app finds that port and starts no second daemon.
- Closing the window leaves `openplan server ping` reporting a running daemon.
- With no `openplan` binary anywhere, the window names `$OPENPLAN_BIN`.
- `cargo build`, `cargo test`, `cargo fmt --check`, and `cargo clippy -- -D warnings` pass.

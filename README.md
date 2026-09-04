# open-planner

Local-first, file-based task manager for humans and AI agents, in plain markdown.
Design and work items in [.plan/tasks/](.plan/tasks/).

## Build

```sh
cargo build
cargo test
cargo fmt --check
cargo clippy -- -D warnings
```

The web UI lives in `web/` (a pnpm workspace). Its build output
(`web/packages/app/dist/`) is gitignored — build it before `cargo build` so the SPA gets
embedded. Without a build the daemon still compiles and runs, but serves no web UI.

```sh
cd web && pnpm install && pnpm -r build   # → web/packages/app/dist
cargo build                               # embeds the SPA
```

Web workspace checks: `pnpm -r typecheck`, `pnpm lint`, `pnpm format:check`, `pnpm -r test`.
Live development: `pnpm --filter @open-planner/app dev` (Vite on :5173, proxying the API to
the daemon on :7373).

## Run

`openplan` is the single binary. Put it on PATH and start its daemon on that build:

```sh
mise run install     # SPA → release binary → PATH → daemon restarted on it
```

The daemon respawns itself from its own executable, so the binary that starts it is the one that
keeps serving. Installing and restarting together is what keeps the daemon and the checkout the
same build.

Without installing, run it from the checkout as `cargo run -p op-cli -- <args>`:

```sh
openplan list                       # tasks in ./.plan
openplan open                       # the web UI in your browser
openplan server start               # background daemon: realtime API + web UI on 127.0.0.1:7373
openplan server ping                # report daemon status
openplan server stop                # stop the background daemon
openplan project list               # repositories the daemon serves
openplan merge-driver <O> <A> <B>   # git merge driver for .plan/**.md
```

Every task command goes through the daemon and starts it if it is down, so a query answers the same
whether the CLI or the web UI asked it. `lint` is the exception: it checks the files in front of you
and never starts a daemon. One daemon serves every repository on the machine: the first write from
a repository registers it, and `openplan project` manages the registry. `OPENPLAN_HOME` picks the
daemon's state directory (default `~/.plan`), `OPENPLAN_PORT` its port (default 7373).

### Desktop window

```sh
mise run gui     # the window on the running daemon, starting one when none runs
```

It loads `http://127.0.0.1:<port>/`, so it shows the SPA the daemon serves. Run `mise run install`
after a change to the SPA. `OPENPLAN_BIN` names the `openplan` binary to start the daemon with,
`CARGO_HOME` moves the cargo directory it searches.

### Icons

`assets/icon.svg` is the only source. Edit it, then rasterize:

```sh
mise run icons   # → crates/op-gui/icons/ and web/packages/app/public/
```

## License

MIT OR Apache-2.0.

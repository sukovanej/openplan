# open-planner

Local-first, file-based task manager for humans and AI agents, in plain markdown.
Design and work items in [.plan/tasks/](.plan/tasks/).

> Compiling skeleton: crates, dependency edges, and the `openplan` binary are wired;
> domain logic lands in child tasks.

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

`openplan` is the single binary — run it via `cargo run -p op-cli -- <args>`:

```sh
openplan list                       # tasks in ./.plan
openplan server start               # background daemon: realtime API + web UI on 127.0.0.1:7373
openplan server ping                # report daemon status
openplan server stop                # stop the background daemon
openplan project list               # repositories the daemon serves
openplan merge-driver <O> <A> <B>   # git merge driver for .plan/**.md
```

Reads work straight off the files; writes (`create`, `set`, `move`, `delete`) go through the daemon
and start it if it is down. One daemon serves every repository on the machine: the first write from
a repository registers it, and `openplan project` manages the registry. `OPLAN_HOME` picks the
daemon's state directory (default `~/.plan`), `OPLAN_PORT` its port (default 7373).

## License

MIT OR Apache-2.0.

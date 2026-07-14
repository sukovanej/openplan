# open-planner

Local-first, file-based task manager for humans and AI agents, in plain markdown.
Design in [SPEC.md](SPEC.md); work items in [.plan/tasks/](.plan/tasks/).

> Compiling skeleton: crates, dependency edges, and the `oplan` binary are wired;
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

`oplan` is the single binary — run it via `cargo run -p op-cli -- <args>`:

```sh
oplan list                       # tasks in ./.plan
oplan server start               # background daemon: realtime API + web UI on 127.0.0.1:7373
oplan server ping                # report daemon status
oplan server stop                # stop the background daemon
oplan merge-driver <O> <A> <B>   # git merge driver for .plan/**.md
```

## License

MIT OR Apache-2.0.

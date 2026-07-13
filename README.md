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

## Run

`oplan` is the single binary — run it via `cargo run -p op-cli -- <args>`:

```sh
oplan list                       # tasks in ./.plan
oplan serve                      # realtime API + web UI on 127.0.0.1:7373
oplan merge-driver <O> <A> <B>   # git merge driver for .plan/**.md
```

## License

MIT OR Apache-2.0.

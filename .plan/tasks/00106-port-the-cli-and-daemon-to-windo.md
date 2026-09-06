---
status: backlog
created: 2026-09-06T11:43:19Z
tags:
- cli
- daemon
- feature
---
# Port the CLI and daemon to Windows

Make the workspace compile and run on `x86_64-pc-windows-msvc`. The CLI, the
daemon, and the merge driver must work.

## Design

- **Unix-only calls.** `op-cli` and `op-daemon` use `nix`, unix signals, and
  `std::os::unix::process::CommandExt`. `op-store` uses `MetadataExt`. Replace
  each call with a platform-neutral one, or split the code per platform.
- **Stop the daemon.** `Control::stop` sends SIGTERM. Windows has no SIGTERM.
  The daemon answers `/shutdown` over HTTP first, so Windows keeps that path and
  uses `TerminateProcess` as the last step.
- **Spawn the daemon.** `spawn_detached` calls `process_group(0)`. Windows needs
  `CREATE_NEW_PROCESS_GROUP` and `DETACHED_PROCESS`.
- **Home directory.** `Home::resolve` must find `%LOCALAPPDATA%` on Windows.
- **Paths in files.** The store writes task paths into task files. A parent link
  must stay `./00042-title.md` on every platform.
- **Locks.** `fs2` supports Windows. Prove the daemon lifetime lock and the
  start lock behave the same.
- **Release.** Add `x86_64-pc-windows-msvc` to `[workspace.metadata.dist]
  targets`. cargo-dist then makes a PowerShell installer.
- **CI.** Add a Windows job to `ci.yml`. A tag must not start the first Windows
  build.

## Constraints

- The workspace sets `unsafe_code = "forbid"`. A direct `TerminateProcess` call
  needs `unsafe`. Take a crate that wraps it, or lift the lint in one crate and
  say why.
- The merge driver runs inside `git merge`. Test it with Git for Windows.

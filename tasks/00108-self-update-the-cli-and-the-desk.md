---
status: todo
created: 2026-09-06T11:45:17Z
tags:
- cli
- feature
- ui
---
# Self-update the CLI and the desktop app

`openplan update` replaces the CLI and the desktop app with the newest release.

## Design

- **One command, no flags.** `openplan update` takes no arguments. It reads the
  GitHub `releases/latest` endpoint, which skips prereleases. Nothing checks for
  a new version on its own. There is no periodic check and no check at start.
  The command prints the version and exits 0 when the newest release is already
  installed.
- **Platforms.** macOS updates the CLI and `OpenPlan.app`. WSL updates the CLI,
  because WSL runs no app. The command ignores the deb and the AppImage. Windows
  comes with [[./00106-port-the-cli-and-daemon-to-windo.md]] and
  [[./00107-ship-the-windows-desktop-app.md]].
- **Trust.** The download uses HTTPS to github.com, and the command verifies the
  SHA256 that the release publishes. cargo-dist writes that file for the CLI
  archive. `release-app.yml` must write one for each app artifact. There is no
  signing key: it would live in the same secrets that build the artifacts, so it
  protects nothing that TLS does not.
- **New release artifact.** `release-app.yml` uploads
  `OpenPlan-<target>.app.tar.gz` next to the dmg. The updater unpacks the
  tarball and renames the bundle. It never mounts a dmg.
- **App receipt.** The app writes its pid, its bundle path, and its version into
  `OPENPLAN_HOME` at start. The daemon already keeps such a file, and `Control`
  reads it. The update needs the pid to quit the app and the path to replace it.
- **Order.** Read the newest version. Download both artifacts. Verify both
  digests. Quit the app. Stop the daemon. Replace both. Start the daemon from
  the new CLI. Wait until it answers `/health`. Open the app again when it ran
  before.
- **Failure is loud.** Any step stops the command and prints the reason. The old
  install stays. The command replaces nothing until every download verifies, so
  it needs no rollback.
- **Refuse a binary it does not own.** The command stops when the CLI sits in
  `~/.cargo/bin`, in a Homebrew prefix, under `/usr`, in `/nix/store`, or under
  `/mnt` in WSL. It prints the command that owns that binary. A path under
  `/mnt` is on DrvFs, which drops the execute bit and gives no atomic rename.
- **Code.** A new crate `op-update` holds the download, the digest check, and
  the replacement. `op-cli` calls it. `reqwest` gains `rustls-tls-native-roots`,
  so a machine behind a TLS proxy still works. Cargo unifies features, so the
  daemon links TLS as well.

## Constraints

- The app binary can be the daemon. `op-gui/src/lib.rs` calls `Control::ensure`,
  and `Control::spawn_detached` starts `current_exe`. A replaced bundle under a
  running daemon leaves the old code serving, so the order above stops the
  daemon first.
- The daemon stops through the HTTP `/shutdown` path that `Control::stop`
  already uses. It sends a signal only when that path fails.
- A browser tab keeps the old SPA until the user reloads it.

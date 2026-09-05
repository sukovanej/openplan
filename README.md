# openplan

Local-first, file-based task manager for humans and AI agents, in plain markdown.
Design and work items in [.plan/tasks/](.plan/tasks/).

## Install

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/sukovanej/openplan/releases/latest/download/openplan-installer.sh | sh
```

The installer puts `openplan` in `~/.local/bin` and adds that directory to your shell profile.
`OPENPLAN_INSTALL_DIR` picks another directory, and `OPENPLAN_NO_MODIFY_PATH=1` keeps the
profile untouched. Releases carry binaries for macOS (Apple silicon and Intel) and Linux
(x86_64 and arm64). Windows is not supported yet. Every archive and its checksum is on the
[releases page](https://github.com/sukovanej/openplan/releases).

The desktop app is a separate download on the same release: `OpenPlan_<version>_<arch>.dmg` for
macOS, `.deb` or `.AppImage` for Linux. It carries the daemon, so it needs no `openplan` on `PATH`.
It is signed ad-hoc, not with a Developer ID, so macOS asks you to confirm the first open.

In GitHub Actions:

```yaml
- uses: sukovanej/openplan/.github/actions/setup@v0.0.1
  with:
    version: 0.0.1   # omit for the latest release
- run: openplan lint
```

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
Live development: `pnpm --filter @openplan/app dev` (Vite on :5173, proxying the API to
the daemon on :7373).

## Run

`openplan` is the single binary. Put it on PATH and start its daemon on that build:

```sh
mise run install     # SPA → release binary → PATH → daemon restarted on it
```

The daemon respawns itself from its own executable, so the binary that starts it is the one that
keeps serving. Installing and restarting together is what keeps the daemon and the checkout the
same build.

Without installing, run it from the checkout as `cargo run -p openplan -- <args>`:

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
after a change to the SPA. The window starts its own daemon when none runs, so it needs no
`openplan` on `PATH`; it obeys `OPENPLAN_HOME` and `OPENPLAN_PORT` like every other command.

### Icons

`assets/icon.svg` is the only source. Edit it, then rasterize:

```sh
mise run icons   # → crates/op-gui/icons/ and web/packages/app/public/
```

## Release

The product version is `version` in `[workspace.package]`, and it follows
[semver](https://semver.org). Every crate takes it, and `openplan --version` prints it.
`mise` installs `cargo-dist` and `cargo-edit` from `[tools]` in `mise.toml`.

`CHANGELOG.md` follows [Keep a Changelog](https://keepachangelog.com). Write each section by
hand. cargo-dist takes the GitHub Release notes from the section for the version, so this file
is what users read on the release page.

```md
## [0.0.2] - 2026-09-10

### Added
- The lines you want users to read.
```

Then bump on a branch:

```sh
mise run release 0.0.2       # bump the version, commit
```

The task stops when `CHANGELOG.md` has no section for the version. Merge that commit into
`main`, then tag it:

```sh
git tag v0.0.2 && git push origin v0.0.2
```

The tag starts `.github/workflows/release.yml`. It builds each target, makes the archives, the
checksums, and the installer, and publishes a GitHub Release.

[cargo-dist](https://axodotdev.github.io/cargo-dist/) generates that workflow from
`[workspace.metadata.dist]` in `Cargo.toml`. After a change there, run `dist init --yes` and
commit the result.

The desktop app ships as a bundle, not as a binary in a tarball, so `crates/op-gui` sets
`dist = false` and cargo-dist skips it. `.github/workflows/release-app.yml` follows the Release
workflow, builds the bundle on each platform, and uploads it to the same release. Nothing
generates that file. Edit it by hand.

## License

MIT. See [LICENSE](LICENSE).

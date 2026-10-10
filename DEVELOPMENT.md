# Development

## Build

```sh
cargo build
cargo test
cargo fmt --check
cargo clippy -- -D warnings
```

The web UI is in `web/` (a pnpm workspace). Its build output (`web/packages/app/dist/`) is
gitignored. Build it before `cargo build` to embed the SPA. Without it, the daemon compiles and
runs, but serves no web UI.

```sh
cd web && pnpm install && pnpm -r build   # → web/packages/app/dist
cargo build                               # embeds the SPA
```

Web workspace checks: `pnpm -r typecheck`, `pnpm lint`, `pnpm format:check`, `pnpm -r test`.
Live development: `pnpm --filter @openplan/app dev` (Vite on :5173, proxying the API to
the daemon on :7373).

## Run

Put the binary of the checkout on PATH and restart the daemon on it:

```sh
mise run install     # SPA → release binary → PATH → daemon restarted on it
```

The daemon respawns itself from its own executable, so the binary that starts it keeps serving.
Install and restart together to keep the daemon and the checkout on the same build.

Without installing, run it from the checkout as `cargo run -p openplan -- <args>`.

## Desktop window

```sh
mise run gui     # the window on the running daemon, starting one when none runs
```

It loads `http://127.0.0.1:<port>/`, so it shows the SPA the daemon serves. Run `mise run install`
after a change to the SPA. The window starts its own daemon when none runs, so it needs no
`openplan` on `PATH`. It obeys `OPENPLAN_HOME` and `OPENPLAN_PORT` like every other command.
Releases carry no desktop app for now.

On Windows, install the GUI and run the CLI and daemon in WSL instead. With the daemon already
listening in WSL, open the Windows app and it connects through WSL's localhost forwarding. It
waits up to five seconds for `http://127.0.0.1:7373/health`, then tells you if the bridge is not
available. Set `OPENPLAN_PORT` before launching the app when the WSL daemon uses a fixed port
other than 7373. Port `0` is not supported because its randomly selected port stays in WSL.

## Agent skills

`skills/` holds the agent skills. `npx skills add sukovanej/openplan` installs them from there.
This checkout keeps an installed copy in `.agents/skills/` and `skills-lock.json`. After you change
a skill, install it again:

```sh
mise run skills  # skills/ → .agents/skills/, with a symlink in .claude/skills/
```

## Icons

`assets/icon.svg` is the only source. Edit it, then rasterize:

```sh
mise run icons   # → crates/op-gui/icons/ and web/packages/app/public/
```

## Store formats

`format` in the `config.toml` of each store tells which layout its files have. A store without
the key is format 1. `FORMATS` in `crates/op-tracker/src/format.rs` lists the formats that this
binary knows, oldest first.

To change the layout, add a format to the end of `FORMATS`:

- Set `migrate` to a function that rewrites a store of the format before it. The function gets
  the whole store and returns the writes. Give it no time, no random value, and no version: two
  daemons that migrate the same store must write the same bytes, or their sync conflicts.
- Set `released: None`. `mise run release` puts the version there.
- Add a store of the old format to the tests, and check what the migration makes of it.

A daemon migrates a store by itself only when a release reads the new format. Until then, a
canary or a source build serves an older store read-only, and `openplan migrate` migrates it.

To stop migrating from the oldest format, remove its entry, remove `migrate` from the next one,
and add the oldest format to `retired` with the last release that migrates it.

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
`dist = false` and cargo-dist skips it. `.github/workflows/release-app.yml` builds the bundle on
each platform and uploads it to a release. For now, it runs only when you start it by hand. Nothing
generates that file. Edit it by hand.

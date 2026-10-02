# openplan

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/screenshot-dark.png">
  <img alt="The task page of the openplan web UI" src="assets/screenshot-light.png">
</picture>

Local-first task manager for humans and AI agents. Each task is a markdown file. A team keeps
its tasks in the git repository of the code, in the ref `refs/openplan/tasks`, apart from the
code branches. One person can keep them in a local directory instead.

## Features

- **Tasks in plain markdown.** The front matter holds the status, the parent, the dependencies,
  the tags, and the linked pull requests. The body is free markdown.
- **Sync through git.** The daemon pushes and pulls the tasks ref every 30 seconds and soon after
  each write. A team needs no server other than its git remote.
- **One daemon for every project.** The CLI and the web UI both talk to it, so they always show
  the same data. It starts by itself on the first command.
- **Realtime web UI.** It has a task list, a task page, docs, an activity feed, tags, and a flow
  graph of the open tasks. A change from the CLI or a teammate shows at once.
- **Docs next to the tasks.** Write design notes as docs, nest them, and link them from tasks
  with `[[name]]`.
- **Diagrams.** A Mermaid block (flowchart, sequence, or ER diagram) in a task or doc shows as a
  diagram.
- **Full history.** Each write is a revision. `openplan history` shows who changed what.
- **Agent skills.** `openplan setup-skills` installs skills for Claude Code and Codex. With them,
  an agent reads and writes the tasks through the CLI.
- **Pull requests.** Link GitHub pull requests and GitLab merge requests to a task.

## Install

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/sukovanej/openplan/releases/latest/download/openplan-installer.sh | sh
```

The installer puts `openplan` in `~/.local/bin` and adds that directory to your shell profile.
`OPENPLAN_INSTALL_DIR` picks another directory, and `OPENPLAN_NO_MODIFY_PATH=1` keeps the
profile untouched. Releases carry binaries for macOS (Apple silicon and Intel) and Linux
(x86_64 and arm64). Every archive and its checksum is on the
[releases page](https://github.com/sukovanej/openplan/releases). `openplan update` installs the
newest release.

## Quick start

In the root of a git repository:

```sh
openplan init --abbreviation ABC     # task keys start with ABC, as in ABC-1
openplan setup-skills                # let Claude Code and Codex use the tasks
openplan tasks create "Ship the login page" --status todo
openplan tasks set ABC-1 status in_progress
openplan open                        # the web UI in your browser
```

To join the tasks that a teammate already pushed, clone the repository and run `openplan init`
with no abbreviation.

## Usage

```sh
openplan tasks list                  # the tasks of this project
openplan tasks get ABC-1             # the whole task file
openplan tasks comment ABC-1 "..."   # add a comment
openplan tasks tree ABC-1            # the subtasks of a task
openplan doc create storage          # a new doc
openplan history ABC-1               # who changed a task, and when
openplan sync                        # exchange the tasks with the remote now
openplan lint                        # find problems in the tasks, docs, and skills
openplan server start | stop         # the background daemon on 127.0.0.1:7373
openplan project list                # the projects the daemon serves
```

`openplan <command> --help` shows all options.

Every task command goes through the daemon and starts it if it is down. `lint` is the exception.
It reads the tasks itself and never starts a daemon. The first command from a project registers
it with the daemon. `OPENPLAN_HOME` sets the state directory of the daemon (default `~/.plan`).
`OPENPLAN_PORT` sets its port (default 7373).

`openplan migrate` moves the tasks of a repository that keeps them in `.plan/` beside the code
into the tasks ref.

### GitHub Actions

```yaml
- uses: sukovanej/openplan/.github/actions/setup@v0.0.1
  with:
    version: 0.0.1   # omit for the latest release
- run: openplan lint --skills   # the agent skills; the tasks change apart from the code
```

## Development

### Build

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

### Run

Put the binary of the checkout on PATH and restart the daemon on it:

```sh
mise run install     # SPA → release binary → PATH → daemon restarted on it
```

The daemon respawns itself from its own executable, so the binary that starts it keeps serving.
Install and restart together to keep the daemon and the checkout on the same build.

Without installing, run it from the checkout as `cargo run -p openplan -- <args>`.

### Desktop window

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

### Icons

`assets/icon.svg` is the only source. Edit it, then rasterize:

```sh
mise run icons   # → crates/op-gui/icons/ and web/packages/app/public/
```

### Release

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

## License

MIT. See [LICENSE](LICENSE).

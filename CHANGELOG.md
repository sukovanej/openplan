# Changelog

All notable changes to openplan are in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased](https://github.com/sukovanej/openplan/compare/v0.0.3...main)

### Changed

- The agent skills teach openplan only, and do not set a code workflow. The
  `task-management-merge` skill settles the task status and lets the
  repository's own process do the merge. It does not require a worktree, a
  squash merge, or `gh`, and it does not delete the branch or sync main.

## [0.0.3](https://github.com/sukovanej/openplan/compare/v0.0.2...v0.0.3) - 2026-09-25

### Added

- A team shares one set of tasks. A git project keeps its tasks in the git ref
  `refs/openplan/tasks`, apart from the code. The ref is not a branch, so
  GitHub shows no branch for it and offers no pull request. Every daemon syncs
  the ref with the remote every 30 seconds and 2 seconds after each write. Sync
  merges with a merge commit. It never rebases and never force-pushes.
- Sync merges two changes to one task by field and by line. Tags,
  dependencies, and comments merge as sets. When two people change the same
  field or the same lines differently, the task keeps both versions as a git
  conflict block, and the published version is in force until someone picks:
  - The web UI shows each conflict, with buttons to keep one version, both
    versions, or new text.
  - `openplan get` warns about conflicts, `openplan list` marks them, and
    `openplan lint` reports them.
  - The API gives a field a `conflict` state and each task a `conflicts`
    count. `POST /api/projects/{project}/tasks/{id}/resolve` settles a block
    in the body.
- Two tasks created at the same time under one number both stay. The later
  one gets the next free number, and the merge revision says so.
- The daemon finds the problems in the tasks after every change: a reference to
  a task that does not exist, a parent or dependency cycle, a tag that is not
  registered, a field or a comment log that does not parse, a missing or second
  title, and two files with one number. The API gives each task a `problems`
  list, the web UI shows it, and `openplan list`, `show`, and `get` point the
  problems out.
- One person can keep the tasks in a local `.plan/` directory instead, with a
  history in `.plan/.history.sqlite`. An edit made by hand becomes a revision
  too.
- Every write is a revision with an author, an agent, a time, and a message.
  `openplan history [key]` lists the revisions, and `openplan get <key>
  --revision <id>` prints a task as it stood then. The web UI shows the history
  of a task, a task at a revision, and the activity of a project.
- `openplan init --abbreviation <ABC>` starts the tasks of a project. In a
  clone, the first `openplan` command fetches the tasks that a teammate already
  pushed. `openplan init` without `--abbreviation` does the same.
- `openplan migrate` moves a `.plan/` directory beside the code into the tasks
  ref, with its whole history and its authors.
- `openplan write <key> --file <path>` replaces a whole task file, as
  `openplan get` prints it.
- `openplan sync` syncs now, and `openplan sync --status` tells how the last
  sync went. The web UI shows the sync state in its header.
- A web UI that reconnects gets the changes it missed, not only a full reload.
- A click on a `d2` diagram in the web UI opens it over the whole window. Esc
  or the close button closes it.

### Changed

- The tasks are no longer files in the checkout. Read and write them with the
  `openplan` CLI or the web UI. A task write needs no worktree and no commit.
- A write through the daemon carries its author and its agent, so each
  revision names who made it.
- `openplan lint` reports the problems that the daemon finds, the conflicts,
  and agent skill files that differ from the binary. It takes task keys, not
  file paths. `openplan lint --skills` checks only the skill files.
- A `d2` diagram wider than the task column shrinks to fit it. It no longer
  scrolls sideways.

### Removed

- Branch-aware tasks: the task and branch matrix, `--branch`,
  `--all-branches`, `openplan branches`, and the branch badges and switcher in
  the web UI. A task now has one version.
- Rolling updates and `openplan publish`. Sync replaces them.
- `openplan merge-driver`. Sync merges the tasks itself.
- `openplan lint --fix`, the check of links from a task into the source code,
  and the checks of tag files. The write path now keeps references canonical
  and tag files valid.

## [0.0.2](https://github.com/sukovanej/openplan/compare/v0.0.1...v0.0.2) - 2026-09-11

### Added

- Rolling updates. An edit that belongs to no feature branch now lands on
  `openplan/rolling-updates` instead of whatever branch you happen to stand on.
  The daemon commits it and keeps the branch rebased on the default branch.
  Publishing stays yours: it pushes the branch and opens a pull request.
- A header control in the web UI. It counts what waits to be published, lists it
  when you click, and publishes from there. When a rebase hits a conflict, it
  names the files and the commands that fix them.
- `openplan publish`, with `--dry-run` to read the list first.
- `--branch` on `create`, `set`, and `delete`.
- The Windows desktop app. The release page carries an `.msi` and a
  `-setup.exe` for Windows x64. The installer carries the WebView2
  bootstrapper, so it needs no separate download. The app connects to a daemon
  you run in WSL; it starts no daemon of its own.
- Default tags. A store with no tag registry gets `bug`, `feature`, and `draft`
  when it takes its first task, so a new project can tag that task without
  registering a name first. A registry that already exists stays as it is.
- `openplan update`. It reads the newest GitHub release, checks the published
  SHA-256 of each download, and replaces the CLI and `OpenPlan.app`. It quits
  the app and stops the daemon first, then starts the daemon from the new
  binary and opens the app again when it ran before. It refuses a binary that
  cargo, Homebrew, Nix, a system package, or a Windows drive in WSL owns and
  prints the command that updates it instead. Nothing checks for a new version
  on its own.
- Agent sessions. The daemon runs a coding agent (Claude Code or Codex) that
  writes tasks in the rolling-updates worktree. The web UI gets an agent page
  where you start a session, read its output, and send it messages.
- `d2` fenced code blocks in a task body render as diagrams in the web UI. Dark
  mode gets its own theme. A compile error keeps the source and prints the d2
  message above it.
- The review popover shows the diff of a pending rolling update and can discard
  one update or all of them.
- The status of a task changes from the board and the task view. Click the
  status mark or press `s`. The subtask box moves from `s` to `a`.
- The release page carries `OpenPlan-<target>.app.tar.gz` for macOS and a
  `.sha256` next to every app artifact.

### Changed

- The web UI uses SF Pro for the interface and Menlo for code. It bundled Geist
  before.
- The header shows the daemon connection as a colored dot with a tooltip. It
  showed the words "daemon up" or "daemon down" before.
- The CLI archive on the release page is a `.tar.gz`. It was a `.tar.xz`.

- `openplan merge-driver` merges by frontmatter field and by markdown section.
  It conflicted on any difference before.
- Search order. Tasks with the most recent changes come first.
- The `task-management` skill. The agent creates a task only when the user asks
  for one, in those words. A request to do work made a task before this change.
- The `task-management` skill. The agent tags each task it creates. It takes the
  names from `openplan tag list` and registers none of its own.

### Fixed

- `openplan open` now opens the web UI in the Windows default browser when run
  from WSL, including minimal distributions without `xdg-open`.
- `OPENPLAN_PORT` with a value that is not a port number now stops the command.
  The CLI, the daemon, and the Windows app used port 7373 instead.
- The task reference chip in prose clipped descenders. It has a taller line
  box now.
- The desktop app carried no bundle signature, only the ad-hoc signature the
  linker puts on every arm64 binary. macOS called a downloaded copy "damaged"
  and offered only the Trash. Tauri now signs the app before it makes the dmg,
  so macOS gives the usual unidentified-developer dialog instead.

## [0.0.1](https://github.com/sukovanej/openplan/releases/tag/v0.0.1) - 2026-08-27

The first release.

### Added

- The `openplan` binary. It creates, lists, searches, shows, moves, tags,
  comments on, and deletes the tasks in `.plan/tasks/`.
- A background daemon. It serves a realtime API and the web UI on
  `127.0.0.1:7373`. One daemon serves every repository on the machine.
- The web UI. It shows the task list, the task detail with editable markdown
  sections, dependencies, tags, comments, and the task-by-branch matrix.
- Branch-aware reads. `search` and the matrix find tasks on every local branch
  and worktree.
- `openplan lint`. It checks frontmatter, references, cycles, and duplicate
  numbers, and it never starts a daemon.
- `openplan merge-driver`. Git merges `.plan/**.md` files with it.
- `openplan setup-skills`. It installs the agent skills in a repository.
- Binaries for macOS (Apple silicon and Intel) and Linux (x86_64 and arm64),
  with a one-line installer.
- The desktop app. It opens one window on the daemon's web UI, and it starts a
  daemon out of itself when none runs. A dmg for each macOS target and a deb for
  each Linux target. There is no arm64 AppImage yet.

### Known problems

- The desktop app has no valid signature, and macOS calls it damaged. Run
  `xattr -cr /Applications/OpenPlan.app` after you copy it in. The next release
  fixes this.

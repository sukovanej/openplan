# Changelog

All notable changes to openplan are in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

- The desktop app is not signed. macOS shows a Gatekeeper warning until the app
  carries a Developer ID certificate. Open it from the Finder context menu the
  first time.

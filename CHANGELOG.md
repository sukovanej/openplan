# Changelog

All notable changes to openplan are in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased](https://github.com/sukovanej/openplan/compare/v0.0.1...main)

### Added

- Default tags. A store with no tag registry gets `bug`, `feature`, and `draft`
  when it takes its first task, so a new project can tag that task without
  registering a name first. A registry that already exists stays as it is.

### Changed

- Search order. Tasks with the most recent changes come first.
- The `task-management` skill. The agent creates a task only when the user asks
  for one, in those words. A request to do work made a task before this change.
- The `task-management` skill. The agent tags each task it creates. It takes the
  names from `openplan tag list` and registers none of its own.

### Fixed

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

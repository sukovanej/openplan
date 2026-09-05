---
status: done
created: 2026-09-05T10:22:41Z
dependencies:
- ./00093-release-process-versions-changel.md
- ./00104-desktop-app-a-tauri-shell-for-th.md
---
# Ship the desktop app in the release

## Purpose

OPP-93 releases the `openplan` binary from one tag. `crates/op-gui` sets `dist = false`, because
cargo-dist makes tarballs and a Tauri app is a bundle. This task puts the bundles on the same
release. It must not write a second release pipeline.

## Why cargo-dist cannot do it

`[[workspace.metadata.dist.extra-artifacts]]` does attach a file to the release. `dist generate`
puts the build command in `build-global-artifacts`, which is fixed to `ubuntu-22.04`. That runner
cannot make a `.dmg`, so the app needs a workflow of its own.

## The workflow

Add `.github/workflows/release-app.yml`, started by `workflow_run` on the `Release` workflow
finishing. Do not use `on: release`: cargo-dist makes the release with `GITHUB_TOKEN`, and a
release that token makes starts no further workflow.

The job checks out `github.event.workflow_run.head_sha` and takes the tag from
`github.event.workflow_run.head_branch`, which holds the tag name for a tag push. It stops when the
Release workflow failed, and when the head branch is not a version tag.

One job for each platform builds the SPA, runs `cargo tauri build`, and uploads the bundle with
`gh release upload` to that tag's release. Mirror the four runners that `dist plan` reports, so no
job cross-compiles:

| runner | target | bundles |
| --- | --- | --- |
| `macos-14` | `aarch64-apple-darwin` | `dmg` |
| `macos-15-intel` | `x86_64-apple-darwin` | `dmg` |
| `ubuntu-22.04` | `x86_64-unknown-linux-gnu` | `deb`, `appimage` |
| `ubuntu-22.04-arm` | `aarch64-unknown-linux-gnu` | `deb` |

The Linux jobs install the same WebKitGTK 4.1 packages the CI `rust` job installs. Every job builds
`web/packages/app` first, because `op-server` embeds `web/packages/app/dist` at compile time.

Do not ship an arm64 AppImage until a job proves the bundler makes one. Say so in `CHANGELOG.md`
instead of shipping an x64 file under an arm64 name.

## The version

`crates/op-gui/tauri.conf.json` declares no `version`, so Tauri reads the crate version, which is
`[workspace.package] version`. `mise run release` already moves that. Nothing else to keep in step.

## No bundled CLI

OPP-104 made `openplan-gui` link `op-daemon` and start a daemon by a re-exec of itself. The bundle
therefore needs no `openplan` binary inside it, and the app needs none on `PATH`. A person who wants
the CLI runs the installer that OPP-93 publishes.

## Signing is not in this task

macOS shows a Gatekeeper warning until the app is signed with a Developer ID certificate and
notarised. That needs an Apple Developer account at 99 USD each year. Say so in `CHANGELOG.md` and
open a task when you buy the account.

## Acceptance

- A tag produces everything OPP-93 defines, plus a dmg for each macOS target and a deb for each
  Linux target, on the same release.
- `dist generate --check` reports no change to `release.yml`.
- A downloaded dmg opens on a mac with no `openplan` on `PATH`, starts a daemon, and shows the
  board.
- A failed Release workflow uploads no bundle.

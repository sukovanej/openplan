---
status: todo
created: 2026-09-06T11:43:22Z
dependencies:
- ./00106-port-the-cli-and-daemon-to-windo.md
tags:
- feature
- ui
---
# Ship the Windows desktop app

Bundle `openplan-gui` for Windows and put the installer on the release page.

## Design

- **Bundle.** `cargo tauri build` makes an MSI through WiX and an installer
  through NSIS. Ship the NSIS one. It installs per user, so it needs no
  administrator rights.
- **WebView2.** The window needs the WebView2 runtime. The installer must carry
  the bootstrapper.
- **Workflow.** Add a `windows-latest` row to `release-app.yml`. The row builds
  the web SPA first, like every other row, because `op-server` embeds the SPA at
  compile time.
- **Signing.** The build has no certificate, so SmartScreen warns on first run.
  The macOS bundle signs ad hoc today, so this matches what ships now.
- **Icons.** `mise run icons` writes no `.ico`. Add one, and add it to
  `tauri.conf.json`.

## Constraints

- The app starts the daemon from its own executable, so the daemon must run on
  Windows first.

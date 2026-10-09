# Changelog

All notable changes to openplan are in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased](https://github.com/sukovanej/openplan/compare/v0.0.7...main)

### Added

- Create a task in the web UI. Press `c`, click **New task** in the header, or
  run "Create a task" from the command palette. The dialog takes a title, a
  description, tags, and a project. `⌘⏎` (`Ctrl+⏎` on Linux and Windows)
  creates the task. `⇧⌘⏎` creates it and keeps the dialog open for the next
  task, with the same project and tags. The dialog keeps the draft when you
  close it.

### Changed

- The shortcut badges use the UI font. On a Mac they show a chord the way macOS
  does, for example `⇧⌘⏎`.

## [0.0.7](https://github.com/sukovanej/openplan/compare/v0.0.6...v0.0.7) - 2026-10-08

### Added

- `--json` on `openplan tasks show`, `openplan project list`, `openplan url`,
  `openplan tag colors`, and `openplan server ping`.
- The OpenAPI spec documents `GET /api/events`. The response is
  `text/event-stream`, and each event has the `ChangeEvent` schema.

### Changed

- `npx skills add sukovanej/openplan` installs the agent skills. It installs
  them for Claude Code, Codex, and each other agent that
  [skills](https://github.com/vercel-labs/skills) supports. Run
  `npx skills update` to update them.
- A build from source needs Rust 1.99 or later.

### Removed

- `openplan setup-skills`. Use `npx skills add sukovanej/openplan`.
- The skill check of `openplan lint` and its `--skills` option.
- Agent sessions. The daemon no longer runs a coding agent, and
  `/api/projects/{project}/agent/sessions` and its routes are gone. A new
  release now installs at once, because the daemon no longer waits for a
  session to end.

### Fixed

- The history of one task or doc shows the change on the same line as the
  author. Before, the doc sidebar put "Edited" on a new line.

## [0.0.6](https://github.com/sukovanej/openplan/compare/v0.0.5...v0.0.6) - 2026-10-01

### Added

- A task records its pull requests and merge requests in the front matter
  field `pull_requests`. `openplan tasks pr add <key> <address>` links one,
  `openplan tasks pr remove` unlinks one, `openplan tasks create --pr` links
  one at creation, and `openplan tasks show` prints each address. A number
  names a pull request in the GitHub or GitLab repository of the project
  remote. The task page has a "Pull requests" section, the task list marks a
  task that has pull requests, and the activity view shows each link. The
  openplan skill tells the agent to link the pull request that it opens.

### Changed

- The Tags page edits a tag in place. A click on the chip or the slug renames
  the tag, and a click on the description changes the description. Enter or a
  click outside the field saves, and Escape cancels. An empty name keeps the
  old name, and an empty description removes the description. The pencil
  button is gone.
- The activity view, the task history, and the revision notice show a
  dependency change as task chips with a + or − sign. Before, they showed the
  keys as text.
- A task reference chip shows the status mark and the key, and no title. A doc
  reference chip shows the icon and the name, and no title. Each chip reads
  its task or doc, so a change to the task or doc updates the chip.
- The HTTP API no longer has `TaskDetail.refs`, `TaskDetail.doc_refs`,
  `DocDetail.refs`, `DocDetail.doc_refs`, or `DocRef`. Read the task or the doc
  of a reference instead.
- The activity view reads older revisions when you scroll near the end of the
  list. The "Show older revisions" button is gone from the activity view. When
  a read fails, the view shows the error and a "Retry" button. The task history
  and the doc history keep the button.
- A diff card that the pointer opens follows the cursor, 16 px below and to
  the right of it. The card closes when the pointer leaves the line. A card
  that the keyboard opens stays at the line.
- The flow no longer starts from a backlog task. It shows a backlog task when a
  shown task needs it, or when you name its status or its key.
- A flow card and a box header put the key and the title on one line when they
  fit.
- The header of the task page no longer has a Flow link. The `f` key still
  opens the flow of the task.

### Fixed

- A graph diagram no longer draws two edges that cross when a swap of two
  neighboring nodes removes the crossing.
- A long task chip no longer runs out of the history column.

## [0.0.5](https://github.com/sukovanej/openplan/compare/v0.0.4...v0.0.5) - 2026-09-29

### Added

- The body editor edits a table as a grid. A click on a cell edits it in
  place, and Tab, Enter, and the arrow keys move between cells. A toolbar adds
  and deletes rows and columns and sets the alignment of a column.
- The openplan-docs skill tells the agent to read and write the project docs
  with `openplan doc` when the user mentions a doc, and to show each doc it
  creates or changes in the built-in browser of the agent app.
- The web UI header shows a warning control when a project has a fault that
  you must fix: no git `user.name`, a failed sync, a project root that is gone,
  tasks that do not parse, or outside changes that the daemon could not read.
  The control lists each fault with its project, and it changes as faults start
  and end. `GET /api/faults` lists the faults, and the `faults_changed` event
  tells a client that the list changed.
- On a phone (narrower than 768 px), the web UI has a tab bar at the bottom
  with Tasks, Docs, Activity, Tags, and Flow. The header is one row. Its search
  button opens the command palette, and its theme button steps through light,
  dark, and system. The edit button of a table or a diagram shows on a touch
  screen.

### Changed

- The task commands are in the `openplan tasks` group: `create`, `list`,
  `search`, `get`, `write`, `comment`, `comments`, `show`, `tree`, `move`,
  `set`, and `delete`. For example, `openplan get OPP-42` is now
  `openplan tasks get OPP-42`. Run `openplan setup-skills` to update the agent
  skills in a checkout.
- The web UI selects the project with a menu in the header. The selection
  applies to the tasks and to the docs. On any page, `0` selects all projects,
  and `1` to `9` select the first nine projects. `o` opens the menu. The docs
  of one project are at `/<project>/docs`, not at `/docs?project=<project>`.
- The header has links with icons to the tasks, the docs, the activity, the
  tags, and the flow of the selected project, and it highlights the page you
  are on. These links replace the Docs and Flow links of the header and the
  Activity and Tags links of the task list.
- `g t` goes to the tasks of the selected project and replaces `g l`. `g d`
  goes to the docs, `g a` goes to the activity, `g l` goes to the tags, and
  `g f` goes to the flow of the selected project.
- The activity and the tags have a page for all projects, at `/activity` and
  `/tags`. The activity of all projects shows the revisions of every project,
  newest first. The tags of all projects show each project in a section of its
  own.
- A new project cannot take the name `activity` or `tags`, as it cannot take
  `docs` or `flow`.
- The web UI shows a keyboard key, such as the keys in the keyboard help, like
  inline code in a task body: red monospace type with a border, and smaller
  than before.
- The keyboard help (`?`) puts its groups in columns when they do not fit the
  height of the window. When the window is too narrow for more columns, the
  help scrolls.
- Every write needs git `user.name`. openplan no longer signs a write with
  `$USER` or with `openplan`. With no name, a write fails and tells you how to
  set one, and a read still works. A local project outside a git repository
  uses the global git config: `git config --global user.name "Your Name"`.
- The HTTP API no longer has `ProjectView.status` or `SyncView.error`. Read
  `GET /api/faults` instead.
- A cylinder node (`[(text)]`) in a Mermaid diagram has a darker lid. The lid
  gets taller as the node gets wider, from 6 to 12 px, so a wide cylinder no
  longer looks flat.
- A diamond node (`{text}`) wraps its label until it is no more than twice as
  wide as it is high. Before, a long label made a flat diamond, such as 293 by
  60 px.
- Diamond, hexagon, lean, trapezoid, and asymmetric nodes have rounded corners.
- An edge ends on the outline of a node where the outline does not fill its
  box: at the rounded tip of a diamond, and at the slanted side of a lean,
  trapezoid, or asymmetric node in a `LR` or `RL` flowchart. Before, a gap
  could stay between the edge and the node.
- The entity tables of an `erDiagram` show the type of an attribute as inline
  code, and the comment in faint italic. A column that no attribute fills
  takes no space.
- The task body and its headings use Open Sans, which the app bundles. Before,
  the body used Source Serif 4 and the headings used the system font.
- The task body has no line above a level 2 heading. The heading sizes follow
  the size of the body text. In the body editor, a nested list item starts
  under the text of its parent item.
- The activity view, the task page, and the comments show the agent as an
  icon. The tooltip of the icon gives the name of the agent.
- The activity view shows a title change as "Title", as it shows a description
  change as "Description". The diff card of the change shows the two titles.
- A tooltip shows when the pointer enters. Before, it showed after 300 ms.
- A task key with the prefix of another project, such as `CQR-97`, gets the
  message `CQR-97 is in another project; this project's keys start with OPP-`.
  Before, the message said that it was not a task key.
- The `reference_path` lint names the field and gives the path to write, for
  example: ``the parent `2` is not a path; write `./00002-two.md` ``.

### Fixed

- A comment, a tag rename, and a sync merge of a task no longer write the
  parent and the dependencies as bare numbers, such as `parent: '1'`. They
  keep the path to each task file, as every other write does. Before, `openplan
  lint` then reported a `reference_path` problem that no person had made.
- An open web UI page reloads when it cannot load a part of the app, such as the
  diff view, after the daemon starts again on a new build of the same version.
  Before, it showed "Failed to fetch dynamically imported module".
- In the body editor, a wrapped line of a list item starts under the text of
  the first line, not under the marker.
- The word mark in a diff stops at the last changed word. Before, it also
  marked the space after the word and the indent before the first word.
- A task or doc reference chip no longer cuts off the bottom of letters such as
  "g" and "y".
- Below 1024 px, the task and doc body no longer collapses to zero height. A
  long task reference chip no longer makes the body editor wider than its
  panel.

## [0.0.4](https://github.com/sukovanej/openplan/compare/v0.0.3...v0.0.4) - 2026-09-26

### Added

- The openplan skill tells the agent to show each task it creates or changes
  in the built-in browser of the Claude desktop app or the Codex app.
  `openplan setup-skills` writes the name of the browser tool that each agent
  uses into the skill of that agent.
- The web UI edits the title and the description of a task in place. The
  description is a markdown editor with a live preview: the markdown shows where
  the caret is, and the rest shows as it renders. `/` inserts a block, `@` or
  `[[` inserts a reference to a task, and `e` on a task page starts the edit.
  The text saves when you leave it, and a draft that did not save comes back on
  the next visit.
- `PUT /api/projects/{project}/tasks/{id}/text` writes the title and the
  description of a task. It takes the text the edit started from and merges the
  edit with what other writers changed since. Where both changed the same lines,
  a conflict block keeps both versions.
- The web UI shows the author of each task in the task list and on the task
  page. The author is the person who wrote the revision that created the task,
  and the task page also shows the agent. A task list row and the task detail
  in the API have an `author` with a `name`, an `email`, and an `agent`.
- Canary builds. Each push to `main` replaces the prerelease `canary` with a
  new build of the CLI and the daemon, versioned `<next patch>-canary.<run>`.
  `openplan update --canary` installs it. `openplan update` goes back to the
  newest stable release, also when that release is older.
- Docs: markdown pages that live with the tasks, in `docs/<name>.md` of the
  same git ref or local directory. A doc has a title, a body, and an optional
  parent doc. `openplan doc` creates, lists, prints, edits, nests, renames, and
  deletes them. The web UI has a docs page across projects (`g d`) and a page
  for each doc. `[[name]]` links to a doc and `[[OPP-42]]` to a task, from a
  task or a doc. Renaming a doc moves every link to it and the docs nested
  under it. Deleting a doc moves the docs nested under it up to its parent. `openplan migrate` brings the docs in `.plan/docs/` along with the
  tasks.
- A file names every other file by its path, relative to its own directory:
  `[[../docs/storage.md]]` from a task, `[[../tasks/00042-ship-login.md]]` from
  a doc, and `parent: ./architecture.md` in a doc. A write turns a key or a doc
  name into that path, and the API and the CLI show the key or the name again.
  `openplan lint` reports a reference that a file spells in another way.
- The doc page edits the title and the body in place with the editor of the
  task page. `PUT /api/projects/{project}/docs/{name}/text` merges the edit
  with what other writers changed since. A new title renames the doc. The `[[`
  menu of the editor offers docs as well as tasks.
- A doc page shows its parent in the header, its nested docs and its history
  in a side column, and the person who created it, as a task page does. "Add
  doc" nests an existing doc or writes a new one, as "Add subtask" does for
  tasks. A revision of a doc opens as that revision left it. The activity view
  shows each doc change with its title and a link to the doc.
- Sync merges a doc as it merges a task: the parent by field, and the body by
  line. When two people change the parent or the same lines differently, the
  doc keeps both versions, and the published version is in force until
  someone picks. `openplan doc get` warns about conflicts, `openplan doc list`
  marks them, and `openplan lint` reports them. When sync gives a task a new
  number because another task took its number first, the links to it in your
  docs follow it.
- `openplan lint` checks docs too: Mermaid diagrams, references to a task or a
  doc that does not exist, and parent cycles. `openplan url` prints the page of
  a doc.
- `GET /api/events` takes the cursor of the last event a client saw in the
  query too (`?last_event_id=<id>`), because a new EventSource cannot set the
  `Last-Event-ID` header. The web UI uses it to resume after a reconnect.
- In the status menu of the web UI, one letter key sets a status and closes the
  menu: `b` backlog, `t` todo, `p` in progress, `r` in review, `d` done, and
  `c` cancelled. The menu shows the letter next to each status.
- The sync state of a project (`GET /api/projects/{project}/sync`, and `sync`
  in the project list) has a `syncing` flag. The daemon sends a `sync_changed`
  event when a sync starts and another when it ends. The web UI spins the sync
  icon while a sync runs.
- `mermaid` fenced code blocks in a task body render as diagrams in the web UI.
  The daemon draws them (`POST /api/diagram`) with its own layout engine: a
  subset of `flowchart`, `sequenceDiagram`, and `erDiagram`. A block that does
  not parse shows the message and marks the line.
- `openplan lint` reports a `mermaid` block that does not parse, in a task body
  or in a comment, with its line and column in the task file. The daemon and
  the web UI show it as a task problem (`diagram`).
- The daemon updates itself. It checks for a new release 5 minutes after it
  starts and then every hour. A canary build follows the canary release, and
  any other build follows the newest stable release. The daemon downloads and
  verifies the release, waits until no agent session runs, replaces its
  executable, and starts again on it in the same process, with the same pid and
  port. A request to start an agent session after that stop gets 503. The
  daemon does not update a binary that a package manager owns, a build in a
  directory with a `CACHEDIR.TAG` (cargo's `target/`), or a daemon that the
  desktop app runs.
- `openplan update --auto on|off` turns the updates of the daemon on or off.
  The default is on. The setting and the result of the last check are in
  `OPENPLAN_HOME/update.json`, and `openplan server ping` prints them.
- The web UI reloads the page when the daemon runs a new version after a
  reconnect, so the tab gets the new web app. When the page holds an open
  dialog or typed text, it shows "New version" with a Reload button instead.
- `openplan url <key>...` prints the address of each task page in the web UI.
  The openplan skill tells the agent to write each task key in a reply as a
  link to that page.
- In the activity view and in the history of a task, a change line opens a
  card with the diff of the change when the pointer is on the line or when
  the line has the keyboard focus.
  `GET /api/projects/{project}/revisions/{revision}/diff?path=<path>` gives
  the unified diff of one document against the first parent of the
  revision. The diff stops at 400 lines, and a binary document has no diff.

### Changed

- `TaskDetail` and revision snapshots carry `description` in place of `body`:
  the text without the `# ` title line, with task references as keys.
  `openplan get --json` shows the same.
- The `daemon_stopping` event has a `reason`: `stop` or `update`. On `update`,
  the web UI shows "Updating" and connects again at once.
- `openplan update` refuses a binary in a directory with a `CACHEDIR.TAG`, such
  as a `cargo build` or `cargo run` build in `target/`.
- A release carries only the CLI and the daemon for now. It carries no desktop
  app. The app workflow runs only when you start it by hand.
- `openplan update` skips `OpenPlan.app` when the release carries no app
  bundle. The app keeps its version. It failed before.
- The history reads what each revision changed from the task files, not from
  the commit message. `openplan history` and the web UI show the same result
  for every revision, also for a revision that plain git or an import wrote.
  Examples: `OPP-114: status → in_review, description`, a new title, tags
  added or removed, new comments, a task that a sync moved to a new number, and
  a renamed tag or doc. The history API gives each entry a `summary` (one line for each
  document), `tasks` (the changed fields of each task), and `tags`. openplan
  writes its commit messages from the same description, so `git log
  openplan/tasks` agrees with `openplan history`.
- One `openplan` agent skill replaces the `task-management`, `task-comments`,
  and `task-management-merge` skills. `openplan setup-skills` removes the three
  old skills, and `openplan lint --skills` reports an old skill that stays.
- The agent skill teaches openplan only, and does not set a code workflow. At a
  merge, it settles the task status and lets the repository's own process do
  the merge. It does not require a worktree, a squash merge, or `gh`, and it
  does not delete the branch or sync main.
- The daemon reads only the tasks that changed. Before, each write read every
  task file two times. On 1,500 tasks a write takes 5 ms instead of 33 ms, and
  the history of one task takes 10 ms instead of 231 ms.
- A local `.plan/` project reads a file only when its size, times, or inode
  changed, and keeps large documents, such as images, out of memory until a
  reader asks for them.
- The web UI refreshes each read once for a burst of changes, such as a sync
  that brings in many tasks. A key press on a big board renders only the rows
  that change.
- The CLI help shows the permitted values of `--status` on `create` and `list`,
  and of `--color` on `tag create`. Shell completion offers them too.
- The sync popover of the web UI shows one line for each project: the name,
  the time of the last successful sync, and "Sync now". A warning icon marks a
  failed sync, and its tooltip gives the error. The header button and "Sync
  now" use the same icon and the same state color.
- Each relative time in the web UI updates while it is on screen. Below one
  minute, it counts in steps of ten seconds: "just now", "10 seconds ago",
  "20 seconds ago", and so on.
- The flow page shows an SVG that the daemon draws (`GET /api/flow/drawing`)
  for the size of the page. It asks for a new drawing after a resize. A
  two-finger scroll or the wheel pans a diagram, and a pinch or Ctrl with the
  wheel zooms it. A click on a card opens its task without a reload.
- The web app is smaller: its assets are 2.3 MB instead of 12 MB.

### Removed

- `POST /api/projects/{project}/tasks/{id}/resolve`. To settle a conflict
  block, replace it in the description and write the text with
  `PUT /api/projects/{project}/tasks/{id}/text`.
- `d2` fenced code blocks no longer render as diagrams. They show as code. Use
  `mermaid`.
- `GET /api/flow`. `GET /api/flow/drawing` takes the same query.

### Fixed

- A Mermaid self-loop (`a -->|text| a`) shows its label beside the loop. Two
  loops on one node no longer share one path. A loop on the last node of a row
  stays inside its subgraph and inside the drawing.
- `openplan lint` reports a link between a node and a subgraph that holds it,
  and a link from a subgraph to itself. The diagram drew such a link through
  its own node and dropped its label.
- Two or more commands that open a new local project at the same time now all
  work. Before, a command could fail with "database is locked". A command could
  also delete a file of the project from the disk, such as `.plan/config.toml`,
  when another command recorded that file at the same time.
- An open task page did not show a change to a task around it, such as a new
  subtask from the CLI or from a sync, until a reload. A task page and a doc
  page now read again after each change to a task or a doc in their project.
- Enter in a search box, such as "Add subtask" or "Change parent", right after
  typing picked an option for the text before the last keys. It now picks the
  first option for the text in the box.
- A project named `flow` had no board, because the flow page has that
  address. A new project does not take the name `api`, `docs`, or `flow`: it
  takes `docs-2`, for example. A rename to one of these names is refused.

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

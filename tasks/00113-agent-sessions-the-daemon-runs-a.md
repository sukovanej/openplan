---
status: done
created: 2026-09-10T00:15:45Z
dependencies:
- ./00109-rolling-updates-a-branch-with-a.md
tags:
- daemon
- feature
---
# Agent sessions: the daemon runs a coding agent that writes tasks on the rolling-updates branch

A person opens a task page, writes what they want, and a coding agent writes
the task file while they watch. This task gives the daemon the agent session
that page talks to, on top of the agent crates from PR #141 (`op-agent`,
`op-agent-claude`, `op-agent-codex`). The page itself is
[[./00114-ui-the-agent-page-a-chat-beside-t.md]].

The agent crates already give one interface for a session: `Agent::start`
spawns the CLI once, `SessionHandle` sends a prompt, an interrupt, an approval,
or a shutdown, and one stream of `AgentEvent` reports what the agent does.
`Transcript::apply` folds that stream into the state a reader who joins late
needs. This task adds nothing to those crates. It owns sessions in the daemon,
puts them behind routes, and decides where the agent works.

## Where the agent works

The agent runs in the rolling-updates worktree. Its edits then follow the path
every unpublished edit already follows, from
[[./00109-rolling-updates-a-branch-with-a.md]]: the watcher reports the change
on `openplan/rolling-updates`, the index reads it back at once, the worker
commits it after the quiet window, and a person publishes it from the review
popover. The daemon adds no second write path.

That worktree is a sparse checkout of `.plan`, so the code the agent must read
to design a task is not there. The agent reads it in the serve root, by
absolute path. The session tells the agent both paths.

Three things can change later: the agent could work in a worktree of its own,
or in the primary checkout on the default branch. So the place is one value,
`Workspace { cwd, code_root, branch }`, with one constructor today:
`Workspace::rolling_updates(&Project)`. It sets `cwd` to
`repo.rolling_updates_worktree()`, `code_root` to `project.path`, and `branch`
to `ROLLING_UPDATES_BRANCH`. A dedicated worktree sets `cwd` and `code_root`
to that worktree and `branch` to its branch. The default branch sets both paths
to the serve root. Everything below reads the three fields and nothing else, so
a new place is one constructor and one field on the create request. Do not
add that field now.

## Session

`op-server/src/agent.rs` owns the sessions. `AppState` gains
`agents: Arc<AgentSessions>`, a map from a session id to:

- `id`, `project`, `workspace`, `kind: AgentKind`, `started_at`
- `task: RwLock<Option<String>>`, the task the session works on
- `handle: SessionHandle`, the transcript under a mutex, and one
  `broadcast::Sender<SessionEvent>`

One tokio task per session reads `Session::next_event`, applies each event to
the transcript, and sends `SessionEvent::Agent(event)` on the broadcast. The
same task subscribes to the daemon's `ChangeEvent` channel. While a turn runs,
the first `TaskChanged` on the workspace branch of this project binds `task`
when it is not bound yet, and sends `SessionEvent::Task { id }`. A session that
starts on an existing task is bound from the start. That is how the page learns
which task to preview. A person who creates a task on the branch during the
same seconds binds the wrong task; the branch is the daemon's, so this is rare
and accepted.

```rust
#[serde(tag = "kind", rename_all = "snake_case")]
enum SessionEvent {
    Snapshot(SessionView),
    Agent(AgentEvent),
    Task { id: String },
}
```

`SessionView` is `{ id, project, agent, task, branch, cwd, status, started_at,
transcript: Transcript }`. `Transcript` serialises as it is; the page draws
from it.

The session id is eight random bytes as hex, from `getrandom`. A stale link
after a daemon restart then answers 404 and nothing else.

A session ends in three ways. `DELETE` sends `shutdown` and removes the entry
once `Exited` arrives. The agent exiting on its own keeps the entry, with
`Status::Exited`, so the page can still show the transcript and the reason.
The daemon stopping sends `shutdown` to every session and waits for the
five-second grace the driver already has. Nothing is persisted.

## Starting the agent

The request names the agent kind, `claude_code` by default. The daemon builds:

- Claude Code: `ClaudeCode::new().tools(Tools::Only(["Read", "Grep", "Glob",
  "Bash", "Edit", "Write"])).skills(Skills::Disabled)`.
- Codex: `Codex::new()`.

Both get `SessionOptions::new(workspace.cwd)` with `Permissions::Full`,
`McpPolicy::Disabled`, `Persistence::Ephemeral`, and `instructions`. The
worktree is the daemon's and the agent may only touch `.plan/tasks`, so an
approval prompt on each write would earn nothing; the instructions carry the
rule. The approval route below still exists, so a later
`Permissions::AcceptEdits` needs no new route.

`AppState::new` takes the agents as `BTreeMap<AgentKind, Arc<dyn Agent>>`, so
a test hands in a fake and the daemon hands in the two real ones. Adding
`op-agent`, `op-agent-claude`, and `op-agent-codex` to `op-server` is what
brings the agent crates their first consumer.

The instructions are the appended system prompt. They say, in this order:

- The task files are in `<cwd>/.plan/tasks`. That is the only place to write.
- The code is at `<code_root>`. Read it there. Never write there.
- Create a task with `<current_exe> create "<title>" --tag ... --body-file
  <file>`. It prints the key. Read one with `<current_exe> get <key>`. List
  the registered tags with `<current_exe> tag list` and use only those.
- Edit an existing task by editing its file under `.plan/tasks`. Run
  `<current_exe> lint` after an edit.
- Never run `git`. The daemon commits. Never create a worktree.
- When the session is bound to a task: the key, and that the work is on this
  task alone.
- The writing rules: the repository's `CLAUDE.md` sits at the worktree root,
  so Claude Code loads it; Codex does not, so the instructions repeat the
  Simplified Technical English rule and the one-feature-slice rule.

`current_exe` is the daemon's own binary. The CLI it names resolves the
project through this daemon and writes on the worktree's branch, which is the
workspace branch. Reading and writing through the CLI then needs no `--branch`.

## API

All routes sit under `/api/projects/{project}/agent/sessions`. Every one
answers 503 when the project has no rolling-updates branch, like the
rolling-updates routes, and 404 for a session id the daemon does not hold.

- `GET` lists `[{ id, agent, task, status, started_at }]`, newest first. The
  page uses it to attach to a running session of a task after a reload.
- `POST` with `{ agent?, task?, prompt }` starts a session and sends the first
  prompt. Answers 201 `{ id }`. Answers 404 when `task` names no task on the
  workspace branch. Answers 503 with the spawn error when the binary cannot
  start.
- `GET /{id}/events` is an SSE stream. The first event is
  `Snapshot(SessionView)`, then every live event. A reader who subscribes
  after a hundred events sees the same page as one who saw them all, and no
  read races the subscription. The stream ends with the session, and on the
  daemon's stop, the way `/api/events` does.
- `POST /{id}/prompt` with `{ text }` answers 202. Answers 409 while a turn
  runs; the agent takes one turn at a time, and the page shows Stop instead of
  Send meanwhile.
- `POST /{id}/interrupt` answers 202.
- `POST /{id}/approvals/{approval}` takes an `ApprovalDecision` as the body and
  answers 202. Answers 404 for an approval the transcript does not hold open.
- `DELETE /{id}` answers 204.

The OpenAPI document carries the JSON routes. The events route is registered
beside `/api/events`, outside the document, because an SSE body has no schema
there.

## Accepted costs

- The agent runs with full permissions in a worktree the daemon owns. The
  commit takes `.plan` and `.gitattributes` only, so a stray file elsewhere
  never reaches the branch, but a `git` command the agent runs there can
  disturb the worker. The instructions forbid it; nothing enforces it.
- An edit that lands while the worker rebases makes git refuse the rebase.
  The worker warns and tries again after the next quiet window, as today.
- Two sessions on the same branch see each other's edits. The branch is one
  worktree, so this is the same as two people editing it.
- Codex reports no cost, so the usage line shows tokens alone there.

## Verify

Server tests in `crates/op-server/tests/agent.rs`, with a fake `Agent` whose
`start` returns `op_agent::session::open()` and feeds scripted events:

- `POST` starts the session, sends the prompt as the first command, and the
  snapshot on the events route shows `TurnStarted` with that prompt.
- The events route sends the snapshot first, then the events that arrive
  after the subscription, and a late subscriber's snapshot holds the folded
  entries the early one received as events.
- A `TaskChanged` on the workspace branch during a turn binds the task, sends
  `Task { id }`, and the list shows it. A `TaskChanged` on another branch
  does not. A session started with `task` is bound before its first event.
- A prompt while a turn runs answers 409, and after `TurnEnded` answers 202.
- An interrupt reaches the handle as `Command::Interrupt`.
- An approval decision reaches the handle with the id it named, and an unknown
  id answers 404.
- `DELETE` sends `Shutdown`, and the entry is gone after `Exited`.
- An agent that exits on its own keeps the entry with `Status::Exited`.
- The daemon's stop sends `Shutdown` to every live session.
- A project with no rolling-updates branch answers 503.
- The instructions name the worktree, the code root, the daemon's binary, and
  the task when the session has one.

One live test in the same file, under `#[ignore]`, that starts the real
`claude` binary against a temporary repository with a rolling-updates
worktree, asks for a task, and finds one new file in the worktree's
`.plan/tasks` when the turn ends.

## Comments

### 2026-09-10T00:58:43Z by Milan Suk via claude-code

> `AppState::new` keeps the two real backends and `AppState::with_agents` takes the fake instead, because `new` has 30 call sites and `with_registry`/`with_health` are the house pattern for injection.
>
> `AgentKind` gained `PartialOrd, Ord, Hash` in `op-agent`; `BTreeMap<AgentKind, _>` needs them. That is the only change to the agent crates.
>
> The bound task sits in the same mutex as the transcript, not a separate `RwLock`. Every broadcast send happens under that mutex, so a reader takes its snapshot and subscribes without racing the events the snapshot already holds.
>
> The DTOs live in `op-server`, not `op-api`, so the shared wire crate never depends on `op-agent`.
>
> The live test asks the agent for the file, not for `<binary> create`: the instructions name the running executable, and in a test that executable is the test.
>
> `web/packages/api-client` still has no agent routes. Run `mise run generate-web-client` for [[./00114-ui-the-agent-page-a-chat-beside-t.md]].

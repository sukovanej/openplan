---
status: backlog
created: 2026-07-14T17:30:13Z
tags:
- daemon
- feature
- ui
---
# Presence: show who's working on what, from Claude Code sessions

## Goal

Show, live in the web UI, **who is working on what** — which agent/human session
is actively on each task right now — driven by Claude Code session activity, with
an optional live view of the working session's transcript.

This is the machine-fact coordination channel: "is anyone else on
this task, so I don't collide." It is orthogonal to the git-content signals the
daemon already broadcasts (the branch badges + the "dirty" overlay) — those say
*a file changed*; presence says *a person/agent is here*, true from the moment
they start, before any file is touched.

## Design

### Presence is a machine-fact, not git content

Claims live in the daemon's in-memory registry (`op-presence::Registry`), keyed by
task id, **never** written into `.plan/*.md` frontmatter. Putting session ids in
the task file would push a machine-fact into branch-scoped git content and violate
the reads-global/writes-local invariant. The "sessions" data is a
field on the *presence record / API response*, not on the markdown.

A claim record: `{ session_id, worktree, branch, since, last_active }`. Multiple
sessions may claim the same task (two agents, or a human + agent) — the registry
holds a set per task.

### Binding a session to a task (Claude Code hook adapter)

Ship a `.claude/settings.json` hook adapter (Claude-specific *source* of presence,
layered on a generic API — see below):

- On session activity in a worktree, a `PostToolUse` / `UserPromptSubmit` hook
  POSTs `{ session_id, cwd }` to the daemon (`POST /api/presence`). The daemon
  maps `cwd → worktree → branch → the in_progress task(s) there` and
  records/refreshes the claim. The agent never confirms anything — its normal
  tool activity *is* the heartbeat.
- On `SessionEnd` / `Stop`, the hook releases the claim.

(Hook event names to be confirmed against the installed Claude Code version before
wiring.)

### Liveness from the session transcript files

Each Claude session appends a live transcript at
`~/.claude/projects/<project>/<session-id>.jsonl`; the file's mtime bumps on every
turn. The daemon watches that directory with its existing `notify` watcher: new
content on a session's transcript = that session is alive → refresh its claims.
This observational signal covers gaps between hook pings.

A **reaper** expires claims whose transcript has been quiet past a TTL and whose
session hasn't sent a release — so a crashed agent doesn't hold a task forever.

Coupling note: transcript-watching depends on Claude Code's private
`~/.claude/projects/.../*.jsonl` layout and format, which is not a stable
contract. Isolate it behind the adapter so a format change can't break core
presence.

### Agent-agnostic core + Claude adapter

Keep the primitive generic: a `claim / refresh / release` API keyed by an actor
(`{ session_id | actor, worktree, branch }`) with a TTL, plus `PresenceChanged`
on the SSE stream. The Claude hook + transcript-watching are *one source* of
presence, not its definition — a human editing in their IDE, or a non-Claude
agent, can claim through the same API and still appear in the UI.

### Realtime UI — who's working on what

- New presence indicator per task row: a live dot / avatar showing the active
  session(s), fed by `PresenceChanged` over the existing `/api/events` SSE stream.
- Expose presence to the client — either `GET /api/presence` or enrich
  `TaskListItem` with the active sessions for that task.

### Stretch: live transcript in the UI

Because the daemon can already read a session's transcript file, expose an
endpoint to serve/stream a session's transcript, and a UI panel to watch the live
transcript of whoever is working a task (click the presence dot → see what the
agent is doing). This couples tightly to the Claude transcript schema and raises
size/privacy questions, so it is a distinct layer — land the who's-here signal
first, this second.

## Relationship to other work

- Reuses the SSE broadcast + `notify` watcher infra from
  `emit-change-events-for-all-branches-and-worktrees`.
- Independent of `branch-aware-crud`.
- **Cross-branch collision warning (from `23`).**
  That design adds *ambient* edits routed to the branch `openplan/updates`,
  which are **unclaimed by nature** — a human's quick UI triage edit heartbeats no
  session and lands on a branch nobody has checked out. It creates a new collision
  axis presence doesn't cover today: a human ambient-edits task X while an agent is
  actively claimed on task X on a feature branch → the two collide *later*, at
  the rolling-updates rebase onto main, possibly long after. Consider whether presence
  should warn at *edit* time ("an agent is actively working this task on
  `feat/auth`; your edit may conflict when it merges") instead of letting the
  conflict surface silently at merge. Open: does presence stay branch-local as
  designed, or become cross-branch-aware to surface this?

## Out of scope

- Cross-machine presence (local-first; single machine only).
- Writing presence into git task files.

## Acceptance

- Start a Claude session in a worktree working a task → within seconds that task's
  UI row shows a live "session … working" indicator, no manual refresh.
- Ongoing tool activity keeps it lit; idle past the TTL, or ending the session,
  clears it (reaper handles a crash with no release).
- Two sessions on the same task both appear.
- (Stretch) clicking the indicator shows that session's live transcript.

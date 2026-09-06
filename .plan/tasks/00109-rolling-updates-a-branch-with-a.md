---
status: in_review
created: 2026-09-06T12:31:28Z
parent: ./00039-continuous-changes-accumulation-v.md
tags:
- cli
- daemon
- feature
- git
---
# Rolling updates: a branch with a standing worktree for task edits (backend + CLI)

Some task edits belong to no feature branch: a
reprioritisation, a rewritten description, a new subtask, any edit made in the
global UI view. Today such an edit lands on whichever branch the caller stands
on, which is arbitrary. This task gives such edits their own branch.

This replaces phases 0 to 6 of the first plan. That plan kept these edits in a
custom ref and wrote git objects with no worktree. A real branch with a standing
worktree does the same job with much less code.
[[./00039-continuous-changes-accumulation-v.md]] records why the storage changed.

## The rolling-updates branch

Such edits accumulate on the branch `openplan/rolling-updates`. The daemon keeps one
worktree for it at `<git-common-dir>/openplan-rolling-updates`. The worktree uses a
cone-mode sparse checkout of `.plan`, so it holds the task files and no code.

Three things come free from code that exists:

- **Reads.** `Index::rebuild` reads every name in
  `repo.local_branches()`, so the branch appears with no new read path.
- **Uncommitted reads.** The index already lays a live worktree's files over its
  committed blobs (`crates/op-index/src/lib.rs:240`), so an edit there reads
  back as soon as it reaches the disk.
- **Serialised writes.** `Store` writes each task file under a file lock
  (`crates/op-store/src/lib.rs:327`), so any process can write. The CLI does not
  need the daemon.

## The daemon's three jobs

**Commit.** The daemon commits the updates worktree after a quiet window of
about 5 seconds. A burst of keystrokes becomes one commit. At startup the daemon
commits whatever is pending, which covers the edits the CLI made while the
daemon was down.

**Refresh.** The daemon runs `git rebase <default-branch>` in the updates
worktree, so the branch stays the default branch plus a linear stack of its
own commits. The trigger is the default branch tip moving. Debounce on a quiet
window of about 1 minute. A periodic sweep and a rebase at startup catch the
events the watcher drops. `ChangeEvent::RefMoved` exists already
(`crates/op-api/src/event.rs:20`) and nothing emits it. Emit it, and add the
commit id.

**Publish.** Only a person publishes. The daemon then fast-forwards the default
branch to the updates branch tip. Publish never merges and never forces.

## Conflicts

The merge driver merges edits to different sections. A same-section divergence
stops the rebase. Leave the rebase in progress. The conflicted files then sit in
the updates worktree with conflict markers, so a person or an agent opens them,
fixes them, and runs `git rebase --continue`.

The consequence, stated plainly: while the rebase runs, `Index` drops the
worktree from `live` because `op_in_progress` is true, so a write to it gets
the existing `NotWritable` refusal. The branch stays blocked until a person
resolves the conflict. The route below reports the conflicting files and the
worktree path.

A conflict needs the same section of the same task changed on both sides since
the last rebase, which is rare. An ephemeral second worktree would keep the branch
writable through a conflict, but the resolved result then goes stale against the
edits that arrived meanwhile. One worktree and one rule is the better trade.

## Merge driver

`crates/op-cli/src/mergedriver.rs` is a stub. It conflicts on any byte
difference. Replace it with the real 3-way merge at frontmatter-field and
section granularity. Reuse `op-md` for heading addressing and `op-task` for
parsing.

- Edits to different sections merge.
- Edits to different frontmatter fields merge.
- Only a same-section or same-field divergence conflicts.
- The parser must not choke on `<<<<<<<` markers.
- The driver writes the merged bytes to the `%A` path. Git's driver contract
  carries the result in that file, not in the exit code.
- Ship `/.plan/**/*.md merge=openplan` in a `.gitattributes` at the repository
  root. `**.md` does not match a nested path, so it must be `**/*.md`.
- Register `merge.openplan.driver` in the local git config when the project
  initialises.
- Where the driver is absent, git falls back to a line merge. CI and fresh
  clones have no driver, so a `.plan` merge there conflicts textually.

The spike in [[./00023-design-a-continuous-changes-accu.md]] proved that
`git rebase` calls the driver for `.plan/*.md`. This design uses the mechanism
the spike tested.

## Reaching the branch

`openplan/rolling-updates` is a branch like any other. A write reaches it by
naming it, with `--branch openplan/rolling-updates` on the CLI or `?branch=` on
the API. Nothing is rerouted, and a write that resolves to the default branch
lands on the default branch and its worktree.

`--branch` already existed on `list`, `get`, `comments`, and `show`. This adds it
to `create`, `set`, and `delete`, which the API already accepted.

## Index

The rolling-updates branch needs no special case in the index, and must not get one.

- **Headline.** `supersedes` decides by ancestry
  (`crates/op-index/src/lib.rs:418`). That branch is the default branch plus
  its own commits, so a version there descends from the default branch's
  version and wins on its own. Against a feature branch's version of the same
  task neither descends from the other, so `tie_break` picks by time, exactly as
  two feature branches do today. Excluding it from headline candidacy
  would hide every unpublished edit, which defeats the branch.
- **Baseline.** `is_baseline` matches only the default branch
  (`crates/op-index/src/lib.rs:470`), so it gets diffed against its merge
  base. Its cells are what it changed, which is the pending list for
  the route below. No new diff code.
- **Write target.** `Index::write_branch` needs no change. Only its caller
  changes. A task created there lives on that branch alone, so its headline
  already names it and the write reaches it with no redirect.
- **`headline_pref`** ranks the serve root 2 and the default branch 1, and the
  rolling-updates branch gets 0 (`crates/op-index/src/lib.rs:1149`). That rank breaks an
  exact-second tie only. Leave it.

The branch reaches a person in one place: `BranchSwitcher` on the task detail page
lists a task's versions per branch. Show it there under a plain name such
as "Rolling updates". Do not hide it. A version that comes from it is an
unpublished edit, and saying so is the point.

## API

- `GET /api/projects/{project}/rolling-updates` returns
  `{ pending: MatrixCell[], conflict: { files, worktree } | null }`. It carries
  facts, not a state to render: nothing to publish is an empty `pending`, and
  blocked is a `conflict`. `pending` is the matrix diff against the default
  branch, which the index already computes for every branch that is not the
  baseline.
- `POST /api/projects/{project}/rolling-updates/publish` fast-forwards and reports
  the new tip.
- Push a `rolling_updates_changed` event on the existing SSE channel.

## Publish

- The default branch is **not checked out**: update the ref.
- The default branch is **checked out in worktree W**: the normal case, because
  the primary checkout holds it. Update W's index and working tree as well.
  Refuse when W's `.plan` is dirty. Nothing here writes code into W, so the delta is
  task files only.

The only failure is a non-fast-forward, because the default branch can move
after the last rebase. Refuse with a retriable error that names the fix. The
refresh follows the move on its own, so the next attempt succeeds.

CLI `openplan publish` calls the endpoint and needs the daemon.

## Backup

Opt in with the git config key `openplan.backupRemote`, which holds a remote
name or a URL. Unset means no backup.

After the branch tip moves, the daemon runs:

```
git push --force <remote> openplan/rolling-updates:openplan/rolling-updates
```

Force is safe: this machine is the only writer and the mirror is write-only.
The push is best effort and stays off the critical path. A burst of tip moves
coalesces into one push. A failure logs a warning, does not fail the edit, and
retries on the next tip move. Push once at startup.

## Accepted costs

- `git branch` lists the branch and `git push --all` pushes it. Nothing stops a
  person from sharing it. The name says what it is.
- Production code shells out to `git` for rebase, commit, and push. Only tests
  do that today. gix cannot rebase and cannot push.
- The updates worktree belongs to the daemon. A person must not work in it.

## Spike results (git 2.50.1)

- A cone-mode sparse checkout of `.plan` gives a worktree that holds the task
  files and no code. Cone mode also keeps root files, so `.gitattributes` is
  there.
- `git rebase` in that worktree works. A default-branch commit that only changes
  code outside the cone replays cleanly and materialises nothing.
- The merge driver runs during that rebase and gets `%P` as the repository
  relative path. Git calls it for every file both sides changed, even when the
  hunks do not overlap, so the driver decides every case. A same-section line
  edit on both sides therefore conflicts, as it must.
- A driver that exits non-zero stops the rebase and leaves the conflict markers
  in the worktree, which is the resolution path this design needs.
- `/.plan/**.md` does not match `.plan/tasks/00001-t.md`. `**` is only special as
  a whole path component. Use `/.plan/**/*.md`, which covers `.plan/x.md` too.
- A worktree inside the git common directory works. `git worktree list` shows
  it, its own status is clean, the primary worktree's status ignores it, and
  `git fsck` reports nothing. gix discovery still needs a check in code.

## Verify

- Merge driver (`crates/op-cli/tests/`): different sections merge and `%A` holds
  the merged bytes; different frontmatter fields merge; a same-section
  divergence conflicts; a file that holds conflict markers still parses.
- Setup: a fresh project gets the branch, the worktree, the sparse checkout, the
  `.gitattributes`, and the git config entry.
- Writes: a write from the main worktree lands on `openplan/rolling-updates`;
  `--branch openplan/rolling-updates` from any worktree does the same; a write
  that names no branch lands on the worktree's own branch, the default branch
  included.
- Reads: an edit there reads back through the aggregation before the daemon
  commits it.
- Commit: a burst of edits becomes one commit; edits made while the daemon was
  down get committed at startup.
- Refresh: a move of the default branch triggers one rebase after the quiet
  window; a burst coalesces to one; a clean rebase advances the branch; a move
  made while the daemon was down gets picked up at startup.
- Conflict: an injected same-section divergence leaves the rebase in progress,
  reports `Blocked` with the task ids and the worktree path, and refuses
  writes to that branch; `git rebase --continue` after a fix clears the state.
- Publish: a clean publish fast-forwards the default branch and clears the
  pending count; publish with that branch checked out updates its working tree;
  a dirty `.plan` there is refused; a non-fast-forward is refused and leaves the
  branch unchanged; `openplan publish` with the daemon down errors clearly.
- Backup: with `openplan.backupRemote` set, a tip move pushes to a local bare
  remote; unset pushes nothing; a burst coalesces to one push; an unreachable
  remote logs a warning and does not fail the edit.

## Comments

### 2026-09-06T13:21:42Z by Milan Suk via claude-code

> The trigger for a moved default branch polls that ref every 5 seconds. It does not emit a new watcher event as the task says. Reading one ref is a file read, and the poll doubles as the sweep and the reconcile at startup, so op-watch needed no change.

### 2026-09-06T13:21:42Z by Milan Suk via claude-code

> Writes are not rerouted, on the user's decision. A write that resolves to the default branch lands there, and `openplan/rolling-updates` is reached by naming it. An earlier version of this work redirected such writes and overlaid the branch on reads of the default branch; both are removed.

### 2026-09-06T13:21:42Z by Milan Suk via claude-code

> The write redirect skips a task the rolling-updates branch does not carry. A file left uncommitted on the default branch belongs to no branch the rolling-updates branch was built from, so redirecting it reported the task as missing.

### 2026-09-06T13:21:42Z by Milan Suk via claude-code

> The backup push has op-git tests only. Nothing exercises the daemon's pusher against a live mirror.

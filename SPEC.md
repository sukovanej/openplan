# open-planner — Initial Spec (draft v0.1)

> Local-first, file-based system for managing development work as **tasks** —
> usable interchangeably by humans and AI agents. A native (Rust) CLI built for surgical,
> low-context-pollution access, plus a realtime, Linear-like web UI that humans watch and
> edit while agents work. Everything is local. Everything is plain markdown.

Status: **draft for redline.** Items marked **[OPEN]** are unresolved decisions.

---

## 1. Goals & non-goals

**Goals**
- Native-binary speed; no runtime dependency.
- **Section-level addressing** as the core primitive — read/overwrite a named part of a
  file so agents pull/replace only what they need (minimal context pollution).
- Markdown files stay **pristine** (no injected anchors/markers); edits produce minimal diffs.
- Humans and multiple agents operate the same store **concurrently and safely**.
- **Realtime web UI**: a human watches and edits while agents change things live.
- Fully **local** — no account, no cloud, no server dependency.

**Non-goals (v1)**
- Cloud sync, multi-user auth, mobile.
- LLM-in-the-loop summarization (summaries, if any, are human-written fields).
- MCP server (planned v2 — the CLI core is designed to be wrapped later).
- Workflow automation, templating engines, dependency solvers (v2+).

---

## 2. Core concepts (the ontology)

**One primitive: the task.** A task is a markdown file = YAML frontmatter (metadata) +
a section-addressable markdown body.

- Tasks nest **arbitrarily**: a top-level task with children *is* a "project"/epic; there is
  no separate project type.
- The body is free-form markdown organized into named sections (§5) — addressed and edited
  section by section. There is no separate document type.

(Standalone reference documents may return later as a second primitive — deferred, §12.)

---

## 3. Data model

### 3.1 Identity, title & frontmatter
- **Identity = a number**, never stored in the file. The daemon allocates it (§7.3) above the
  highest already in use on *any* local branch **or in any worktree's files** — a number is taken
  from the moment a file carries it, before any commit does — so a number is issued once per
  repository and no two tasks, on any branch, can ever share an id.
- **Two layers, one identity.** The number allocates the task and names its file; the **key** —
  `<ABBR>-<number>`, `OPP-42` — is the id everywhere above the store: the API, the CLI, the URLs, and
  the UI. `id` in every payload is the key. Disk and wire diverge on purpose: a task file stays
  numeric so an editor, a `grep`, and a plain markdown reader can follow a reference without oplan,
  while the key is a boundary rendering of that same number. No slug, no title fragment enters
  either, so nothing in an id can go stale when a task is renamed.
- **One spelling of a key, and no leniency.** `OPP-42` names the task; `42`, `opp-42`, `OPP-042`, and
  another store's `WEB-7` name nothing, and a write that uses one is refused rather than guessed at.
  A silent numeric fallback would reintroduce the second spelling the key exists to remove. Inside a
  key the number is decimal with no sign and no padding, the same one string that names the file.
- **`ABBR` comes from `.plan/config.toml`** — `abbreviation = "OPP"`, exactly three uppercase ASCII
  letters, hand-edited. It is **required**: a store with no valid abbreviation has no id space above
  its files, so there is nothing to degrade into. The daemon refuses to start and every CLI command
  exits non-zero (`.plan/config.toml: 'abbreviation' required`) — this is not the per-field "a read
  never fails" rule below, which governs a *task's* fields. One store has one abbreviation, read from
  the worktree the daemon serves (§7.10); another branch's copy of the file is ignored, so one task
  renders as one key across the whole cross-branch matrix. The daemon watches the file and applies a
  valid change live; one that leaves it missing or invalid stops the daemon exactly as it would have
  refused to start.
- **The file is named `<nnnnn>-<slug(title)>.md`** — `00042-ship-login-page.md`. Both halves of the
  name serve a human reading the directory: the padding sorts a listing in task order, the slug says
  which task each file is. Neither is part of the id, and a task is located by the number its name
  starts with — so renaming a file keeps the task as long as the digits do, and a title change
  leaves a slug that is merely out of date rather than a broken reference. A `.md` file whose name
  starts with no number is not a task and is left alone.
- **A file names another task by its file** — `parent: ./00042-ship-login-page.md`, and `[[./00042-ship-login-page.md]]`
  in prose. The store, which alone can see the directory, writes that form; everything above it —
  the API, the CLI, the UI — speaks the key. The point is that the directory stands on its own: a
  reference is a path an editor, a `grep`, or a plain markdown reader can follow without oplan. Only
  the leading digits identify the target, so a reference to a retitled task still resolves, and a
  reference whose task is gone keeps its number, which names no file because there is none to name.
  A human may write `[[OPP-42]]`; it resolves, and the next write through the daemon normalizes it to
  the file form — a reference naming no file yet is written as the key, since a markdown renderer
  knows no bare number. `[[42]]` and a foreign `[[WEB-7]]` are refused on write, and a hand-edited
  file that already holds one renders it as the plain text it is. What counts as a reference is what a
  renderer would linkify, so a `[[…]]` quoted inside code or an existing link is prose about the
  spelling and is neither rewritten nor refused.
- **Title = the body's single `# H1`.** Every task **must** contain **exactly one level-1
  heading**, and that is the title. Never stored in frontmatter.

Frontmatter carries only what is *not* derivable from the file itself:

| field | type | notes |
|---|---|---|
| `status` | enum | `backlog` / `todo` / `in_progress` / `in_review` / `done` / `cancelled`. `blocked` is **computed** from unmet `dependencies` — not stored. |
| `created` | RFC3339 | UTC, set once when the task is written. Required. Its counterpart `updated` is **derived** — the author time of the last commit to touch the file, or the file's own mtime while the working tree holds an uncommitted change to it — never stored. |
| `parent` | ref? | adjacency-list hierarchy (see §3.2). Absent = top-level. |
| `dependencies` | ref[] | task→task blocking; a reference may target a section (`./00042-ship-login-page.md#Section`). Omitted when empty. |

A task with no parent and no dependencies has frontmatter of just `status` and `created`.

**Writes are strict, reads are per-field.** A write parses the whole frontmatter or refuses, so a
file is never rewritten from a version we could not fully understand. A read never fails: each field
is parsed on its own and carries either its value or its own error (`missing` / `invalid`), and a
file whose fence or YAML is unreadable reports that once for the whole frontmatter. Nothing
substitutes a plausible value for one it could not read — a task with an unreadable `status` has no
status, is grouped apart on the board rather than filed under one it never claimed, and matches no
status filter.

`updated` reports the same way, though it is derived rather than read: git accepts any int64 as a
commit's author date, so a commit may hold one no calendar can express. Such a commit dates nothing
— the tasks it changed carry the reason in place of a time, and every other task, and every other
read, is unaffected.

**An uncommitted edit is dated by the file it wrote.** A working-tree edit belongs to no commit, so
its file's mtime dates it. Reading the clock instead would make every such task report as changed
*now* on every read, so work paused days ago would never age — and the same date orders the versions
of a task across branches, where a live edit stands against the commits of the branches it competes
with rather than automatically ahead of them. Only a filesystem that gives up no time at all falls
back to now.

A **deletion** carries no `updated` at all, committed or not: its file is gone, and the directory
that held it is restamped by every later write to any sibling task, so dating a removal by it would
report whenever some unrelated task was last edited.

### 3.2 Hierarchy & links
- **Hierarchy is a reference graph, not physical nesting.** Subtasks are their own files
  with a `parent` pointer. (A project task with 200 subtasks stays 200 small files, not one
  giant file — preserves context-efficiency and per-file locking.)
- Sibling **order** via fractional `rank`.
- **References** name the target's file and may target a **section** (`./00042-ship-login-page.md#Section`).

---

## 4. Storage layout (hybrid)

Files are the **source of truth**; the central index is a rebuildable cache.

```
<repo>/.plan/                 # in-repo store, versioned with the code; the dir's presence marks a store
  config.toml                 # `abbreviation = "OPP"`; required, hand-edited (§3.1)
  tasks/<nnnnn>-<slug>.md   # `00042-ship-login-page.md`; the number names the file, `OPP-42` is the id (§3.1)

~/.plan/                      # central index (per machine)
  registry.toml               # list of tracked store paths
  index.db                    # derived cache for cross-project UI (rebuildable by scan)
```
- In-repo → coding agents already have it in their working dir; it travels with the code.
- Central index → the UI aggregates many projects; a watcher per store feeds realtime.
- **[OPEN]** store folder name (`.plan/`? `.op/`?) and binary name.

---

## 5. Addressing model — the differentiator

### 5.1 The tree
Every file body is an `mdast`-style tree: **document → section (heading) → block**
(paragraph / list / list-item / code / table).

- Path grammar: `-t 'Section'`, `-t 'Section.Subsection'` (dotted, for nesting).
- Positional block access: `-t 'Section.1'` = first block under the section — **read-only**.
- A **section** spans from its heading to the next same-or-higher heading, **including**
  nested subsections.
- The single `# H1` is the **title** (§3.1), not an addressable section; sections start at `##`.

### 5.2 Anchors are dynamic, not persisted
- No markers or `{#id}` are written into files — they stay pristine.
- Handles are produced by `list` on demand (section handle = heading slug, deduped;
  block handle = content-hash or position).
- **Contract for agents: `list` → act.** A handle is not stable across a rename/edit, so
  don't hard-cache it across a mutation. Per-file locking (§6) makes this safe.

### 5.3 Edits splice source, never reprint
- Overwriting a section computes that node's **source byte range** and splices the new text
  in — the rest of the file is byte-for-byte untouched. No full reparse-and-serialize
  (which would reflow the whole document and blow up git diffs).

---

## 6. Concurrency

- **One in-band writer**: every mutation — web UI, CLI, agent — goes through the machine daemon's
  HTTP API (§7.3), so id allocation happens in exactly one place and creation cannot race. The store's
  locking below still stands: a hand-edit, an editor, or a `git` operation writes outside the daemon.
- **Per-file advisory lock**: acquire lock on the file → read-modify-write → release.
  Different files run fully in parallel; same file serializes. We don't need sub-file
  concurrency — simple and correct.
- **Atomic writes** (write temp + rename) so the UI file-watcher never reads a partial file.
- **Claim semantics**: an agent claims a task by setting `assignee` + `status=in_progress`
  under the lock; a second agent sees it's taken.
- **[OPEN]** optional: mirror each mutation into a git commit as an audit trail (v2).

---

## 7. Git & multi-worktree architecture

Tasks live in-repo (`.plan/`), versioned with the code, so they are **branch-scoped
by design**. Multiple worktrees on multiple branches work them in parallel. Git is the
versioning substrate — we read it, we don't rebuild it. Single machine only.

### 7.1 Model — one logical task, many branch-versions
- **Logical identity = the id**, immutable and stable across branches (`title` is just a field;
  renaming never changes identity — the id carries no trace of the title to go stale, and a branch
  that re-slugs a task's file still holds the same task).
- Each branch holds its own **version** of a task. Because ids come from one allocator whose floor
  spans every local branch (§3.1), independent creations on different branches never collide *or*
  falsely unify; a task shared across branches shares its number through common ancestry. Two
  branches can only ever hold two *versions* of one task `N`, never two different tasks — so a merge
  is a content merge, never a renumbering.
- **Core invariant: reads are global, writes are local.** You can read any task on any branch
  (from the object DB); you can only *mutate* a task on the branch whose worktree you have
  checked out. Cross-branch access is read-only aggregation.

### 7.2 Two kinds of state
| | Persistent state | Coordination / presence |
|---|---|---|
| **What** | title, sections, `status`, `dependencies`, `rank`, doc refs | who is *actively* working task X *right now*, in which worktree |
| **Scope** | **branch-scoped** (diverges per branch — intended) | **global** across all worktrees |
| **Home** | in git (`.plan/*.md`) | machine-local daemon registry, **not** in git |
| **Answers** | "where does this task stand *on branch feat/auth*" | "is anyone else on this task, so I don't collide" |
| **Lifetime** | permanent; merges via git | ephemeral; heartbeat + timeout, like editor presence |

`status: in_progress` is a *branch fact*. "Claimed right now" is a *machine fact*. Presence is
the coordination channel; git is the content channel. This is how cross-branch coordination
works without un-branching the data.

### 7.3 The daemon (singleton per machine)
One daemon per machine, managing N registered projects; single UI + coordination endpoint.
Per project it:
- enumerates worktrees (`git worktree list --porcelain`);
- reads **all local branch refs** (`refs/heads/*`) and builds the full task×branch matrix from
  the object DB (§7.4);
- overlays live **working-tree** state for each active worktree;
- holds the **presence registry** (§7.6);
- pushes to the UI and answers CLI coordination queries;
- **is the sole in-band writer**: it serves every create/update/delete, allocating ids from one
  in-memory counter (§3.1). A write names its target **branch**, which the daemon resolves to that
  branch's live worktree at write time — or refuses (§7.9). Nothing durable holds the counter: a
  committed file would conflict on every parallel-branch merge, so a restart re-seeds from the
  branches and worktree files instead — the ids on disk are the durable floor.

`oplan lint --fix` is the one sanctioned out-of-band writer: it allocates no id and resolves no
branch, so it rewrites task files directly under the store's advisory lock and atomic rename with no
daemon (§6), rather than through the in-band write path.

**Degraded fallback:** if the daemon is down, the CLI reads files / the object DB directly and
uses the on-disk claims file (file-locked), so headless agents still function. **Reads** degrade;
**writes** do not — a CLI write starts the daemon (auto-start, one per machine) and fails loudly if
it cannot, rather than writing behind the allocator's back. Auto-start roots the daemon at the
repository's **main checkout**, never at the caller's worktree: worktrees come and go per task, and a
daemon rooted in a removed one can no longer resolve any branch.

**Logging tiers** (`RUST_LOG`, default `info`): `info` covers lifecycle only — startup,
lifecycle failures, and change publications. `debug` adds one line per HTTP request (method,
path, matched route, request id, status, latency); requests slower than 1s log at `warn`. A
request that fails logs the underlying cause and the failure classification at `error`, both
tagged with the request's route and id.

### 7.4 Building the matrix from the object DB (all branches)
- For each local branch, read its `.plan/` tree via libgit2/gitoxide (not per-file shell-outs).
- **Dedup by blob OID**: task files unchanged across branches share one blob → parse each unique
  blob once. Work is proportional to *distinct versions*, not tasks × branches.
- **Blob-OID cache** in `index.db`: content-addressed, so the cache never invalidates.
- **Incremental**: when a ref moves, `diff-tree old..new` → re-parse only changed blobs → update
  the affected matrix cells. No full rescans.
- **Effective state** of (task, branch) = the dirty working-tree copy if a worktree has that
  branch checked out, else the committed blob at the branch HEAD.
- Group branches by blob OID per task → "these branches are identical, these diverge."

### 7.5 Change detection
- `notify` watcher on each active worktree's `.plan/` → working-tree edits (realtime).
- Watch `HEAD` (main + `.git/worktrees/*/HEAD`) → branch switches (whole-`.plan/` swaps).
- Watch `refs/` + `packed-refs` (or poll `git for-each-ref`) → commits, new/deleted branches, merges.
- Watch `.git/worktrees/` → worktrees added/removed.
- **Debounce + git-op awareness**: if `index.lock` / `MERGE_HEAD` / `rebase-merge/` /
  `rebase-apply/` is present, wait for the op to settle before re-scanning — never show torn state.

### 7.6 Coordination / presence (machine-local, not in git)
- Registry keyed by logical task id: `{ actor, worktree, branch, pid, since, lastHeartbeat }`.
- `op task claim <id>` acquires + heartbeats; auto-expires on heartbeat timeout; releases on
  `op task done` / explicit release / process exit (best-effort).
- `op task claim-status <id>` and live presence dots in the UI.
- Persisted to `~/.plan/<project>/runtime/claims.json`; the CLI uses it directly (file-locked)
  when the daemon is down. Orthogonal to the branch-scoped `status` field.

### 7.7 Conflicts & the section-aware merge driver
- **Creation never conflicts** (one file per task + repo-wide id allocation → different files).
- **Same-region edits on two branches** can conflict on merge → detect `<<<<<<<` markers, mark
  the task **conflicted** in the UI, offer a resolution view; the parser never chokes on markers.
- **Section-aware merge driver**, registered via `.gitattributes` (`/.plan/**.md merge=openplan`):
  a 3-way merge that operates at frontmatter-field and section granularity. Non-overlapping
  section/field edits **auto-merge**; only genuine same-section overlaps conflict. Two branches
  editing different sections of the same task merge cleanly. This is only possible because the
  format is structured (§5) — it's a direct payoff of the addressing model.

### 7.8 Git-operation safety
- Git rewrites the working tree outside our advisory locks. The daemon marks a worktree **busy**
  while a git op is mid-flight; the CLI queues/refuses writes to a busy worktree and retries
  after it settles.
- Our own writes: advisory per-file lock + atomic temp-write + rename.
- Agents commit task edits as part of their loop, shrinking the uncommitted window.

### 7.9 CLI branch-awareness
- Default scope = the current working directory's worktree/branch.
- `op task show <id> --branches` → status matrix across all branches.
- `op task list --all-branches` / `--branch <name>` (read-only for non-checked-out branches).
- `op task claim | release | claim-status <id>`.
- Writes always target the current worktree; cross-branch is read-only. Reads go straight to the
  files and the object DB; **writes** go to the daemon (§7.3) carrying the caller's branch — including
  from a linked worktree the daemon does not itself serve, since the branch, not a path, names the
  target. A checkout that moves the branch between the call and the write therefore yields the
  correct worktree or a refusal, never a wrong-branch write.
- A write to a branch **no live worktree holds** is refused (the daemon never fabricates a commit),
  and *creation obeys the same rule* — a task can only be created on a branch that is checked out
  somewhere. A worktree mid-`merge`/`rebase` counts as not live (§7.8).

### 7.10 Boundaries
- **Single machine only.** Cross-machine coordination (cloud worktrees on different hosts) needs
  a real shared server and breaks local-first — out of scope.
- Scope = local branches (`refs/heads/*`) + worktrees. Remote-tracking refs may be shown read-only.
- **One repository per daemon, for now.** §7.3's N registered projects are not built yet, and a branch
  name means nothing outside its repository, so a write whose repository the running daemon does not
  serve is refused rather than resolved against a same-named branch elsewhere.

### 7.11 Ambient edits & the rolling-updates ref

§7.1's "reads global, writes local" fits **code-coupled** edits — an agent on `feat/auth`
flipping `status` as part of its loop (§7.8), where the edit is genuinely a branch fact (§7.2).
It leaves a gap for **ambient / triage edits that belong to no feature branch**: reprioritizing,
rewriting a description, adding a subtask, changing `rank`, or any edit made through the global UI.
Today those land on whatever branch is checked out, which is arbitrary. The **rolling-updates ref**
is a dedicated lane where such edits accumulate over time and are then published into `main`.

**Storage — a custom ref, not a branch.** The lane lives at **`refs/open-plan/rolling-updates`**, a
custom ref *outside* `refs/heads/*`. Consequences: excluded from the default push refspec (can't be
accidentally shared), invisible to `git branch` / `git checkout` and the feature-branch swimlane,
addressed by the daemon by full ref path. It is still *explicitly* pushable for backup (below).

**Accumulation is worktree-less.** No standing worktree exists for the ref. An ambient edit is
committed straight into the object DB: write blob → build tree → commit (parent = current tip) →
compare-and-swap the ref. The daemon is the **sole writer**, serializing writes in-process — no
cross-process lock. Inbound edits are **debounced/coalesced** so rapid UI keystrokes and
drag-reorder collapse into few commits, keeping history legible.

**Reconciliation is also worktree-less** — the §7.7 section merge runs in-process. The daemon
**replays** each pending ambient commit `C` onto the new `main` as a sequence of **3-way tree
merges** (`gix::merge::tree`): per step, `base = tree(parent(C))`, `ours = ` the accumulated tip,
`theirs = tree(C)` — cherry-pick semantics, so the base is `C`'s *own parent*, never a single fixed
merge-base (that would re-merge already-replayed edits). Each step hands its content conflicts to the
same section-merge library the `merge=openplan` driver wraps; non-overlapping section/field edits
auto-merge, a genuine same-section overlap yields the conflicting `.plan/*.md` set. The chain stays a
linear `main` + ambient stack, and no checkout is ever needed. The merge machinery being unable to
run is a hard error, not a shell-out fallback. (An **ephemeral worktree** running `git rebase` is a
fallback only if the section merge ever needs a real working tree; not the primary path. The spike
confirmed the driver fires under git's own merge machinery — `rebase` replay and `merge-tree` — which
is the same machinery `gix::merge::tree` reuses.)

**Flow — refresh to track `main`, fast-forward to publish:**
- `main → updates` **refresh** — keep the ref reconciled with `main` so it is always
  `main` + the pending ambient delta. **Event-driven** off the ref-watcher (§7.5): triggered when
  `main` moves, not on a timer, and **debounced** on a quiet window (default ~1 min) so a burst of
  ref moves coalesces into one refresh once `main` settles. Reconcile worktree-less (above): the
  driver auto-merges non-overlapping section/field edits; a real same-section overlap surfaces from
  the merge-tree conflict set → mark those tasks **conflicted** in the UI (§7.7), hold the ref at its
  last good tip, and retry on the next refresh once a human reconciles. Refresh is **daemon-only**;
  if the daemon is down nothing refreshes (acceptable — refresh isn't urgent, publish is manual). A
  **periodic sweep** + **startup reconcile** (compare the ref's merge-base against current `main`) are
  in-daemon safety nets for watcher events dropped under load or missed while the daemon was off —
  **not** a daemon-down path.
- `updates → main` **publish** — **manual / explicit only**, and a pure **fast-forward**: because
  the ref is always reconciled on top of `main`, publish just advances `main` to the ref's tip. No
  merge commit on `main`, and publish **can never conflict** — every conflict is forced to surface
  earlier, at refresh. No background process ever mutates `main`.

**Routing — which write goes to the ref.** "Ambient" = a write with no feature-branch context:
- **UI**, global task-centric view (no worktree context) → `rolling-updates`.
- **UI**, worktree swimlane scoped to a branch → that branch (a feature context; consistent with §9).
- **CLI / agent** in the **`main`** (trunk) worktree → `rolling-updates` (trunk is not a feature lane).
- **CLI / agent** in a feature worktree → that branch (the §7.1 default).
- Escape hatch: `--ambient` forces a CLI write to `rolling-updates` even from a feature worktree, for
  triage edits unrelated to the current branch.

**Backup — durability only.** The daemon **force-pushes** `refs/open-plan/rolling-updates` to a
configured mirror remote so un-published ambient edits survive disk/machine loss. Safe because this
machine is the **sole writer** and the mirror is write-only (nobody pulls it) — no distributed
concurrency. Multi-machine / collaborative sync is **out of scope**: it would forbid the rewrite,
require a merge-based flow with push-race handling, and cross the §7.10 single-machine boundary.

**UI surface — one header control + review popover.** A single color-coded sync icon sits in the
header (beside daemon status / theme toggle) with a pending-count pill, reusing the app's convention
(`emerald`=good, `amber`=warn) plus blue for "action available":
- *In sync* — muted, check: "nothing to publish." *Pending (N)* — blue pill: "N changes ready to
  publish to `main`." *Syncing* — blue spinner: publish or auto-refresh running. *Blocked* — amber,
  warning: a refresh conflict paused sync. *Offline* — dim, disabled: daemon down.
- Click opens a **"Rolling updates" review popover** — it never publishes blind. It lists the pending
  ambient changes (task + what changed), flags conflicting ones in amber, and carries the primary
  **"Publish N to `main`"** action inside, keeping publish manual and reviewable. In *blocked* state
  it shows the conflict and a **"Resolve conflict"** action instead. Mockup:
  [`.plan/assets/sync-button-options.html`](.plan/assets/sync-button-options.html).

**Scope.** v1 publishes to **`main` only**. Fan-out to active feature branches is v2. The
cross-branch collision warning for ambient edits is presence (§7.6), tracked separately.

## 8. CLI surface (sketch)

Design rules: **JSON output for agents** (`--json`), pretty for humans; every read supports
`--fields` (projection) and `--depth` (tree/list bounding) to cap context.

```
# entities
# every <key> is `<ABBR>-<number>` — `OPP-42` (§3.1); a bare number is refused
op task create "<title>" [--parent <key>] [--status ..] ...
op task list [--status ..] [--parent <key>] [--tag ..] [--json] [--fields id,title,status]
op task tree <key> [--depth N]                # hierarchy, bounded
op task get <key> [-t 'Section.Sub'] [--json] # whole file, or one target
op task show <key>                            # metadata only
op task set <key> <field> <value>            # validated metadata write (status/priority/…)
op task move <key> --parent <key> [--before/--after <key>]  # reparent / reorder (rank)
op task link <key> --doc <doc-id>[#Section] | --dep <key>
op task next                                  # actionable, unblocked tasks  [v1.x]

# section/block ops (task bodies)
op task sections <key>                         # list addressable targets (handles)
op task get      <key> -t 'Section'            # read one target
op task set      <key> -t 'Section' --file f   # overwrite one target (splice)
op task append   <key> -t 'Section' ...        # append block(s) to a section

# server
op serve [--port N] [--open]                   # launches realtime web UI
```

---

## 9. Realtime web UI

- Embedded in the binary; `op serve` connects to (or spawns) the daemon and opens the SPA over
  HTTP + WebSocket. One UI per machine, fed by the daemon's aggregated matrix.
- **Live push**: working-tree edits, commits, branch switches, merges, and presence changes all
  stream to the UI as they happen.
- **Branch-aware, task-centric primary view**: one row per logical task with per-branch status
  badges (`done@main`, `in_progress@feat/auth` …), branches grouped by identical version,
  divergent branches highlighted, and live presence dots for active worktrees.
- **Newest first within a status group**: a task with no `rank` sorts by `updated`, most recently
  changed at the top; an explicit `rank` (§3.2) still places its task.
- **Branch/worktree swimlane view** as the secondary lens ("show me this worktree's board").
- Interactions: status change, drag-to-reorder (`rank`), inline section edit, create task/doc,
  claim/release (create is task-only). **Writes target the current/selected worktree's branch only** (reads global,
  writes local — §7.1).
- **Files remain source of truth** — the UI reflects external edits (editor, agent, git ops); it
  never owns state.

---

## 10. Tech (proposed)

- **Rust**, single static binary.
- Markdown: `comrak` (sourcepos) or `markdown-rs` (mdast + positions); `pulldown-cmark`
  `OffsetIter` is the splice-friendly alternative for byte ranges. **[OPEN]** pick during spike.
- Web: `axum` + `tokio` (+ ws); `rust-embed` for the SPA; `notify` for file watching;
  `fs2`/`fd-lock` for advisory locks; `serde`/`serde_yaml` for frontmatter.
- Git: `gitoxide` (or `git2`/libgit2) for object-DB reads, tree walks, ref enumeration, and
  `diff-tree`; a custom merge driver binary for `.plan/**.md` (§7.7).

---

## 11. Open decisions

1. ~~Subtasks as separate files + `parent` pointer~~ — **resolved: yes.**
2. ~~Store folder name~~ — **resolved: `.plan/`.** Project name **open-planner**. Binary name still open — `op` collides with the 1Password CLI; likely `oplan`.
3. ~~`status` enum values~~ — **resolved** (§3.1); `blocked` is computed, not stored.
4. ~~Docs as a second primitive~~ — **removed for now** (deferred, §12); tasks only.
5. Assignee/agent identity — **deferred.**
6. Block-handle scheme (content-hash vs position-on-read) — **deferred.**

---

## 12. Deferred to v2+
Standalone reference **docs** as a second primitive · MCP server · git-commit audit trail ·
dependency/next-task solver · templates · multi-user/sharing · cloud sync.

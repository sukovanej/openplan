---
created: 2026-09-27T14:15:55Z
---
# Storage and sync

Tasks live as git objects under `refs/openplan/tasks`, not in the working tree. An edit is a commit. Sync is a fetch, a merge, and a push of that one ref.

## Components

```mermaid
flowchart TD
  ui["Web UI / CLI"] -->|HTTP| daemon["Daemon<br/>op-server"]
  daemon --> index["op-index<br/>read model of the head"]
  daemon --> backend["GitBackend<br/>op-backend-git"]
  daemon --> loop["SyncLoop<br/>one per project"]
  loop --> backend
  backend -->|gix| odb[("Local object database<br/>refs/openplan/tasks")]
  backend -->|git CLI| origin[("origin<br/>refs/openplan/tasks")]
  backend -. "HeadMoved events" .-> daemon
  policy["TaskMergePolicy<br/>op-tracker"] --> backend
```

## Storage layout

```mermaid
flowchart TD
  ref["refs/openplan/tasks"] --> tip["commit<br/>OPP-131: status → in_progress<br/>author + Via: claude-code"]
  track["refs/openplan/remotes/origin/tasks<br/>last tip seen on origin"] --> older["older commits"]
  tip -- parent --> older
  tip --> tree["tree"]
  tree --> files["config.toml<br/>tasks/00120-stop-a-hung-git-fetch.md<br/>tags/bug.md<br/>docs/storage-and-sync.md<br/>assets/…"]
```

Both refs are outside `refs/heads/` and `refs/remotes/`: a forge shows no branch, and `git fetch --prune` does not delete them.

## Write

```mermaid
flowchart TD
  start(["Edit from the API"]) --> lock["Take the writing lock"]
  lock --> tip["Read the tip of refs/openplan/tasks"]
  tip --> fn["Run the write function<br/>on the snapshot of the tip"]
  fn --> objs["Write blobs, tree, commit<br/>parent = tip"]
  objs --> empty{"Tree changed?"}
  empty -- no --> nothing(["No commit"])
  empty -- yes --> cas{"Move the ref<br/>only if it still = tip"}
  cas -- "moved by another process" --> wait["Sleep 1–20 ms"] --> tip
  cas -- ok --> event(["HeadMoved, origin Local<br/>index reloads, sync in 2 s"])
```

After 64 failed attempts the write stops with `Contended`.

## Sync

```mermaid
flowchart TD
  fetch["git fetch +refs/openplan/tasks<br/>into refs/openplan/remotes/origin/tasks"]
  integrate{"Compare the local tip<br/>and the remote tip"}
  ff["Fast-forward the local ref"]
  merge["Merge commit, 2 parents<br/>TaskMergePolicy resolves conflicts"]
  push["git push --no-verify<br/>local tip → origin refs/openplan/tasks"]
  record["Move the tracking ref<br/>to the pushed tip"]
  done(["Done"])
  fetch --> integrate
  integrate -- "remote ahead" --> ff --> done
  integrate -- "both moved" --> merge --> push
  integrate -- "local ahead" --> push
  integrate -- "equal" --> done
  push -- accepted --> record --> done
  push -- "rejected, at most 5 times" --> fetch
```

Each git command has a 60 s deadline. At the deadline the backend kills git and ssh.

## Merge policy

```mermaid
flowchart TD
  conflict{"Both sides changed<br/>the same path"} --> kind{"Which document?"}
  kind -- "task or doc" --> case{"What did each side do?"}
  case -- "both edited" --> fields["Merge field by field and line by line<br/>a clash keeps both versions with labels<br/>the remote version stays in effect"]
  case -- "edit vs remove" --> keep["The edit wins"]
  case -- "both created the same task number" --> renum["The remote task keeps the number<br/>the local task gets the next free number<br/>local links to it are rewritten"]
  kind -- "tag, config, asset" --> other{"Edit vs remove?"}
  other -- yes --> keep
  other -- no --> theirs["The remote version wins"]
```

## When sync runs

```mermaid
flowchart LR
  start["Daemon start"] -->|now| sync(["Sync"])
  write["Local write"] -->|2 s| sync
  button["openplan sync<br/>Sync now in the UI"] -->|now| sync
  idle["No trigger"] -->|30 s| sync
  fail["Failed sync"] -->|"30 s × 2^n, max 10 min"| sync
  watchdog["Watchdog, every 5 s"] -->|refresh| ext(["HeadMoved, origin External<br/>for moves by other processes"])
```

A new clone has no tasks ref. The daemon fetches it from `origin` one time when it opens the checkout.

## Code map

| File | Holds |
| --- | --- |
| `crates/op-backend/src/merge.rs` | Three-way merge by path, the `MergePolicy` trait |
| `crates/op-backend/src/sync_loop.rs` | Sync timer and backoff |
| `crates/op-backend-git/src/lib.rs` | Ref names, open, the commit retry loop |
| `crates/op-backend-git/src/objects.rs` | Tree and commit writes, the ref compare-and-swap |
| `crates/op-backend-git/src/sync.rs` | Fetch, integrate, push |
| `crates/op-tracker/src/policy.rs` | `TaskMergePolicy` |
| `crates/op-server/src/project.rs` | Backend open, first fetch, sync loop start, watchdog |

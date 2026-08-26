---
status: todo
created: 2026-08-26T07:12:36Z
parent: ./00012-tags-registered-labels-name-colo.md
---
# Tag refusals: give the delete conflict a reason a caller can read

The daemon answers a tag delete with 409 for three different reasons: tasks on
this branch reference the tag, the branch has no writable worktree, or the
daemon's root is gone. Only the first one changes if the caller sends `--force`.

The web UI cannot tell them apart. It offers "Delete anyway" after any 409. When
the refusal was not a reference count, that button sends `force=true` and the
daemon refuses again with the same message.

The store's messages also speak to a CLI user. `StoreError::TagReferenced` says
"pass --force to delete it". The tags validation error says "register it with
`openplan tag create`". The web UI shows both messages in a toast. A person who
uses only the browser reads a flag name for a button they pressed.

## Do this

- Give `ApiErrorBody` a machine-readable reason, or give the referenced delete
  its own status. The web UI must know which refusal it received.
- Move the `--force` hint and the `openplan tag create` hint from `op-store` to
  `op-cli`. The store states the fact. The CLI adds the remedy.
- Offer "Delete anyway" only for a reference count. Show every other refusal as
  a plain error.

## Acceptance criteria

- [ ] A delete that a reference count refuses is distinguishable from a delete
      that a worktree refuses, by status or by field.
- [ ] `openplan tag delete` still prints the `--force` hint.
- [ ] No `op-store` message names a CLI flag or a CLI command.
- [ ] The web UI offers a forced delete only after a reference count.

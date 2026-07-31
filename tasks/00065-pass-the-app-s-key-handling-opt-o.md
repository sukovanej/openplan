---
status: todo
created: 2026-07-31T12:19:28Z
parent: ./00059-web-ui-extract-reusable-componen.md
---
# Pass the app's key-handling opt-out into TaskBody

`data-keys-ignore` is the app's keyboard-dispatcher contract — `web/packages/app/src/lib/keys/match.ts` is what reads it — yet `web/packages/task-ui/src/task-body.tsx` hardcodes it on `Prose`. The package should not know the app's key-handling opt-out exists.

Let `TaskBody` spread extra props onto `Prose`, and have the detail route pass `data-keys-ignore` at the call site, so the keyboard system's vocabulary lives entirely in the app.

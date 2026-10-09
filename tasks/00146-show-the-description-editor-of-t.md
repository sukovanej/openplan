---
status: backlog
created: 2026-10-09T13:26:14Z
tags:
- bug
- ui
---
# Show the description editor of the new task dialog without a load placeholder

After a page load, the first `c` opens the new task dialog with a gray skeleton in place of "Add description…". The skeleton stays until the browser has downloaded the code of the description editor. A second open shows the editor at once.

The dialog and the task page use the same lazy `BodyEditor` (`web/packages/app/src/components/task-content.tsx`). It loads `@openplan/editor` (CodeMirror and its markdown grammars) only when the first editor renders, and `<Suspense fallback={<BodySkeleton />}>` shows the skeleton while it loads. On the task page the skeleton looks correct, because the page also waits for the task. The dialog waits for no data, so the skeleton looks like a load of nothing. The Vite dev server makes the wait longer than the built app does.

Fix both parts:

1. Start the load of the editor code before the first open. For example, start it when the app is idle after it starts, or when the pointer moves over **New task**. Keep the editor out of the main bundle.
2. If the dialog opens before the load completes, show the "Add description…" placeholder text, not the gray block.

Check it with a refresh and then `c`, in the built app (`mise run install`) and in the Vite dev server.

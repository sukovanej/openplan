---
status: in_review
created: 2026-08-27T06:56:14Z
---
# Reload cached task detail after an unmounted change

## Problem

A task detail can keep old data after the page closes. If a comment or another task change occurs while the detail has no listener, the next visit reuses the old successful result. The list can show the new comment count while the detail omits the new comment.

## Cause

The client keeps unmounted task queries in its bounded cache. A `task_changed` event refreshes only queries that have listeners. A later subscription reloads a new or failed query, but it does not reload a successful query that missed an event.

## Scope

- Mark cached task queries as invalid when a matching task changes.
- Reload an invalid query when its next listener subscribes.
- Continue to reload mounted queries immediately.
- Do not restore eager network reads for every cached task.
- Apply the behavior to all task changes, not only comments.

## Verify

Add a regression test for this sequence: load a task query, unsubscribe, invalidate the task, and subscribe again. Verify that the query reloads and returns the new value. Keep coverage for the React StrictMode remount case and the bounded cache. Run the focused web tests, type check, lint, and build.

## Comments

### 2026-08-27T07:02:10Z by Milan Suk via codex

> Added the regression test only. It fails after the second subscription because the task loader stays at version 1 instead of returning version 2. Type checking, linting, and formatting pass. The cache fix is not implemented.

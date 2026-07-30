---
status: in_review
created: 2026-07-30T10:51:30Z
---
# Fix the implement-task workflow: meta literal and args parsing

The `implement-task` workflow cannot be launched by name, and rejects the
arguments the `/implement-task` skill documents. Both were hit on the first real
run (OPP-27) and worked around by hand, out of the repo.

## Defect 1 — `meta` is not a pure literal

`.claude/workflows/implement-task.js` opens with:

```js
whenToUse:
  'When a task in .plan/tasks/ is ready to build. Pass args {taskKey:"OPP-42", maxRounds?:3, base?:"main"}. ' +
  'Writes and reviews tests before any implementation. Returns a PR URL; leaves the task in_review.',
```

The Workflow loader requires `meta` to be a pure literal — no variables, calls,
spreads, or concatenation. The `+` makes `whenToUse` a `BinaryExpression`, so the
whole file fails to register:

```
Workflow "implement-task" not found. Available: deep-research, code-review
```

and loading it directly by path fails with
`meta must be a pure literal: non-literal node type in meta: BinaryExpression`.

Fix: join the two halves into one string literal.

## Defect 2 — `args` arrives as a JSON string

`parseTaskArgs` reads:

```js
const key = typeof args === 'string' ? args : args && args.taskKey
```

Passing the documented `{taskKey: "OPP-27", maxRounds: 3}` reaches the script as
the *string* `'{"taskKey":"OPP-27","maxRounds":3}'`, so the `typeof` branch takes
it whole as the key and the regex rejects it:

```
args.taskKey must be a task key like "OPP-42", got "{\"taskKey\":\"OPP-27\",\"maxRounds\":3}"
```

The bare-key form (`args: "OPP-27"`) is the only one that works today, which
silently drops `maxRounds` and `base`.

Fix: in `parseTaskArgs`, when `args` is a string that parses as a JSON object,
parse it and read the fields off the result; keep the bare-key string form
working, and keep the object form working for callers where it survives intact.
Normalise once, at the top, so `base` and `maxRounds` resolve from the same value
as `taskKey` rather than from a raw `args` that may still be a string.

## Also update the skill

`.claude/skills/implement-task/SKILL.md` shows only the object form. Once
`parseTaskArgs` normalises, the documented call works as written and needs no
change — confirm that rather than assuming it.

## Verify

- `Workflow({name: 'implement-task', args: {taskKey: 'OPP-27', maxRounds: 3}})`
  loads and reaches the Setup phase; the workflow appears in the available-workflow
  list alongside `deep-research` and `code-review`.
- `parseTaskArgs` returns `{key: 'OPP-27', base: 'main', maxRounds: 3}` for the
  object form, for the JSON-string form, and `{key: 'OPP-27', base: 'main',
  maxRounds: 3}` for the bare `'OPP-27'` string.
- A non-key input (`'nope'`, `{}`, `undefined`, a JSON string with no `taskKey`)
  still throws the same loud error.
- `meta` contains no expression nodes — every value is a literal.

## Out of scope

- Any behavioural change to the workflow's phases, prompts, or agent counts.
- The `## Verify` vs `## Acceptance criteria` heading mismatch in task bodies —
  separate concern.

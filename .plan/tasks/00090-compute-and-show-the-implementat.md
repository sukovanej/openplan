---
status: todo
created: 2026-08-27T11:13:32Z
---
# Compute and show the implementation flow

## Purpose

A person must know in which order to implement the tasks. The `dependencies` field in a task file
records that order. This feature computes the order and draws it.

The feature has two parts. A daemon endpoint computes the flow. A web view draws the flow as a
diagram.

## The graph

A node is one task. A node carries its project and its task key. A project is not a container. Two
projects give one graph with no edge between them.

An edge points in the direction of the time. It goes from the dependency to the task that
waits for it. The diagram then draws each arrow from left to right.

A parent task is a box. The box contains its children.

## Which tasks the graph contains

The request selects the seed tasks with a filter. The graph then grows from the seeds:

- Add each transitive dependency of a seed.
- Add the parent chain of each included task.
- Add each child of an included parent, if the child is not `done` and not `cancelled`.

The status filter selects the seeds only. The growth uses its own status rule. A `backlog` child of
an included parent is real work, so the box must show it.

The growth stops by itself. The endpoint sets no depth limit.

The endpoint does not add the tasks that depend on a seed. A query for one task must not pull in the
full store.

## How an id resolves

An id resolves inside one project. The endpoint does not resolve a key of a different project. A
`WEB-7` dependency in the `open-plan` store stays unresolved.

An id resolves against the same branch aggregate as the board. The flow and the list then agree.

A dependency that resolves to no task becomes an unresolved node. The label of that node is the raw
text. An unresolved node gets no wave. A dropped edge makes a wave number wrong, so the endpoint must
not drop it.

## What a parent means

Only a leaf task gets a position in the order. A parent gets no position of its own. A parent with no
child is a leaf.

A child inherits the dependencies of its parent. Without this rule a child could start before its own
box.

A dependency on a parent means a dependency on each remaining descendant of that parent. The wave of
a box is the highest wave of its children.

A box with no remaining child becomes a plain node.

## The order

The endpoint computes the waves with longest-path layering. A person can start a task in wave `k`
when wave `k-1` is complete.

The endpoint sorts the tasks in each wave in this order:

1. The count of the tasks that wait for it, directly or indirectly. The highest count comes first.
2. The `rank` field.
3. The task key.

The first rule puts the task that unblocks the most other work at the top. The `rank` field keeps the
manual order between equals.

The waves are global. Two projects share the wave numbers, because wave 1 means "you can start this
now" for both.

## Cycles

A dependency cycle in the loaded graph makes the request fail with 422. The error lists the members
of the cycle.

A cycle that the seeds cannot reach does not fail the request. `openplan lint` reports those.

`openplan lint` already checks the files on disk for cycles — `dependency_cycles` in
`crates/op-lint/src/rules.rs`. This feature changes no lint rule and adds none.

## Examples

### A chain, and two projects in the same waves

`OPP-92` depends on `OPP-91`. `OPP-77` and `WEB-3` depend on nothing.

```
wave 0                wave 1

OPP-91  ───────────►  OPP-92
OPP-77
WEB-3
```

The waves are global, so `WEB-3` shares wave 0 with `OPP-91`. No edge crosses the two projects.

### A parent as a box

`OPP-40` is a parent. Its children are `OPP-41`, `OPP-42` and `OPP-43`. `OPP-42` depends on
`OPP-41`. `OPP-43` is `done`. `OPP-50` depends on `OPP-40`.

```
wave 0                wave 1                wave 2

┌ OPP-40 ─────────────────────────────┐
│                                     │
│  OPP-41  ─────────►  OPP-42         │ ───────────►  OPP-50
│                                     │
└─────────────────────────────────────┘
```

The box hides `OPP-43`, because it is `done`. `OPP-40` gets no position of its own. `OPP-50` waits
for each remaining descendant of `OPP-40`, so it goes to wave 2.

### An inherited dependency

`OPP-40` is a parent of `OPP-41`. `OPP-40` depends on `OPP-60`.

```
wave 0                wave 1

                      ┌ OPP-40 ──────┐
OPP-60  ───────────►  │  OPP-41      │
                      └──────────────┘
```

The arrow lands on the box. `OPP-41` inherits the dependency, so it cannot stay in wave 0.

### An unresolved dependency

`OPP-70` depends on `WEB-7` and on `OPP-99`. `WEB-7` names another project, and `OPP-99` no longer
exists.

```
wave 0

⟦ WEB-7  ⟧ ──┐
             ├──►  OPP-70
⟦ OPP-99 ⟧ ──┘
```

Both become unresolved nodes. An unresolved node gets no wave, and it does not push `OPP-70` into a
later wave: no work can complete it, so it warns about a broken file instead of ordering the work.

### A satisfied dependency

`OPP-80` depends on `OPP-79`, and `OPP-79` is `done`.

```
wave 0

OPP-80
```

The endpoint drops the edge and adds no node. The dependency is complete already.

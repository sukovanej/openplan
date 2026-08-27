---
status: in_review
created: 2026-08-27T11:13:36Z
parent: ./00090-compute-and-show-the-implementat.md
---
# Add GET /api/flow

`GET /api/flow` returns the implementation flow. The parent task holds the graph rules, the parent
rules, the order and the cycle rule. This task builds them.

The endpoint is global. It does not sit under `/api/projects/{project}`, because one request can name
more than one project.

## Query

Each parameter repeats. Parameters with different names combine with AND. Values of one name combine
with OR.

- `project` — a project name.
- `status` — a seed status. With no `status`, each task that is not `done` and not
  `cancelled` is a seed.
- `task` — a task key, for example `OPP-42`.
- `tag` — a tag name.

A `task` parameter needs a `project` parameter. Refuse a `task` without a `project`: two stores can
commit the same abbreviation, so a bare key names no single task.

A request with no parameter selects each unfinished task in each project.

```
GET /api/flow
GET /api/flow?project=open-plan
GET /api/flow?project=open-plan&status=todo&status=in_progress
GET /api/flow?project=open-plan&task=OPP-42
GET /api/flow?project=open-plan&tag=api&tag=web
GET /api/flow?task=OPP-42                        400: task needs project
```

The parameters stay in the query string. The web view then keeps its state in the URL, and a person
can bookmark a flow.

## Response

The response is one flat node list and one edge list:

```json
{
  "nodes": [
    {
      "project": "open-plan",
      "id": "OPP-40",
      "title": "Ship the login page",
      "status": "todo",
      "kind": "box"
    },
    {
      "project": "open-plan",
      "id": "OPP-41",
      "title": "Add the store schema",
      "status": "todo",
      "parent": "OPP-40",
      "kind": "leaf",
      "wave": 0,
      "position": 0,
      "blocks_count": 2
    },
    {
      "project": "open-plan",
      "id": "OPP-42",
      "title": "Read the schema in the API",
      "status": "todo",
      "parent": "OPP-40",
      "kind": "leaf",
      "wave": 1,
      "position": 0,
      "blocks_count": 1
    },
    {
      "project": "open-plan",
      "id": "OPP-50",
      "title": "Deploy",
      "status": "todo",
      "kind": "leaf",
      "wave": 2,
      "position": 0,
      "blocks_count": 0
    },
    {
      "project": "open-plan",
      "id": "WEB-7",
      "kind": "unresolved"
    }
  ],
  "edges": [
    { "project": "open-plan", "from": "OPP-41", "to": "OPP-42" },
    { "project": "open-plan", "from": "OPP-40", "to": "OPP-50" },
    { "project": "open-plan", "from": "WEB-7", "to": "OPP-50" }
  ]
}
```

`kind` is `leaf`, `box` or `unresolved`.

A `box` node carries no `wave`, no `position` and no `blocks_count`. A parent gets no position of its
own, so the response omits the three fields rather than sending a null. The web view reads the span
of a box from its children.

An `unresolved` node carries only `project`, `id` and `kind`. Its `id` is the raw dependency text, so
it can read `WEB-7` or `OPP-99#Design`.

`blocks_count` counts the tasks that wait for the node, directly or indirectly. It is the first sort
key inside a wave. Send it, so a reader can check the order the endpoint chose.

An edge carries one `project`, because no edge crosses a project. `from` is the dependency and `to`
is the task that waits for it. The arrow then points the way the time runs, and the web view draws it
from left to right without reversing anything.

The response sends no node count and no wave count. The client reads `nodes.length`. A second copy of
a number the list already holds can disagree with it.

Compute the response for each request. Do not cache. The store is in memory and the graph is small.
A cache adds a second source of truth for no gain. The web view refetches on the existing SSE stream,
so the diagram stays live.

## Errors

Return 422 for a dependency cycle inside the loaded graph:

```json
{
  "message": "dependencies form a cycle: OPP-12 -> OPP-19 -> OPP-31 -> OPP-12",
  "cycles": [["OPP-12", "OPP-19", "OPP-31"]]
}
```

`ApiErrorBody` in `op-server` carries only `message` today. Add a `cycles` field to it, and skip the
field when it is empty. Each other endpoint then sends the same body as before. Do not put the cycle
in the message text alone: the web view would have to parse a sentence to link the keys.

## Where the code goes

Put the computation in `op-api`, next to `Board::build`. Put the route and the `utoipa` path in
`op-server`. Put the tests in `crates/op-api/tests/`.

## Comments

### 2026-08-27T17:55:16Z by Milan Suk via claude-code

> Decisions the task left open:
>
> - The four query fields narrow each other, and the default `status=todo` is one of them. So
>   `?project=open-plan&task=OPP-42` finds no seed while OPP-42 is `in_progress`; the caller
>   must add `&status=in_progress`. The task says the names combine with AND and that the
>   default is `todo`, and I kept both rules with no special case for `task`. Tell me if a
>   named task must instead override the status default.
> - A `cancelled` dependency counts as satisfied, like a `done` one. The task names only
>   `done`, but the child rule drops both, and no work waits for a cancelled task.
> - A named project must answer. An unknown name gives 404 and a blocked one 503, like every
>   other project route. A request that names no project skips a project it cannot read, as
>   the merged board does.
> - An unknown query parameter gives 400. A typo would otherwise return the whole store and
>   read as an answer.
> - The endpoint refuses no unknown task key. `?project=x&task=OPP-999` returns an empty flow.
> - The nodes come wave by wave, and each box comes before the first leaf it holds. The edges
>   sort by project, then by the key they point to. The task fixes no order, and the tests need
>   one.
> - A cycle report holds one member list for each cycle, turned so the lowest key opens it. Two
>   cycles put both lists in `cycles` and both paths in the message, separated by `;`.
>
> Not done here: `web/packages/api-client` still comes from the old spec. The generator needs
> `pnpm install` in the worktree, and [[./00092-draw-the-implementation-flow-dia.md]] is the
> task that reads the client. Run `mise run generate-web-client` there.
>
> Verified: 33 new tests (23 in op-api, 10 in op-server), 652 in the workspace, clippy and fmt
> clean, `openplan lint` clean. I also ran the endpoint against this repository's own store on
> a scratch daemon: `/api/flow` puts OPP-91 in wave 0 and OPP-92 in wave 1 inside the OPP-90
> box, `?task=` without `?project=` gives 400, and the tag filter returns an empty flow.

### 2026-08-27T18:20:23Z by Milan Suk via claude-code

> Code review: I took six findings and refused seven.
>
> Fixed:
> - A parent cycle made every member a box with no leaf, so `nodes` sent none of them while
>   `wire` still sent their edges — arrows from ids that were not in the response. One
>   `Family` relation now holds the parent link the flow uses. It drops the link of a task
>   that climbs back to itself, so both tasks stand alone, as the board keeps them.
> - A finished parent still passed its dependencies to its live children, which pushed them
>   into a later wave and pulled unrelated tasks in. `remaining_dependencies` now returns
>   nothing for a `done` or `cancelled` task, in the growth, in the layering and in the edges.
> - `?project=x&project=x` read the store twice and sent every task as two nodes with
>   contradictory counts. A repeated value of any name is dropped now.
> - The cycle walk reported one cycle for each tangle. It cuts the step that closed the cycle
>   and walks again, so two cycles through one task both get a report.
> - `the_openapi_spec_documents_every_json_api_route` and its refusal twin did not name
>   `/api/flow`. Both do now, including the 422.
> - `parent_in_flow` unwrapped a nested `Option`, and `cycle_ids` sorted with a branch for an
>   empty cycle that `rings` cannot make. Both are gone.
>
> Refused, with the reason:
> - The 422 body carries bare keys, not project-key pairs. The task fixes that shape.
> - A cycle in one project fails the whole request. OPP-90 says a cycle in the loaded graph
>   fails the request, and one request loads every project.
> - `/api/flow` refuses `fresh` and `branch`. The task names four parameters, and the flow
>   reads the branch aggregate on purpose.
> - The project loop repeats the merged board's loop. The two differ: a named project fails the
>   request, and an unnamed one drops out. A shared helper would need a flag for that.
> - `tests/flow.rs` repeats the test harness of `tests/projects.rs`. `tests/http.rs` already
>   holds a second copy; a common module is one change for all three, not for this task.
> - `blocks_counts` holds a set for each leaf and `leaves_under` re-walks a box. OPP-90 says
>   the graph is small and forbids a cache.
>
> 657 tests pass, clippy and fmt are clean, and the endpoint answers the same for this
> repository's own store.

### 2026-08-27T18:29:54Z by Milan Suk via claude-code

> You removed the `todo` default from the status filter.
>
> With no `status`, a seed is now every task that is not `done` and not `cancelled`. A finished
> task seeds nothing, because the flow orders the work that is left, and a bare `/api/flow` would
> otherwise send this repository's ninety finished tasks. A caller who wants one asks for it:
> `?status=done` still works.
>
> So `?project=open-plan&task=OPP-42` now finds OPP-42 in any unfinished status. A `done` task
> still needs `&status=done`.
>
> I updated the Query section of this task, because it carried the old rule.

---
status: todo
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
- `status` — a seed status. The default is `todo`.
- `task` — a task key, for example `OPP-42`.
- `tag` — a tag name.

A `task` parameter needs a `project` parameter. Refuse a `task` without a `project`: two stores can
commit the same abbreviation, so a bare key names no single task.

A request with no parameter selects each `todo` task in each project.

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

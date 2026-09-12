---
status: in_review
created: 2026-09-10T00:16:28Z
dependencies:
- ./00113-agent-sessions-the-daemon-runs-a.md
- ./00110-ui-rolling-updates-control-and.md
tags:
- feature
- ui
---
# UI: the agent page, a chat beside the live task preview

The page where a person and the agent write a task together: a chat on the
left, the task on the right, drawn by the same components as the task detail
page. It talks to the sessions from
[[./00113-agent-sessions-the-daemon-runs-a.md]].

## Routes

- `AGENT_ROUTE = /:project/agent` opens the page for a task that does not
  exist yet.
- `AGENT_TASK_ROUTE = /:project/agent/:id` opens it for an existing task.
- `?session=<id>` names the session the page is attached to.

Add `agentPath(project, id?, session?)` to `task-path.ts`. React reads the
project and the id from the URL, so a reload draws the same page.

How a person gets there:

- The project board gets a "New task" button at its top right, with the
  `Sparkles` icon. The merged board has no project, so it has no button.
- The task detail page gets an "Agent" action beside "Flow" in the panel
  header.
- The command palette lists "Draft a task with the agent".
- Two keys in `bindings.ts`: `n` on a project board, and `a` on a task detail
  page. The help overlay lists them from the registry.

## Attaching

The page holds one session id, from `?session=`. With none:

- On the task route, `GET /agent/sessions` runs once. The newest session that
  is bound to this task and has not exited becomes the page's session, and
  `?session=` is set with `replace`. That is what a reload comes back to.
- Otherwise the chat is empty and the composer waits. The first send calls
  `POST` with the prompt, and the task when the route has one. On 201,
  `?session=` is set with `replace` and the stream opens.

When `SessionEvent::Task` arrives on the new-task route, navigate to
`AGENT_TASK_ROUTE` for that id, with the same `?session=`, with `replace`. The
URL then names both, and the preview mounts.

A session id that answers 404 is one the daemon no longer holds. The page says
"This session has ended" and drops `?session=`, so the next send starts a
new one.

## Stream

`lib/agent-events.ts` holds the `SessionEvent` schema, written by hand the
way `ChangeEvent` is: `snapshot`, `agent`, and `task`, with `AgentEvent`
under it as a union on `event`. `lib/agent-transcript.ts` holds a `Transcript`
type that mirrors the Rust one and a pure `applyAgentEvent(transcript, event)`
that folds exactly as `Transcript::apply` does. `lib/agent-session.ts` holds
`useAgentSession(project, id)`: it opens an `EventSource` on the events route,
takes the transcript from the snapshot, folds every later event into React
state, and reconnects with the same backoff `realtime.ts` uses. The app's
one `/api/events` connection stays as it is. This is a second connection, open
only while the page is.

The preview needs nothing new. The agent's edit reaches the daemon as a
`task_changed` on the workspace branch, `applyChange` already invalidates that
task's query, and the preview refetches.

## Layout

The same frame as the detail page: two columns, each with its own scroll.

The chat is a `Panel` on the left, `lg:w-[36rem]`. The preview fills the rest.
The preview is the detail page's task view for `(project, id, branch)`, with
`branch` from `SessionView`. Extract `TaskDetailView` and its data hooks from
`routes/detail.tsx` into `routes/task-view.tsx`, so the two routes render one
component and the preview cannot drift from the page. The branch switcher stays
visible; a change of it is local state here rather than a URL parameter.
Before a task is bound, the right column shows an `EmptyState` that says the
agent has not written a task yet, with a skeleton under it while a turn runs.

## Chat

The person asked for a chat that says what is happening and not much more. The
transcript entries draw as follows.

- A `Prompt` is a bubble on the right, in the person's words, whitespace kept.
- A `Message` is prose on the left, rendered with `TaskBody`, so a task key in
  the agent's answer is a link. While `done` is false, a caret blinks at its
  end and the text grows as deltas arrive.
- A run of `Thinking` and `Tool` entries between two messages is one activity
  row. While the turn runs and the run is the last thing in the transcript, the
  row shows a spinner and one verb phrase for the newest entry. When the run
  ends, the row becomes a muted "N steps" line. A click expands it to the list
  of phrases, with a failed step marked in amber. Nothing in it is a raw tool
  name or a JSON input.
- A `Failure` is one amber line with its message, plus "retrying" when the
  agent will.
- An open approval is a card with the tool's phrase, the command or the path
  from its input, and the buttons Allow, Allow for this session, and Deny. It
  leaves when `ApprovalResolved` arrives or the turn ends.

The verb phrases live in `lib/agent-activity.ts`, pure, keyed on the tool
name and its input:

| entry                            | phrase                          |
| -------------------------------- | ------------------------------- |
| `Thinking`                       | Thinking                        |
| `Read`                           | Reading `<file name>`           |
| `Grep`, `Glob`, `web_search`     | Searching                       |
| `Edit`, `Write`, `apply_patch`   | Writing `<file name>`           |
| `Bash`, `shell`                  | Running `<first word>`          |
| anything else                    | Working                         |

The file name is the last path segment. A `Bash` command that starts with the
daemon's binary shows as "Running openplan".

The page header states the session: a spinner and "Starting the agent" until
`Ready`, the model name from `SessionInfo` once it is known, and a banner
"The agent exited" with the code and a "Start a new session" button after
`Exited`. `over_budget` shows one line that says the session spent its budget.

The composer is a textarea at the bottom of the chat. Enter sends,
Shift+Enter breaks the line. While a turn runs the send button is Stop and
calls interrupt. The composer is disabled before `Ready` and after `Exited`.
Under it, one muted line: the session's tokens, and its cost when the agent
reports one.

The chat scrolls to its end as text streams, unless the person scrolled up,
and comes back to following on the next send.

## Colours and motion

Reuse the app's convention: blue for a live session, amber for a failure or an
open approval, muted for a finished activity row. One spinner component in
`@openplan/ui`, `spinner.tsx`, that the header, the activity row, and the
rolling-updates control share. The control's own spinner moves into it.

## Verify

Tests in `web/packages/app/tests/`:

- `agent-transcript.test.ts`: deltas join into one entry; the ended text
  replaces the deltas; streamed tool output stays when the end reports none;
  an approval leaves on resolve and on turn end; the status moves Starting to
  Idle to Running to Idle to Exited.
- `agent-activity.test.ts`: the phrase for each tool name; a run of thinking
  and tools between two messages is one row with the newest phrase and the
  count; a failed step is marked; the run closes when a message follows.
- `agent-session.test.ts`: the newest live session bound to a task is picked,
  an exited one is not, and none is picked on the new-task route.
- `agent-events.test.ts`: every `SessionEvent` and every `AgentEvent` kind
  decodes from the JSON `serde` writes.

Rendering: a `spinner.test.tsx` in `web/packages/ui/tests`, and an interactive
pass with headless Chrome over CDP per the repository's web-UI recipe. Start
from the board with `n`, ask for a task, watch the activity row change as the
agent reads the code, see the preview mount when the task binds, refine it
with a second prompt, see the preview update, reload the page, and find the
same session attached.

## Comments

### 2026-09-10T01:30:25Z by Milan Suk via claude-code

> The task detail page already binds `a` to Add subtask, so the agent page takes `e` there: it edits the task. `n` on a project board is as specified.

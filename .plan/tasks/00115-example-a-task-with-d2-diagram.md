---
status: todo
created: 2026-09-10T19:40:00Z
tags:
- docs
- draft
- ui
---
# Example: a task with d2 diagrams

This task shows how a diagram travels from an agent to the screen. Each fenced
code block tagged `d2` below renders as a picture in the web UI. The source
stays in this file as plain text.

## How a diagram reaches the screen

```d2
agent: Coding agent
task: Task file {
  shape: page
}
daemon: openplan daemon
browser: Web UI {
  markdown: react-markdown
  engine: d2 engine (WASM worker)
  markdown -> engine: d2 fence
}

agent -> task: writes a d2 fence
task -> daemon: reads .plan/tasks
daemon -> browser.markdown: raw markdown
browser.engine -> browser.markdown: SVG for the current theme
```

The daemon never touches the diagram. It sends the raw markdown, and the
browser compiles the fence when the task page opens. The engine loads once per
browser session and stays cached for a year.

## What happens on a theme change

```d2
shape: sequence_diagram

page: Task page
block: DiagramBlock
engine: d2 engine
css: App theme (CSS)

page -> block: mounts a d2 fence
block -> page: "Drawing the diagram"
block -> engine: compile once
engine -> block: diagram
block -> engine: render the current theme
engine -> block: SVG
block -> page: one <img>
css -> block: theme flips
block -> engine: render the other theme once
engine -> block: SVG
```

## The data the engine sees

```d2
fence: {
  shape: sql_table
  source: string {constraint: primary_key}
  theme: string {constraint: primary_key}
  svg: string
  view_box: string
}

cache: {
  shape: cylinder
  label: Map<theme + source, result>
}

fence -> cache: one entry per theme and source
```

## What happens with a bad diagram

The block below has an error on purpose. The UI keeps the source and shows the
compiler message above it.

```d2
cli -> daemon
daemon ->
```

## Rules for agents

- Tag the fence `d2`.
- Use shapes, containers, and connections.
- Do not use imports, icons, links, or layout settings.

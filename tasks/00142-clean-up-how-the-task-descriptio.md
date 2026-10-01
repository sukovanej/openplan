---
status: done
created: 2026-09-28T13:02:04Z
tags:
- feature
- ui
---
# Clean up how the task description looks

Review how the body editor shows a task description. Make the result clean and calm.

## Remove the rule above a level 2 heading

The editor draws a horizontal line above each `##` heading. Remove the line.

The line is a background gradient on `.op-editor .cm-h2` in `web/packages/editor/src/editor.css`. Remove the gradient. Then set the top padding again so that the space above a `##` heading fits between the `#` and `###` levels. Keep the space in padding, not margin, because CodeMirror measures each line from its border box.

## Do a full check of the description

Open some real task descriptions and examine each element:

- heading sizes, weights, and the space above and below each level
- paragraph spacing and line height
- lists, nested lists, and task lists
- code spans and code blocks
- block quotes
- tables
- Mermaid diagrams
- links and task references

Fix each element that looks heavy, crowded, or not the same as the elements near it. Check the light theme, the dark theme, and a phone width.

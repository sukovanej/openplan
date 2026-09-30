---
status: in_progress
created: 2026-09-27T14:53:42Z
tags:
- feature
- ui
pull_requests:
- https://github.com/sukovanej/openplan/pull/239
---
# Edit a tag in place on the Tags page

On the Tags page, the user edits a tag with the pencil button. The button replaces the row with a form that has a name field, a description field, a Save button, and a Cancel button. Replace this form with in-place edits.

## Behaviour

- Click the chip text or the slug text. An input for the tag name replaces them. The input holds the current name and has the focus.
- Click the description. An input for the description replaces it. The input holds the current description and has the focus.
- When a tag has no description, show a muted "Add description" placeholder in its place. Click the placeholder to edit the description.
- Press Enter or move the focus out of the input to save. Send only the field that changed. When the value did not change, send no request.
- Press Escape to cancel. The row shows the old value again.
- An empty name does not save. The row shows the old name again.
- An empty description clears the description.
- When the server refuses the change, keep the input open with the value that the user typed, and show the error.

## Remove

- The pencil button in each tag row.
- The edit mode of `TagForm` in `web/packages/app/src/routes/tags.tsx`. Keep `TagForm` for "Register tag".

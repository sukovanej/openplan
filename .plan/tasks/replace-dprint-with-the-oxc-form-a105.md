---
status: done
---
# Replace dprint with the oxc formatter (oxfmt)

Swap the web frontend's formatter from dprint to the oxc formatter ("exc"),
aligning formatting with the oxlint linter already in use.

## Current state
- `web/dprint.json` configures the TypeScript, JSON, and Markdown plugins.
- `web/package.json` scripts: `format` = `dprint fmt`, `format:check` = `dprint check`.
- `dprint` is a devDependency (via the pnpm catalog).

## Work
- Add the oxc formatter as a devDependency; remove `dprint`.
- Port the relevant dprint options (double quotes, ASI semicolons, excludes)
  to the new formatter's config; drop `web/dprint.json`.
- Update the `format` / `format:check` scripts to invoke the new formatter.
- Reformat the codebase and commit the (mechanical) diff separately from config.
- Update any CI / mise tasks that call `dprint`.

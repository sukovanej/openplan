---
status: in_review
created: 2026-09-04T10:21:30Z
---
# Web: ctrl + n and ctrl + p move the palette selection

The command palette moves its selection with the arrow keys only. Add `ctrl + n` and `ctrl + p` as equal alternatives: `ctrl + n` moves down, `ctrl + p` moves up. They wrap at the ends, as the arrow keys do.

The keys work while the palette input holds the focus, so handle them where `palette.tsx` handles `ArrowDown` and `ArrowUp`. Take the key only with the control modifier, and let a plain `n` or `p` reach the input as text. Stop the browser from acting on the key.

The help overlay does not list the palette's own keys today. Leave it as it is.

Keep the combobox out of this change.

## Comments

### 2026-09-04T10:35:18Z by Milan Suk via claude-code

> The palette maps ctrl + n and ctrl + p to ArrowDown and ArrowUp before the key switch, so one handler keeps both sets of keys. A plain n or p keeps its own meaning and reaches the input. Verified in headless Chrome over CDP: ctrl + n and ctrl + p walk the search hits, and a plain n types into the query.

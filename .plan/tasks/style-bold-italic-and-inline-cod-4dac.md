---
status: in_review
---
# Style bold/italic and inline code in markdown rendering

Improve the markdown inline element rendering in the web UI:

- Bold / strong / italic elements should display in the same color as regular
  body text (currently they may render with a different/emphasized color).
- Inline code elements should not display the literal backtick (`` ` ``)
  characters. Instead, render the element with a lighter background to set it
  apart from surrounding text.

const TEXT_INPUTS = new Set(["text", "search", "email", "url", "tel", "number", "password"])

// A reload drops an open dialog and every field the user typed in. A text field with nothing in it
// holds nothing to lose.
export function holdsUnsavedWork(page: Document): boolean {
  if (page.querySelector('[role="dialog"], [role="alertdialog"], dialog[open]') !== null) return true
  const fields = page.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>("input, textarea")
  return [...fields].some(
    (field) => (field instanceof HTMLTextAreaElement || TEXT_INPUTS.has(field.type)) && field.value !== "",
  )
}

import { act, type ReactNode } from "react"
import { createRoot, type Root } from "react-dom/client"
import { MemoryRouter } from "react-router-dom"
import { afterEach } from "vitest"

// React refuses to run `act` outside a test environment it has been told about, and the flag is a
// global rather than an argument.
;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const mounted: Array<{ root: Root; container: HTMLElement }> = []

// Torn down between tests so a component's effects and document-level listeners cannot outlive the
// test that mounted it.
afterEach(() => {
  for (const { root, container } of mounted.splice(0)) {
    act(() => root.unmount())
    container.remove()
  }
})

export function render(node: ReactNode): HTMLElement {
  const container = document.createElement("div")
  document.body.append(container)
  const root = createRoot(container)
  act(() => root.render(<MemoryRouter>{node}</MemoryRouter>))
  mounted.push({ root, container })
  return container
}

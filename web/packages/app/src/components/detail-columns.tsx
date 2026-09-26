import type { ReactNode } from "react"

import { Panel } from "@openplan/ui"

// Each column scrolls on its own, so the box keeps its frame and its header stays where it is while
// the body runs. Stacked, the two are one page and the page scrolls instead. The aside holds what
// stands beside the task or the doc and shares the width it leaves; narrow enough and it drops under
// it instead. No section in it wears a frame: a section leads with the rule that separates it from the
// one above, and the first has nothing above it to separate from.
export function DetailColumns({ main, aside }: { main: ReactNode; aside: ReactNode }) {
  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto lg:flex-row lg:overflow-hidden">
      <Panel className="dark:[--surface:color-mix(in_srgb,var(--muted)_25%,var(--background))] h-auto min-w-0 lg:h-full lg:w-[59rem]">
        {main}
      </Panel>
      <aside className="min-w-0 lg:min-w-80 lg:flex-1 lg:overflow-y-auto [&>section:first-child]:mt-0 [&>section:first-child]:border-t-0 [&>section:first-child]:pt-0">
        {aside}
      </aside>
    </div>
  )
}

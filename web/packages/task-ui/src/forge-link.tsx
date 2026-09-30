import type { ForgeKind } from "@openplan/api-client"
import { cn, Tooltip } from "@openplan/ui"

import { ForgeIcon } from "./forge-icon"

// An issue, a pull request, or a merge request by the icon of its forge and its short form. The link
// keeps the whole address, and the tooltip shows it.
export function ForgeLink({
  url,
  forge,
  short,
  className,
}: {
  url: string
  forge: ForgeKind
  short: string
  className?: string
}) {
  return (
    <Tooltip content={url} className="min-w-0">
      <a
        href={url}
        target="_blank"
        rel="noreferrer"
        className={cn("group inline-flex min-w-0 items-center gap-1.5 no-underline", className)}
      >
        <ForgeIcon forge={forge} className="size-3.5 shrink-0 opacity-70" />
        <span className="truncate group-hover:underline">{short}</span>
      </a>
    </Tooltip>
  )
}

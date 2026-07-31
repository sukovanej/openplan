import type { ReactNode } from "react"

import { cn } from "./cn"
import { CountPill } from "./count-pill"

export function Section({
  title,
  count,
  action,
  className,
  children,
}: {
  title: string
  count?: number
  action?: ReactNode
  className?: string
  children: ReactNode
}) {
  return (
    <section className={cn("mt-8 border-t pt-5", className)}>
      <div className="mb-2 flex items-center gap-2">
        <h2 className="text-muted-foreground text-xs font-semibold tracking-wide uppercase">{title}</h2>
        {count !== undefined && count > 0 && <CountPill count={count} />}
        {action !== undefined && <div className="ml-auto">{action}</div>}
      </div>
      {children}
    </section>
  )
}

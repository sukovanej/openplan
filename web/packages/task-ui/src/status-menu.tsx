import { Check } from "lucide-react"

import type { Status } from "@openplan/api-client"
import { cn, Kbd, Menu, type MenuItem } from "@openplan/ui"

import { STATUSES, statusIcon, statusLabel, statusMark, statusOfShortcut, statusShortcut } from "./status"

export function StatusMenu({
  current,
  onPick,
  onClose,
  className,
}: {
  current: Status | undefined
  onPick: (status: Status) => void
  onClose: () => void
  className?: string
}) {
  const items: ReadonlyArray<MenuItem> = STATUSES.map((status) => {
    const Icon = statusIcon(status)
    return {
      key: status,
      shortcut: statusShortcut(status),
      content: (
        <>
          <Icon className={cn("size-4 shrink-0", statusMark(status))} aria-hidden />
          <span className="grow">{statusLabel(status)}</span>
          {status === current && <Check className="text-muted-foreground size-3.5 shrink-0" aria-label="Current" />}
          <Kbd token={statusShortcut(status)} className="h-5 min-w-5 px-1" />
        </>
      ),
    }
  })
  const indexOfShortcut = (key: string) => {
    const status = statusOfShortcut(key)
    return status === undefined ? undefined : STATUSES.indexOf(status)
  }
  return (
    <Menu
      label="Status"
      items={items}
      initial={current === undefined ? 0 : STATUSES.indexOf(current)}
      onPick={(index) => onPick(STATUSES[index])}
      onClose={onClose}
      indexOfShortcut={indexOfShortcut}
      className={className}
    />
  )
}

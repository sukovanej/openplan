import { Tooltip } from "@openplan/ui"

import { type ConnectionState, useConnection } from "../lib/connection"

const tooltips: Record<ConnectionState, string> = {
  connecting: "The web app connects to the daemon.",
  live: "The daemon is up.",
  reconnecting: "The daemon is down. The web app connects again.",
  stopped: "The daemon is down.",
}

export function ConnectionStatus() {
  const state = useConnection()
  return (
    <Tooltip content={tooltips[state]}>
      <span
        tabIndex={0}
        role="img"
        aria-label={tooltips[state]}
        className={`size-2.5 shrink-0 rounded-full ${state === "live" ? "bg-success" : "bg-warning"}`}
      />
    </Tooltip>
  )
}

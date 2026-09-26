import { RefreshCw } from "lucide-react"

import { Button, Tooltip } from "@openplan/ui"

import { type ConnectionState, useConnection } from "../lib/connection"

const tooltips: Record<ConnectionState, string> = {
  connecting: "The web app connects to the daemon.",
  live: "The daemon is up.",
  reconnecting: "The daemon is down. The web app connects again.",
  stopped: "The daemon is down.",
  updating: "The daemon installs a new version. The web app connects again.",
  outdated: "The daemon runs a new version. Reload the page to use it.",
}

export function ConnectionStatus() {
  const state = useConnection()
  return (
    <>
      <Tooltip content={tooltips[state]}>
        <span
          tabIndex={0}
          role="img"
          aria-label={tooltips[state]}
          className={`size-2.5 shrink-0 rounded-full ${state === "live" ? "bg-success" : "bg-warning"}`}
        />
      </Tooltip>
      {state === "outdated" ? (
        <span className="text-muted-foreground inline-flex shrink-0 items-center gap-1 text-xs">
          New version
          <Button variant="accent" onClick={() => window.location.reload()}>
            <RefreshCw className="size-3.5" />
            Reload
          </Button>
        </span>
      ) : null}
    </>
  )
}

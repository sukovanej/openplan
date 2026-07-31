import { useConnection } from "../lib/connection"

export function ConnectionStatus() {
  const up = useConnection() === "live"
  return <span className={`text-sm ${up ? "text-success" : "text-warning"}`}>{up ? "daemon up" : "daemon down"}</span>
}

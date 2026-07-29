import { useConnection } from "../lib/connection"

export function ConnectionStatus() {
  const up = useConnection() === "live"
  return (
    <span className={`text-sm ${up ? "text-emerald-600" : "text-amber-600"}`}>{up ? "daemon up" : "daemon down"}</span>
  )
}

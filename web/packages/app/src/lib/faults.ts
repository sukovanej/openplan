import { useQuery } from "@tanstack/react-query"

import type { Fault, FaultKind } from "@openplan/api-client"

import { listFaults } from "./api"
import { faultsKey } from "./query-client"
import { abortable } from "./runtime"

const NONE: ReadonlyArray<Fault> = []

export function useFaults(): ReadonlyArray<Fault> {
  return (
    useQuery({
      queryKey: faultsKey,
      queryFn: abortable(listFaults),
    }).data ?? NONE
  )
}

// A project with one of these cannot show its tasks, so its pages say why instead.
const DEMOTING: ReadonlyArray<FaultKind> = ["root_gone", "unreadable", "newer_store_version"]

export function demotedReason(faults: ReadonlyArray<Fault>, project: string): string | undefined {
  return faults.find((fault) => fault.project === project && DEMOTING.includes(fault.kind))?.message
}

export function useDemotedReason(project: string): string | undefined {
  return demotedReason(useFaults(), project)
}

export function syncFailure(faults: ReadonlyArray<Fault>, project: string): string | undefined {
  return faults.find((fault) => fault.project === project && fault.kind === "sync_failed")?.message
}

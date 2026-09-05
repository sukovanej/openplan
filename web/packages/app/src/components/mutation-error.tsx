import { useMutationState } from "@tanstack/react-query"
import { useState } from "react"

import { Toast } from "@openplan/ui"

import { errorText } from "../lib/format"
import { projectMutationsKey } from "../lib/query-client"

export function MutationError() {
  const mutations = useMutationState({
    filters: {
      mutationKey: projectMutationsKey,
      predicate: (mutation) => mutation.state.status === "success" || mutation.state.status === "error",
    },
    select: (mutation) => ({
      id: mutation.mutationId,
      error: mutation.state.error,
      status: mutation.state.status,
    }),
  })
  const latest = mutations.at(-1)
  const [dismissed, setDismissed] = useState<number>()
  const error = latest?.status === "error" && latest.id !== dismissed ? latest.error : undefined
  return (
    <Toast
      role="alert"
      tone="danger"
      shape="card"
      onDismiss={() => setDismissed(latest?.id)}
      className="fixed inset-x-0 bottom-4 z-50 mx-auto w-fit max-w-xl"
    >
      {error === undefined ? undefined : errorText(error)}
    </Toast>
  )
}

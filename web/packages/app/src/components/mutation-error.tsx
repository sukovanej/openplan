import { useMutationState } from "@tanstack/react-query"
import { useState } from "react"

import { Toast } from "@open-planner/ui"

import { errorText } from "../lib/format"

export function MutationError() {
  const mutations = useMutationState({
    select: (mutation) => ({
      id: mutation.mutationId,
      error: mutation.state.error,
      status: mutation.state.status,
    }),
  })
  const latest = mutations.reduce<(typeof mutations)[number] | undefined>(
    (held, mutation) => (held === undefined || mutation.id > held.id ? mutation : held),
    undefined,
  )
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

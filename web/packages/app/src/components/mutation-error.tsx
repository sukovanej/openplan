import { Toast } from "@open-planner/ui"

import { errorText } from "../lib/format"
import { mutationError, useMutationError } from "../lib/store"

export function MutationError() {
  const error = useMutationError()
  return (
    <Toast
      role="alert"
      tone="danger"
      shape="card"
      onDismiss={mutationError.clear}
      className="fixed inset-x-0 bottom-4 z-50 mx-auto w-fit max-w-xl"
    >
      {error === undefined ? undefined : errorText(error)}
    </Toast>
  )
}

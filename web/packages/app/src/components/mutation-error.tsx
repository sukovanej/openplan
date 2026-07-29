import { X } from "lucide-react"

import { errorText } from "../lib/format"
import { mutationError, useMutationError } from "../lib/store"

export function MutationError() {
  const error = useMutationError()
  if (error === undefined) return null
  return (
    <div
      role="alert"
      className="fixed inset-x-0 bottom-4 z-50 mx-auto flex w-fit max-w-xl items-start gap-3 rounded-lg border border-red-500/30 bg-red-50 px-4 py-3 text-sm text-red-900 shadow-lg dark:bg-red-950 dark:text-red-100"
    >
      <span className="min-w-0">{errorText(error)}</span>
      <button
        type="button"
        onClick={mutationError.clear}
        aria-label="Dismiss"
        className="-mr-1 shrink-0 rounded p-0.5 hover:bg-red-500/15"
      >
        <X className="size-4" />
      </button>
    </div>
  )
}

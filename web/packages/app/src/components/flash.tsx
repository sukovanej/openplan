import { Toast } from "@openplan/ui"

import { useFlash } from "../lib/flash"

export function Flash() {
  const message = useFlash()
  return (
    <Toast
      role="status"
      live="polite"
      tone={message?.tone === "error" ? "danger" : "ok"}
      className="pointer-events-none fixed right-7 bottom-7 z-50"
    >
      {message?.text}
    </Toast>
  )
}

import { X } from "lucide-react"
import type { ComponentProps } from "react"

import { Button } from "./button"
import { cn } from "./cn"
import { Modal } from "./modal"

export function Dialog({
  title,
  className,
  children,
  ...modal
}: Omit<ComponentProps<typeof Modal>, "label"> & { title: string }) {
  return (
    <Modal
      {...modal}
      label={title}
      className={cn("bg-background w-full max-w-md rounded-xl border p-6 shadow-lg", className)}
    >
      <div className="mb-5 flex items-center justify-between">
        <h2 className="text-lg font-semibold tracking-tight">{title}</h2>
        <Button size="icon" aria-label="Close" onClick={modal.onClose}>
          <X className="size-4" aria-hidden="true" />
        </Button>
      </div>
      {children}
    </Modal>
  )
}

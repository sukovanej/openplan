import { createContext, useContext } from "react"

import type { Field_Status } from "@openplan/api-client"

// `gone`: the store holds no such task or doc.
export type TaskRead = "reading" | "gone" | { readonly status: Field_Status | undefined }
export type DocRead = "reading" | "gone" | "found"

export interface ReadRefs {
  readonly useTask: (project: string, id: string) => TaskRead
  readonly useDoc: (project: string, name: string) => DocRead
}

const nobody: ReadRefs = { useTask: () => "reading", useDoc: () => "reading" }

// The app owns the connection to the daemon, so a chip reads its task or doc through whatever it
// provides.
export const RefReader = createContext<ReadRefs>(nobody)

export const useRefReader = (): ReadRefs => useContext(RefReader)

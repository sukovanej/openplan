import type { DocFields, DocMetadata } from "@openplan/api-client"

import { fieldConflict, fieldFailure, fieldMessage, fieldValue, type FieldProblem } from "./metadata"

export type DocFieldName = keyof DocFields

const DOC_FIELD_NAMES: ReadonlyArray<DocFieldName> = ["created", "parent"]

export const docFields = (metadata: DocMetadata): DocFields | undefined => ("kind" in metadata ? undefined : metadata)

const fields = docFields

// Set when the frontmatter could not be read at all — no fence, or YAML that does not parse — so no
// field survived to be reported on its own.
export const docMetadataFailure = (metadata: DocMetadata): string | undefined =>
  "kind" in metadata ? metadata.message : undefined

export const docParentOf = (metadata: DocMetadata): string | undefined => {
  const field = fields(metadata)?.parent
  return field === undefined ? undefined : (fieldValue(field) ?? undefined)
}

export const docCreatedOf = (metadata: DocMetadata): string | undefined => {
  const field = fields(metadata)?.created
  return field === undefined ? undefined : fieldValue(field)
}

export const docConflictedFields = (metadata: DocMetadata): ReadonlyArray<DocFieldName> => {
  const found = fields(metadata)
  return found === undefined ? [] : DOC_FIELD_NAMES.filter((name) => fieldConflict(found[name]) !== undefined)
}

export function docProblems(metadata: DocMetadata): ReadonlyArray<FieldProblem> {
  const whole = docMetadataFailure(metadata)
  if (whole !== undefined) return [{ field: "frontmatter", message: whole }]
  const found = fields(metadata)
  if (found === undefined) return []
  return (["created", "parent"] as const).flatMap((field) => {
    const failure = fieldFailure(found[field])
    return failure === undefined ? [] : [{ field, message: fieldMessage(failure) }]
  })
}

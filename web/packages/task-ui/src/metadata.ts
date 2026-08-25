import type { Field_Status, FieldError, Metadata } from "@open-planner/api-client"

// Both `Metadata` and `Field<T>` are untagged unions: a value is its bare JSON, a failure is an
// object carrying `kind`. No value in the schema is an object with a `kind`, so that is what tells
// them apart.
const isFailure = (value: unknown): value is FieldError =>
  typeof value === "object" && value !== null && !Array.isArray(value) && "kind" in value

export const fieldValue = <T>(field: T | FieldError): T | undefined => (isFailure(field) ? undefined : field)

export const fieldFailure = (field: unknown): FieldError | undefined => (isFailure(field) ? field : undefined)

// Set when the frontmatter could not be read at all — no fence, or YAML that does not parse — so no
// field survived to be reported on its own.
export const metadataFailure = (metadata: Metadata): string | undefined =>
  "kind" in metadata ? metadata.message : undefined

const fields = (metadata: Metadata) => ("kind" in metadata ? undefined : metadata)

export const statusField = (metadata: Metadata): Field_Status | undefined => fields(metadata)?.status

export const parentOf = (metadata: Metadata): string | undefined => {
  const field = fields(metadata)?.parent
  return field === undefined ? undefined : (fieldValue(field) ?? undefined)
}

export const createdOf = (metadata: Metadata): string | undefined => {
  const field = fields(metadata)?.created
  return field === undefined ? undefined : fieldValue(field)
}

export const dependenciesOf = (metadata: Metadata): ReadonlyArray<string> => {
  const field = fields(metadata)?.dependencies
  return field === undefined ? [] : (fieldValue(field) ?? [])
}

export const tagsOf = (metadata: Metadata): ReadonlyArray<string> => {
  const field = fields(metadata)?.tags
  return field === undefined ? [] : (fieldValue(field) ?? [])
}

export interface FieldProblem {
  readonly field: string
  readonly message: string
}

// How a field failure reads, wherever one is shown: the message it carries, or the one word that
// stands for a field the file never named.
export const fieldMessage = (failure: FieldError): string => (failure.kind === "missing" ? "missing" : failure.message)

// Every field that failed, for a surface that reports what is wrong with a task rather than just
// that something is.
export function problems(metadata: Metadata): ReadonlyArray<FieldProblem> {
  const whole = metadataFailure(metadata)
  if (whole !== undefined) return [{ field: "frontmatter", message: whole }]
  const found = fields(metadata)
  if (found === undefined) return []
  return (["status", "created", "parent", "rank", "dependencies", "tags"] as const).flatMap((field) => {
    const failure = fieldFailure(found[field])
    return failure === undefined ? [] : [{ field, message: fieldMessage(failure) }]
  })
}

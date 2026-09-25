import type { Field_Status, FieldError, FrontmatterFields, Metadata } from "@openplan/api-client"

interface AnyConflict {
  readonly kind: "conflict"
}

export type FieldValue<F> = Exclude<F, FieldError | AnyConflict>

export type FieldConflictOf<F> = Extract<F, AnyConflict>

export type FieldName = keyof FrontmatterFields

const FIELD_NAMES: ReadonlyArray<FieldName> = ["status", "created", "parent", "rank", "dependencies", "tags"]

// Both `Metadata` and `Field<T>` are untagged unions: a value is its bare JSON, and a failure or a
// conflict is an object carrying `kind`. No value in the schema is an object with a `kind`, so that
// is what tells them apart.
const tagged = (field: unknown): field is { readonly kind: string } =>
  typeof field === "object" && field !== null && !Array.isArray(field) && "kind" in field

const isConflict = (field: unknown): field is AnyConflict & { readonly value: unknown } =>
  tagged(field) && field.kind === "conflict"

// A conflict reads as the version the remote published, which is the one in force until someone
// picks, so everything that sorts, groups, or filters by a field keeps working through one.
export const fieldValue = <F>(field: F): FieldValue<F> | undefined => {
  if (isConflict(field)) return field.value as FieldValue<F>
  return tagged(field) ? undefined : (field as FieldValue<F>)
}

export const fieldFailure = (field: unknown): FieldError | undefined =>
  tagged(field) && !isConflict(field) ? (field as FieldError) : undefined

export const fieldConflict = <F>(field: F): FieldConflictOf<F> | undefined =>
  isConflict(field) ? (field as FieldConflictOf<F>) : undefined

// Set when the frontmatter could not be read at all — no fence, or YAML that does not parse — so no
// field survived to be reported on its own.
export const metadataFailure = (metadata: Metadata): string | undefined =>
  "kind" in metadata ? metadata.message : undefined

export const frontmatterFields = (metadata: Metadata): FrontmatterFields | undefined =>
  "kind" in metadata ? undefined : metadata

export const statusField = (metadata: Metadata): Field_Status | undefined => frontmatterFields(metadata)?.status

export const parentOf = (metadata: Metadata): string | undefined => {
  const field = frontmatterFields(metadata)?.parent
  return field === undefined ? undefined : (fieldValue(field) ?? undefined)
}

export const createdOf = (metadata: Metadata): string | undefined => {
  const field = frontmatterFields(metadata)?.created
  return field === undefined ? undefined : fieldValue(field)
}

export const dependenciesOf = (metadata: Metadata): ReadonlyArray<string> => {
  const field = frontmatterFields(metadata)?.dependencies
  return field === undefined ? [] : (fieldValue(field) ?? [])
}

export const tagsOf = (metadata: Metadata): ReadonlyArray<string> => {
  const field = frontmatterFields(metadata)?.tags
  return field === undefined ? [] : (fieldValue(field) ?? [])
}

export const conflictedFields = (metadata: Metadata): ReadonlyArray<FieldName> => {
  const found = frontmatterFields(metadata)
  return found === undefined ? [] : FIELD_NAMES.filter((name) => isConflict(found[name]))
}

export interface FieldProblem {
  readonly field: string
  readonly message: string
}

// How a field failure reads, wherever one is shown: the message it carries, or the one word that
// stands for a field the file never named.
export const fieldMessage = (failure: FieldError): string => (failure.kind === "missing" ? "missing" : failure.message)

// Every field that failed, for a surface that reports what is wrong with a task rather than just
// that something is. A conflict is not one: both of its versions are readable.
export function problems(metadata: Metadata): ReadonlyArray<FieldProblem> {
  const whole = metadataFailure(metadata)
  if (whole !== undefined) return [{ field: "frontmatter", message: whole }]
  const found = frontmatterFields(metadata)
  if (found === undefined) return []
  return FIELD_NAMES.flatMap((field) => {
    const failure = fieldFailure(found[field])
    return failure === undefined ? [] : [{ field, message: fieldMessage(failure) }]
  })
}

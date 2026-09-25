import { type Effect, ManagedRuntime } from "effect"
import { FetchHttpClient, type HttpClient } from "effect/unstable/http"

export const runtime = ManagedRuntime.make(FetchHttpClient.layer)

// A query that a newer read replaces cancels its signal. Without the signal the request would run to
// its end, and the daemon would answer a read that nothing waits for.
export const abortable =
  <A, E>(effect: Effect.Effect<A, E, HttpClient.HttpClient>) =>
  ({ signal }: { readonly signal: AbortSignal }): Promise<A> =>
    runtime.runPromise(effect, { signal })

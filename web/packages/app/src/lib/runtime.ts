import { ManagedRuntime } from "effect"
import { FetchHttpClient } from "effect/unstable/http"

export const runtime = ManagedRuntime.make(FetchHttpClient.layer)

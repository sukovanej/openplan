import {
  chordOf,
  fromEvent,
  hasCommandModifier,
  isActivationKey,
  isActivationTarget,
  isEditableTarget,
  sameChord,
  startsWith,
} from "./match"
import type { Binding, RouteScope, RunContext, Scope } from "./types"

export interface DispatcherConfig {
  readonly bindings: ReadonlyArray<Binding>
  readonly routeScope: () => RouteScope
  readonly overlayOpen: () => boolean
  readonly context: () => RunContext
  readonly chordTimeoutMs?: number
}

export function activeScopes(overlayOpen: boolean, route: Scope): ReadonlyArray<Scope> {
  return overlayOpen ? ["overlay"] : ["global", route]
}

export function activeBindings(
  bindings: ReadonlyArray<Binding>,
  scopes: ReadonlyArray<Scope>,
  ctx: RunContext,
): ReadonlyArray<Binding> {
  return bindings.filter(
    (binding) => scopes.includes(binding.scope) && (binding.when === undefined || binding.when(ctx)),
  )
}

interface Match {
  readonly exact: Binding | undefined
  readonly hasPrefix: boolean
}

export class Dispatcher {
  private buffer: ReadonlyArray<string> = []
  private timer: ReturnType<typeof setTimeout> | undefined
  private readonly timeoutMs: number
  private readonly chords: Map<Binding, ReadonlyArray<string>>

  constructor(private readonly config: DispatcherConfig) {
    this.timeoutMs = config.chordTimeoutMs ?? 1000
    this.chords = new Map(config.bindings.map((binding) => [binding, chordOf(binding.keys)]))
  }

  readonly attach = (target: Window = window): () => void => {
    target.addEventListener("keydown", this.handleKeyDown)
    return () => {
      target.removeEventListener("keydown", this.handleKeyDown)
      this.reset()
    }
  }

  get pending(): ReadonlyArray<string> {
    return this.buffer
  }

  readonly handleKeyDown = (event: KeyboardEvent): void => {
    if (event.defaultPrevented) return
    if (!hasCommandModifier(event)) {
      if (isEditableTarget(event.target)) return
      if (isActivationKey(event) && isActivationTarget(event.target)) return
    }

    const ctx = this.config.context()
    const scopes = activeScopes(this.config.overlayOpen(), this.config.routeScope())
    const bindings = activeBindings(this.config.bindings, scopes, ctx)
    const token = fromEvent(event)

    if (this.dispatch(bindings, [...this.buffer, token], ctx, event)) return
    this.reset()
    this.dispatch(bindings, [token], ctx, event)
  }

  private classify(bindings: ReadonlyArray<Binding>, sequence: ReadonlyArray<string>): Match {
    let exact: Binding | undefined
    let hasPrefix = false
    for (const binding of bindings) {
      const chord = this.chords.get(binding)!
      if (sameChord(chord, sequence)) exact = binding
      else if (chord.length > sequence.length && startsWith(chord, sequence)) hasPrefix = true
    }
    return { exact, hasPrefix }
  }

  private dispatch(
    bindings: ReadonlyArray<Binding>,
    sequence: ReadonlyArray<string>,
    ctx: RunContext,
    event: KeyboardEvent,
  ): boolean {
    const { exact, hasPrefix } = this.classify(bindings, sequence)
    if (exact !== undefined) {
      event.preventDefault()
      this.reset()
      exact.run(ctx)
      return true
    }
    if (hasPrefix) {
      event.preventDefault()
      this.arm(sequence)
      return true
    }
    return false
  }

  private arm(sequence: ReadonlyArray<string>): void {
    this.buffer = sequence
    if (this.timer !== undefined) clearTimeout(this.timer)
    this.timer = setTimeout(() => this.reset(), this.timeoutMs)
  }

  private reset(): void {
    this.buffer = []
    if (this.timer !== undefined) {
      clearTimeout(this.timer)
      this.timer = undefined
    }
  }
}

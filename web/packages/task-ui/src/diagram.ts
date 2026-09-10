import type { D2, Diagram, RenderOptions } from "@terrastruct/d2"

export type DiagramTheme = "light" | "dark"

// d2's built-in theme ids: 3 is "Terrastruct", 200 is "Dark Mauve".
const THEME_IDS: Readonly<Record<DiagramTheme, number>> = { light: 3, dark: 200 }
const COMPILE_OPTIONS = { pad: 16, noXMLTag: true, sketch: true, layout: "elk" } as const

export type DiagramSize = { width: number; height: number }
export type DrawnDiagram = { svg: string; size: DiagramSize }
export type DiagramResult = { drawn: DrawnDiagram } | { error: string }

type Compiled = { diagram: Diagram; renderOptions: RenderOptions }

let engine: Promise<D2> | null = null
const compilations = new Map<string, Promise<Compiled>>()
const drawings = new Map<string, Promise<DiagramResult>>()

function ensureEngine(): Promise<D2> {
  engine ??= import("@terrastruct/d2").then(({ D2 }) => new D2())
  return engine
}

// D2.js 0.1.33 answers its worker one message at a time: two calls in flight on one instance get
// each other's replies, and two parallel renders never come back. Every call waits for the last.
let turn: Promise<unknown> = Promise.resolve()

function inTurn<T>(job: () => Promise<T>): Promise<T> {
  const next = turn.then(job, job)
  turn = next.catch(() => undefined)
  return next
}

const VIEW_BOX = /viewBox="[-\d.]+ [-\d.]+ ([\d.]+) ([\d.]+)"/

export function sizeOf(svg: string): DiagramSize {
  const match = VIEW_BOX.exec(svg)
  return { width: Number(match?.[1] ?? 0), height: Number(match?.[2] ?? 0) }
}

// The compiler rejects a source with a JSON list of `{range, errmsg}`, and each errmsg names the
// virtual input file it compiled, which the reader never sees.
export function compileErrorMessage(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error)
  try {
    const problems: unknown = JSON.parse(message)
    if (Array.isArray(problems)) {
      const lines = problems
        .map((problem: unknown) =>
          typeof problem === "object" && problem !== null && "errmsg" in problem ? String(problem.errmsg) : null,
        )
        .filter((line): line is string => line !== null)
        .map((line) => line.replace(/^index:/, ""))
      if (lines.length > 0) return lines.join("\n")
    }
  } catch {
    // Not the compiler's list, so the message stands as it is.
  }
  return message
}

function compile(source: string): Promise<Compiled> {
  let pending = compilations.get(source)
  if (pending === undefined) {
    pending = ensureEngine().then((d2) => inTurn(() => d2.compile({ fs: { index: source }, options: COMPILE_OPTIONS })))
    compilations.set(source, pending)
  }
  return pending
}

async function draw(source: string, theme: DiagramTheme): Promise<DiagramResult> {
  try {
    const [d2, { diagram, renderOptions }] = await Promise.all([ensureEngine(), compile(source)])
    const svg = await inTurn(() => d2.render(diagram, { ...renderOptions, themeID: THEME_IDS[theme] }))
    return { drawn: { svg, size: sizeOf(svg) } }
  } catch (error) {
    return { error: compileErrorMessage(error) }
  }
}

export function drawDiagram(source: string, theme: DiagramTheme): Promise<DiagramResult> {
  const key = `${theme}\n${source}`
  let pending = drawings.get(key)
  if (pending === undefined) {
    pending = draw(source, theme)
    drawings.set(key, pending)
  }
  return pending
}

export function svgDataUrl(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`
}

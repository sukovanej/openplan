import type { D2 } from "@terrastruct/d2"

// d2's built-in theme ids: 0 is "Neutral default", 200 is "Dark Mauve".
const LIGHT_THEME = 0
const DARK_THEME = 200

export type DiagramSize = { width: number; height: number }
export type DrawnDiagram = { light: string; dark: string; size: DiagramSize }
export type DiagramResult = { svg: DrawnDiagram } | { error: string }

let engine: Promise<D2> | null = null
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

async function draw(source: string): Promise<DiagramResult> {
  try {
    const d2 = await ensureEngine()
    const { diagram, renderOptions } = await inTurn(() =>
      d2.compile({ fs: { index: source }, options: { pad: 16, noXMLTag: true } }),
    )
    const light = await inTurn(() => d2.render(diagram, { ...renderOptions, themeID: LIGHT_THEME }))
    const dark = await inTurn(() => d2.render(diagram, { ...renderOptions, themeID: DARK_THEME }))
    return { svg: { light, dark, size: sizeOf(light) } }
  } catch (error) {
    return { error: compileErrorMessage(error) }
  }
}

export function drawDiagram(source: string): Promise<DiagramResult> {
  let pending = drawings.get(source)
  if (pending === undefined) {
    pending = draw(source)
    drawings.set(source, pending)
  }
  return pending
}

export function svgDataUrl(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`
}

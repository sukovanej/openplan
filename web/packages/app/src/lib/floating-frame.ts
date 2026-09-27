import { type PointerEvent as ReactPointerEvent, type RefObject, useEffect, useRef, useState } from "react"

export interface Frame {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
}

export interface Size {
  readonly width: number
  readonly height: number
}

export interface Edges {
  readonly left?: boolean
  readonly right?: boolean
  readonly top?: boolean
  readonly bottom?: boolean
}

export const MIN_SIZE: Size = { width: 360, height: 160 }

const STORAGE_KEY = "openplan.prompt-bar.frame"

export function clampFrame(frame: Frame, viewport: Size): Frame {
  const width = Math.min(Math.max(frame.width, MIN_SIZE.width), viewport.width)
  const height = Math.min(Math.max(frame.height, MIN_SIZE.height), viewport.height)
  return {
    x: Math.min(Math.max(frame.x, 0), viewport.width - width),
    y: Math.min(Math.max(frame.y, 0), viewport.height - height),
    width,
    height,
  }
}

export function moveFrame(start: Frame, dx: number, dy: number, viewport: Size): Frame {
  return clampFrame({ ...start, x: start.x + dx, y: start.y + dy }, viewport)
}

// The dragged edge moves and the opposite one stays, so a frame at its least size or at the window's
// edge stops rather than sliding.
export function resizeFrame(start: Frame, edges: Edges, dx: number, dy: number, viewport: Size): Frame {
  let { x, y, width, height } = start
  if (edges.left === true) {
    const right = start.x + start.width
    x = Math.min(Math.max(start.x + dx, 0), right - MIN_SIZE.width)
    width = right - x
  }
  if (edges.right === true) {
    width = Math.min(Math.max(start.width + dx, MIN_SIZE.width), viewport.width - start.x)
  }
  if (edges.top === true) {
    const bottom = start.y + start.height
    y = Math.min(Math.max(start.y + dy, 0), bottom - MIN_SIZE.height)
    height = bottom - y
  }
  if (edges.bottom === true) {
    height = Math.min(Math.max(start.height + dy, MIN_SIZE.height), viewport.height - start.y)
  }
  return { x, y, width, height }
}

export function parseFrame(stored: string | null): Frame | undefined {
  if (stored === null) return undefined
  let value: unknown
  try {
    value = JSON.parse(stored)
  } catch {
    return undefined
  }
  if (typeof value !== "object" || value === null) return undefined
  const { x, y, width, height } = value as Record<string, unknown>
  const numbers = [x, y, width, height].every((field) => typeof field === "number" && Number.isFinite(field))
  return numbers ? ({ x, y, width, height } as Frame) : undefined
}

function viewport(): Size {
  return { width: window.innerWidth, height: window.innerHeight }
}

function readFrame(): Frame | undefined {
  let stored: string | null = null
  try {
    stored = window.localStorage.getItem(STORAGE_KEY)
  } catch {
    stored = null
  }
  const frame = parseFrame(stored)
  return frame === undefined ? undefined : clampFrame(frame, viewport())
}

function writeFrame(frame: Frame | undefined): void {
  try {
    if (frame === undefined) window.localStorage.removeItem(STORAGE_KEY)
    else window.localStorage.setItem(STORAGE_KEY, JSON.stringify(frame))
  } catch {
    // A blocked or unavailable localStorage keeps the frame for this page load only.
  }
}

export interface FloatingFrame {
  readonly ref: RefObject<HTMLElement | null>
  // Undefined until the reader moves or resizes the element; it then keeps its own place and size.
  readonly frame: Frame | undefined
  readonly move: (event: ReactPointerEvent) => void
  readonly resize: (edges: Edges) => (event: ReactPointerEvent) => void
  readonly reset: () => void
}

export function useFloatingFrame(): FloatingFrame {
  const ref = useRef<HTMLElement>(null)
  const [frame, setFrame] = useState(readFrame)

  useEffect(() => {
    const onResize = () => setFrame((current) => current && clampFrame(current, viewport()))
    window.addEventListener("resize", onResize)
    return () => window.removeEventListener("resize", onResize)
  }, [])

  const drag = (event: ReactPointerEvent, next: (start: Frame, dx: number, dy: number) => Frame) => {
    const node = ref.current
    if (event.button !== 0 || node === null) return
    // Taking the press keeps the prompt's focus and the page's text selection where they are.
    event.preventDefault()
    event.currentTarget.setPointerCapture(event.pointerId)
    const box = node.getBoundingClientRect()
    const start = frame ?? { x: box.left, y: box.top, width: box.width, height: box.height }
    const { clientX, clientY } = event
    let last = start
    const onMove = (moved: PointerEvent) => {
      last = next(start, moved.clientX - clientX, moved.clientY - clientY)
      setFrame(last)
    }
    const onEnd = () => {
      window.removeEventListener("pointermove", onMove)
      window.removeEventListener("pointerup", onEnd)
      window.removeEventListener("pointercancel", onEnd)
      writeFrame(last)
    }
    window.addEventListener("pointermove", onMove)
    window.addEventListener("pointerup", onEnd)
    window.addEventListener("pointercancel", onEnd)
  }

  return {
    ref,
    frame,
    move: (event) => drag(event, (start, dx, dy) => moveFrame(start, dx, dy, viewport())),
    resize: (edges) => (event) => drag(event, (start, dx, dy) => resizeFrame(start, edges, dx, dy, viewport())),
    reset: () => {
      setFrame(undefined)
      writeFrame(undefined)
    },
  }
}

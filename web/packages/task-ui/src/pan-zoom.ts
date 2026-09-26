export interface Size {
  readonly width: number
  readonly height: number
}

export interface PanZoom {
  fit(content: Size, floor?: number): void
  zoomBy(factor: number): void
  dispose(): void
}

export const MIN_SCALE = 0.1
export const MAX_SCALE = 4
const FIT_PADDING = 24
const DRAG_SLOP = 3
const LINE_HEIGHT = 16
// A mouse wheel under Ctrl sends a hundred pixels a notch, a pinch sends a few an event; the cap
// keeps one notch from jumping past what the reader aimed at.
const ZOOM_RATE = 0.01
const MOST_ZOOM_A_STEP = 0.5
// The browser draws a `will-change` layer as a texture and scales that texture, so text goes soft
// until the layer is dropped and redrawn.
const SETTLE_MS = 150

type Point = { readonly x: number; readonly y: number }

// Safari sends a trackpad pinch as gesture events and no wheel event.
type GestureEvent = Event & { readonly scale: number; readonly clientX: number; readonly clientY: number }

const clamp = (value: number, low: number, high: number) => Math.min(high, Math.max(low, value))

// The transform lives here and goes to the layer's style directly: a React render for each frame
// would cost more than the frame has.
export function panZoom(frame: HTMLElement, layer: HTMLElement): PanZoom {
  let x = 0
  let y = 0
  let k = 1
  let settle: ReturnType<typeof setTimeout> | undefined

  const paint = () => {
    layer.style.transform = `translate(${x}px, ${y}px) scale(${k})`
  }
  const moving = () => {
    layer.style.willChange = "transform"
    clearTimeout(settle)
    settle = setTimeout(() => {
      layer.style.willChange = ""
    }, SETTLE_MS)
  }
  const inFrame = (clientX: number, clientY: number): Point => {
    const box = frame.getBoundingClientRect()
    return { x: clientX - box.left, y: clientY - box.top }
  }
  const zoomAt = (at: Point, scale: number) => {
    const next = clamp(scale, MIN_SCALE, MAX_SCALE)
    x = at.x - ((at.x - x) * next) / k
    y = at.y - ((at.y - y) * next) / k
    k = next
    moving()
    paint()
  }
  const panBy = (dx: number, dy: number) => {
    x += dx
    y += dy
    moving()
    paint()
  }

  let gesturing = false
  let gestureScale = 1

  const onWheel = (event: WheelEvent) => {
    event.preventDefault()
    const unit = event.deltaMode === 1 ? LINE_HEIGHT : event.deltaMode === 2 ? frame.clientHeight : 1
    if (event.ctrlKey || event.metaKey) {
      if (gesturing) return
      const step = clamp(-event.deltaY * unit * ZOOM_RATE, -MOST_ZOOM_A_STEP, MOST_ZOOM_A_STEP)
      zoomAt(inFrame(event.clientX, event.clientY), k * Math.exp(step))
      return
    }
    // A mouse without a horizontal wheel scrolls sideways with Shift, and only Safari and Chrome on
    // macOS turn that into a horizontal delta themselves.
    const sideways = event.shiftKey && event.deltaX === 0
    panBy(-(sideways ? event.deltaY : event.deltaX) * unit, -(sideways ? 0 : event.deltaY) * unit)
  }

  const onGestureStart = (event: Event) => {
    event.preventDefault()
    gesturing = true
    gestureScale = k
  }
  const onGestureChange = (event: Event) => {
    event.preventDefault()
    const gesture = event as GestureEvent
    zoomAt(inFrame(gesture.clientX, gesture.clientY), gestureScale * gesture.scale)
  }
  const onGestureEnd = (event: Event) => {
    event.preventDefault()
    gesturing = false
  }

  const pointers = new Map<number, Point>()
  let pressedAt: Point | undefined
  let dragged = false
  let pinch: { readonly spread: number; readonly scale: number } | undefined

  const spread = () => {
    const [a, b] = [...pointers.values()]
    return Math.hypot(a!.x - b!.x, a!.y - b!.y)
  }
  const midpoint = (): Point => {
    const [a, b] = [...pointers.values()]
    return { x: (a!.x + b!.x) / 2, y: (a!.y + b!.y) / 2 }
  }

  const onPointerDown = (event: PointerEvent) => {
    if (event.pointerType === "mouse" && event.button !== 0) return
    pointers.set(event.pointerId, { x: event.clientX, y: event.clientY })
    if (pointers.size === 1) {
      pressedAt = { x: event.clientX, y: event.clientY }
      dragged = false
    } else if (pointers.size === 2) {
      pinch = { spread: spread(), scale: k }
    }
  }
  const onPointerMove = (event: PointerEvent) => {
    const last = pointers.get(event.pointerId)
    if (last === undefined) return
    const now = { x: event.clientX, y: event.clientY }
    if (pointers.size === 1) {
      // A press that barely moves is a click, so a card still opens under a shaky hand.
      if (!dragged) {
        if (pressedAt === undefined || Math.hypot(now.x - pressedAt.x, now.y - pressedAt.y) < DRAG_SLOP) return
        dragged = true
        frame.setPointerCapture(event.pointerId)
        frame.dataset.panning = ""
      }
      pointers.set(event.pointerId, now)
      panBy(now.x - last.x, now.y - last.y)
      return
    }
    if (pinch === undefined) return
    const before = midpoint()
    pointers.set(event.pointerId, now)
    const after = midpoint()
    dragged = true
    panBy(after.x - before.x, after.y - before.y)
    zoomAt(inFrame(after.x, after.y), (pinch.scale * spread()) / pinch.spread)
  }
  const onPointerUp = (event: PointerEvent) => {
    pointers.delete(event.pointerId)
    if (pointers.size < 2) pinch = undefined
    if (pointers.size === 0) delete frame.dataset.panning
  }
  // The click that ends a drag lands on whatever card is under the pointer; the drag was not
  // meant to open it.
  const onClick = (event: MouseEvent) => {
    if (!dragged) return
    dragged = false
    event.preventDefault()
    event.stopPropagation()
  }

  frame.addEventListener("wheel", onWheel, { passive: false })
  frame.addEventListener("gesturestart", onGestureStart)
  frame.addEventListener("gesturechange", onGestureChange)
  frame.addEventListener("gestureend", onGestureEnd)
  frame.addEventListener("pointerdown", onPointerDown)
  frame.addEventListener("pointermove", onPointerMove)
  frame.addEventListener("pointerup", onPointerUp)
  frame.addEventListener("pointercancel", onPointerUp)
  frame.addEventListener("click", onClick, { capture: true })

  return {
    // Centered at the largest scale that shows it all, up to its own size. Below `floor` the reader
    // pans to the rest instead.
    fit(content, floor = MIN_SCALE) {
      const width = frame.clientWidth
      const height = frame.clientHeight
      if (content.width === 0 || content.height === 0 || width === 0 || height === 0) return
      const whole = Math.min((width - FIT_PADDING * 2) / content.width, (height - FIT_PADDING * 2) / content.height)
      k = clamp(Math.min(1, Math.max(floor, whole)), MIN_SCALE, MAX_SCALE)
      x = (width - content.width * k) / 2
      y = (height - content.height * k) / 2
      paint()
    },
    zoomBy(factor) {
      zoomAt({ x: frame.clientWidth / 2, y: frame.clientHeight / 2 }, k * factor)
    },
    dispose() {
      clearTimeout(settle)
      frame.removeEventListener("wheel", onWheel)
      frame.removeEventListener("gesturestart", onGestureStart)
      frame.removeEventListener("gesturechange", onGestureChange)
      frame.removeEventListener("gestureend", onGestureEnd)
      frame.removeEventListener("pointerdown", onPointerDown)
      frame.removeEventListener("pointermove", onPointerMove)
      frame.removeEventListener("pointerup", onPointerUp)
      frame.removeEventListener("pointercancel", onPointerUp)
      frame.removeEventListener("click", onClick, { capture: true })
    },
  }
}

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import fontUrl from '../../../../../../crates/of-drawing/assets/fonts/DejaVuSansCondensed.ttf?url'
import { useStore } from '../store'
import { EMPTY_OVERLAY, type SheetPick, itemMatches, useDrawing } from './state'
import type { Item, P2, Prim, Sheet, SheetRender } from './types'
import { paperSize } from './types'

const FONT_FAMILY = 'DejaVu Sans Condensed'
let fontReady: Promise<void> | null = null

/** Load the drafting font the core lays text out with. */
function loadFont(): Promise<void> {
  fontReady ??= new FontFace(FONT_FAMILY, `url(${fontUrl})`)
    .load()
    .then((f) => {
      document.fonts.add(f)
    })
    .catch(() => {})
  return fontReady
}

/** Dash patterns in mm of paper, as the core writes them (ISO 128-20). */
const DASHES: Record<string, number[]> = {
  continuous: [],
  hidden: [3, 1.5],
  center: [8, 1.5, 0.5, 1.5],
  phantom: [8, 1.5, 0.5, 1.5, 0.5, 1.5],
}

const SELECT = '#1f6feb'
const HOVER = '#58a6ff'
const PICK_PX = 6

interface Camera {
  /** Sheet point at the canvas center, mm. */
  cx: number
  cy: number
  /** Pixels (CSS) per mm. */
  zoom: number
}

/** A drag in progress: moving views or annotations, or panning. */
type Drag =
  | { type: 'pan'; start: P2; camera: Camera }
  | { type: 'move'; start: P2; picks: SheetPick[]; offset: P2; moved: boolean }

function dist(a: P2, b: P2): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1])
}

function segmentDistance(p: P2, a: P2, b: P2): number {
  const dx = b[0] - a[0]
  const dy = b[1] - a[1]
  const len2 = dx * dx + dy * dy
  const t =
    len2 > 0 ? Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2)) : 0
  return dist(p, [a[0] + dx * t, a[1] + dy * t])
}

function angleIn(a: number, start: number, end: number): boolean {
  const TAU = Math.PI * 2
  if (end - start >= TAU - 1e-9) return true
  const norm = (x: number) => ((x % TAU) + TAU) % TAU
  const s = norm(start)
  const span = end - start
  return norm(a - s) <= span + 1e-9
}

function insidePolygon(p: P2, poly: P2[]): boolean {
  let inside = false
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const [xi, yi] = poly[i]!
    const [xj, yj] = poly[j]!
    if (yi > p[1] !== yj > p[1] && p[0] < ((xj - xi) * (p[1] - yi)) / (yj - yi) + xi)
      inside = !inside
  }
  return inside
}

/** Distance from a sheet point to a primitive, mm (0 inside fills and text). */
export function primDistance(prim: Prim, p: P2): number {
  switch (prim.type) {
    case 'line':
      return segmentDistance(p, prim.a, prim.b)
    case 'arc': {
      const a = Math.atan2(p[1] - prim.center[1], p[0] - prim.center[0])
      if (angleIn(a, prim.start, prim.end)) return Math.abs(dist(p, prim.center) - prim.radius)
      const ends = [prim.start, prim.end].map(
        (t) =>
          [
            prim.center[0] + prim.radius * Math.cos(t),
            prim.center[1] + prim.radius * Math.sin(t),
          ] as P2,
      )
      return Math.min(...ends.map((e) => dist(p, e)))
    }
    case 'polyline': {
      let best = Infinity
      const pts = prim.points
      for (let i = 1; i < pts.length; i++)
        best = Math.min(best, segmentDistance(p, pts[i - 1]!, pts[i]!))
      if (prim.closed && pts.length > 2)
        best = Math.min(best, segmentDistance(p, pts[pts.length - 1]!, pts[0]!))
      return best
    }
    case 'fill':
      return prim.loops.some((l) => insidePolygon(p, l)) ? 0 : Infinity
    case 'text': {
      const angle = prim.angle ?? 0
      const c = Math.cos(-angle)
      const s = Math.sin(-angle)
      const dx = p[0] - prim.at[0]
      const dy = p[1] - prim.at[1]
      const u = dx * c - dy * s
      const v = dx * s + dy * c
      const h = prim.height * 0.75
      const du = u < 0 ? -u : u > prim.width ? u - prim.width : 0
      const dv = v < -h * 0.25 ? -h * 0.25 - v : v > h ? v - h : 0
      return Math.hypot(du, dv)
    }
  }
}

/** Points a placement can snap to: ends, midpoints and centers. */
function snapPoints(prim: Prim): P2[] {
  switch (prim.type) {
    case 'line':
      return [prim.a, prim.b, [(prim.a[0] + prim.b[0]) / 2, (prim.a[1] + prim.b[1]) / 2]]
    case 'arc': {
      const out: P2[] = [prim.center]
      if (prim.end - prim.start < Math.PI * 2 - 1e-9)
        for (const t of [prim.start, prim.end])
          out.push([
            prim.center[0] + prim.radius * Math.cos(t),
            prim.center[1] + prim.radius * Math.sin(t),
          ])
      return out
    }
    case 'polyline':
      return prim.points.length < 40
        ? prim.points
        : [prim.points[0]!, prim.points[prim.points.length - 1]!]
    default:
      return []
  }
}

/** Move a primitive by an offset (for drag previews). */
function shift(prim: Prim, d: P2): Prim {
  const m = (p: P2): P2 => [p[0] + d[0], p[1] + d[1]]
  switch (prim.type) {
    case 'line':
      return { ...prim, a: m(prim.a), b: m(prim.b) }
    case 'arc':
      return { ...prim, center: m(prim.center) }
    case 'polyline':
      return { ...prim, points: prim.points.map(m) }
    case 'fill':
      return { ...prim, loops: prim.loops.map((l) => l.map(m)) }
    case 'text':
      return { ...prim, at: m(prim.at) }
  }
}

function drawPrim(ctx: CanvasRenderingContext2D, prim: Prim, toPx: (p: P2) => P2, zoom: number) {
  switch (prim.type) {
    case 'line': {
      const a = toPx(prim.a)
      const b = toPx(prim.b)
      ctx.beginPath()
      ctx.moveTo(a[0], a[1])
      ctx.lineTo(b[0], b[1])
      ctx.stroke()
      return
    }
    case 'arc': {
      const c = toPx(prim.center)
      ctx.beginPath()
      // Sheet y is up: angles turn the other way on screen.
      ctx.arc(c[0], c[1], prim.radius * zoom, -prim.start, -prim.end, true)
      ctx.stroke()
      return
    }
    case 'polyline': {
      if (prim.points.length < 2) return
      ctx.beginPath()
      const f = toPx(prim.points[0]!)
      ctx.moveTo(f[0], f[1])
      for (const p of prim.points.slice(1)) {
        const q = toPx(p)
        ctx.lineTo(q[0], q[1])
      }
      if (prim.closed) ctx.closePath()
      ctx.stroke()
      return
    }
    case 'fill': {
      ctx.beginPath()
      for (const l of prim.loops) {
        if (l.length < 3) continue
        const f = toPx(l[0]!)
        ctx.moveTo(f[0], f[1])
        for (const p of l.slice(1)) {
          const q = toPx(p)
          ctx.lineTo(q[0], q[1])
        }
        ctx.closePath()
      }
      ctx.fill('evenodd')
      return
    }
    case 'text': {
      const size = prim.height * zoom
      if (size < 0.8) return
      const at = toPx(prim.at)
      ctx.save()
      ctx.translate(at[0], at[1])
      ctx.rotate(-(prim.angle ?? 0))
      ctx.font = `${size}px "${FONT_FAMILY}"`
      // Match the core's advance width exactly.
      const measured = ctx.measureText(prim.text).width
      const want = prim.width * zoom
      if (measured > 0 && want > 0) ctx.scale(want / measured, 1)
      ctx.fillText(prim.text, 0, 0)
      ctx.restore()
      return
    }
  }
}

/** The sheet: renders the core's display list; picks, moves and places. */
export function SheetCanvas() {
  const drawing = useStore((s) => s.drawing)
  const sheetId = useDrawing((s) => s.sheet)
  const render = useDrawing((s) => s.render)
  const selection = useDrawing((s) => s.selection)
  const target = useDrawing((s) => s.target)
  const overlay = useDrawing((s) => s.overlay)
  const fitSeq = useDrawing((s) => s.fitSeq)
  const canvas = useRef<HTMLCanvasElement>(null)
  const wrap = useRef<HTMLDivElement>(null)
  const [camera, setCamera] = useState<Camera>({ cx: 210, cy: 148, zoom: 2 })
  const [hover, setHover] = useState<SheetPick | null>(null)
  const [snap, setSnap] = useState<P2 | null>(null)
  const [cursor, setCursor] = useState<P2 | null>(null)
  const [, setFontTick] = useState(0)
  const drag = useRef<Drag | null>(null)
  const [dragOffset, setDragOffset] = useState<{ picks: SheetPick[]; offset: P2 } | null>(null)

  const sheet: Sheet | undefined = drawing?.drawing.sheets.find((s) => s.id === sheetId)
  const paper = useMemo<P2>(() => (sheet ? paperSize(sheet) : [420, 297]), [sheet])

  useEffect(() => {
    void loadFont().then(() => setFontTick((t) => t + 1))
  }, [])

  // Zoom to the sheet.
  useEffect(() => {
    const el = wrap.current
    if (!el) return
    const w = el.clientWidth || 800
    const h = el.clientHeight || 600
    const zoom = Math.min((w - 40) / paper[0], (h - 40) / paper[1])
    setCamera({ cx: paper[0] / 2, cy: paper[1] / 2, zoom: Math.max(zoom, 0.05) })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fitSeq, sheetId, paper[0], paper[1]])

  const size = useRef<P2>([800, 600])
  const toPx = useCallback(
    (p: P2): P2 => [
      size.current[0] / 2 + (p[0] - camera.cx) * camera.zoom,
      size.current[1] / 2 - (p[1] - camera.cy) * camera.zoom,
    ],
    [camera],
  )
  const toSheet = useCallback(
    (x: number, y: number): P2 => [
      camera.cx + (x - size.current[0] / 2) / camera.zoom,
      camera.cy - (y - size.current[1] / 2) / camera.zoom,
    ],
    [camera],
  )

  // Draw.
  useEffect(() => {
    const el = canvas.current
    const box = wrap.current
    if (!el || !box) return
    const draw = () => {
      const dpr = window.devicePixelRatio || 1
      const w = box.clientWidth
      const h = box.clientHeight
      size.current = [w, h]
      if (el.width !== Math.round(w * dpr) || el.height !== Math.round(h * dpr)) {
        el.width = Math.round(w * dpr)
        el.height = Math.round(h * dpr)
        el.style.width = `${w}px`
        el.style.height = `${h}px`
      }
      const ctx = el.getContext('2d')!
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
      ctx.fillStyle = '#8a9099'
      ctx.fillRect(0, 0, w, h)
      // Paper with a shadow.
      const lo = toPx([0, paper[1]])
      const hi = toPx([paper[0], 0])
      ctx.fillStyle = 'rgba(0,0,0,0.25)'
      ctx.fillRect(lo[0] + 4, lo[1] + 4, hi[0] - lo[0], hi[1] - lo[1])
      ctx.fillStyle = '#ffffff'
      ctx.fillRect(lo[0], lo[1], hi[0] - lo[0], hi[1] - lo[1])
      if (!render || render.sheet !== sheetId) return
      ctx.lineCap = 'round'
      ctx.lineJoin = 'round'
      const moving = dragOffset
      for (const item of render.graphics.items) {
        const selected = selection.some((p) => itemMatches(item, p))
        const hovered = !selected && !!hover && itemMatches(item, hover)
        const offset = moving?.picks.some((p) => itemMatches(item, p)) ? moving.offset : null
        const prim = offset ? shift(item.prim, offset) : item.prim
        const c = item.pen.color
        const color = selected ? SELECT : hovered ? HOVER : `rgb(${c[0]},${c[1]},${c[2]})`
        ctx.strokeStyle = color
        ctx.fillStyle = color
        ctx.lineWidth = Math.max(item.pen.weight * camera.zoom, selected || hovered ? 1.6 : 0.6)
        const dashes = DASHES[item.pen.style] ?? []
        // Dashes grow with line weight relative to 0.25 mm (ISO 128-20).
        const k = (item.pen.weight / 0.25) * camera.zoom
        ctx.setLineDash(dashes.map((d) => d * Math.max(k, 0.5)))
        drawPrim(ctx, prim, toPx, camera.zoom)
      }
      ctx.setLineDash([])
      // Stale and failed views.
      for (const v of render.views) {
        if (!v.bbox || (!v.stale && !v.error)) continue
        const a = toPx([v.bbox[0][0] - 2, v.bbox[1][1] + 2])
        const b = toPx([v.bbox[1][0] + 2, v.bbox[0][1] - 2])
        ctx.strokeStyle = v.error ? '#d1242f' : '#bf8700'
        ctx.setLineDash([6, 4])
        ctx.lineWidth = 1
        ctx.strokeRect(a[0], a[1], b[0] - a[0], b[1] - a[1])
        ctx.setLineDash([])
      }
      // Selected views get their box.
      for (const p of selection) {
        if (p.type !== 'view') continue
        const v = render.views.find((x) => x.id === p.view)
        if (!v?.bbox) continue
        const off: P2 = moving?.picks.some((x) => x.type === 'view' && x.view === p.view)
          ? moving.offset
          : [0, 0]
        const a = toPx([v.bbox[0][0] - 3 + off[0], v.bbox[1][1] + 3 + off[1]])
        const b = toPx([v.bbox[1][0] + 3 + off[0], v.bbox[0][1] - 3 + off[1]])
        ctx.strokeStyle = SELECT
        ctx.setLineDash([4, 3])
        ctx.lineWidth = 1
        ctx.strokeRect(a[0], a[1], b[0] - a[0], b[1] - a[1])
        ctx.setLineDash([])
      }
      // Panel overlays.
      ctx.strokeStyle = '#cf222e'
      ctx.fillStyle = '#cf222e'
      ctx.lineWidth = 1.2
      ctx.setLineDash([5, 3])
      for (const [a, b] of overlay.lines) drawPrim(ctx, { type: 'line', a, b }, toPx, camera.zoom)
      for (const c of overlay.circles)
        drawPrim(
          ctx,
          { type: 'arc', center: c.center, radius: c.radius, start: 0, end: Math.PI * 2 },
          toPx,
          camera.zoom,
        )
      ctx.setLineDash([])
      for (const p of overlay.points) {
        const q = toPx(p)
        ctx.fillRect(q[0] - 3, q[1] - 3, 6, 6)
      }
      if (snap && target) {
        const q = toPx(snap)
        ctx.strokeStyle = '#1a7f37'
        ctx.lineWidth = 1.5
        ctx.strokeRect(q[0] - 5, q[1] - 5, 10, 10)
      }
    }
    draw()
    const observer = new ResizeObserver(draw)
    observer.observe(box)
    return () => observer.disconnect()
  }, [render, sheetId, camera, selection, hover, overlay, snap, target, dragOffset, toPx, paper])

  /** What lies under a sheet point. */
  const pickAt = useCallback(
    (at: P2): SheetPick => {
      const tol = PICK_PX / camera.zoom
      let best: { item: Item; d: number } | null = null
      for (const item of render?.graphics.items ?? []) {
        if (item.owner.type === 'format') continue
        const d = primDistance(item.prim, at) - (item.owner.type === 'annotation' ? tol * 0.3 : 0)
        if (d <= tol && (!best || d < best.d)) best = { item, d }
      }
      const viewAt = (p: P2) =>
        render?.views.find(
          (v) =>
            v.bbox &&
            p[0] >= v.bbox[0][0] &&
            p[0] <= v.bbox[1][0] &&
            p[1] >= v.bbox[0][1] &&
            p[1] <= v.bbox[1][1],
        )?.id ?? null
      if (!best) return { type: 'point', at, view: viewAt(at) }
      const o = best.item.owner
      if (o.type === 'annotation') return { type: 'annotation', id: o.id, at }
      if (o.type === 'view' && o.entity)
        return { type: 'entity', view: o.view, entity: o.entity, at, prim: best.item.prim }
      if (o.type === 'view') return { type: 'view', view: o.view, at }
      return { type: 'point', at, view: viewAt(at) }
    },
    [render, camera.zoom],
  )

  /** The nearest snap point within reach, for placements. */
  const snapAt = useCallback(
    (at: P2): P2 | null => {
      const tol = 8 / camera.zoom
      let best: { p: P2; d: number } | null = null
      for (const item of render?.graphics.items ?? []) {
        if (item.owner.type !== 'view') continue
        for (const p of snapPoints(item.prim)) {
          const d = dist(p, at)
          if (d <= tol && (!best || d < best.d)) best = { p, d }
        }
      }
      return best?.p ?? null
    },
    [render, camera.zoom],
  )

  const local = (e: React.PointerEvent | React.WheelEvent | React.MouseEvent): [number, number] => {
    const r = canvas.current!.getBoundingClientRect()
    return [e.clientX - r.left, e.clientY - r.top]
  }

  const onPointerDown = (e: React.PointerEvent) => {
    canvas.current!.setPointerCapture(e.pointerId)
    const [x, y] = local(e)
    const at = toSheet(x, y)
    if (e.button === 1 || e.button === 2 || (e.button === 0 && e.altKey)) {
      drag.current = { type: 'pan', start: [x, y], camera }
      return
    }
    if (e.button !== 0) return
    if (target) {
      const pick = pickAt(at)
      const snapped = snapAt(at)
      const place = snapped ?? at
      const routed: SheetPick | null = target.kinds.includes(pick.type)
        ? pick.type === 'point'
          ? { ...pick, at: place }
          : pick
        : target.kinds.includes('point')
          ? {
              type: 'point',
              at: place,
              view: pick.type === 'point' ? pick.view : 'view' in pick ? pick.view : null,
            }
          : null
      if (routed) target.onPick(routed)
      else useStore.getState().toast(target.prompt, 'info')
      return
    }
    const pick = pickAt(at)
    const ds = useDrawing.getState()
    if (pick.type === 'point') {
      // Empty paper inside a view's box picks the view.
      if (pick.view !== null) {
        const vp: SheetPick = { type: 'view', view: pick.view, at }
        ds.select(vp, e.shiftKey || e.metaKey || e.ctrlKey)
        drag.current = { type: 'move', start: at, picks: [vp], offset: [0, 0], moved: false }
      } else {
        ds.select(null)
        drag.current = { type: 'pan', start: [x, y], camera }
      }
      return
    }
    const additive = e.shiftKey || e.metaKey || e.ctrlKey
    const already = ds.selection.some(
      (s) => JSON.stringify({ ...s, at: 0 }) === JSON.stringify({ ...pick, at: 0 }),
    )
    if (!already) ds.select(pick, additive)
    // Dragging an edge moves its view; dragging an annotation moves it.
    const movable: SheetPick[] = (already ? useDrawing.getState().selection : [pick]).map((p) =>
      p.type === 'entity' ? { type: 'view', view: p.view, at: p.at } : p,
    )
    drag.current = { type: 'move', start: at, picks: movable, offset: [0, 0], moved: false }
  }

  const onPointerMove = (e: React.PointerEvent) => {
    const [x, y] = local(e)
    const at = toSheet(x, y)
    setCursor(at)
    const d = drag.current
    if (d?.type === 'pan') {
      setCamera({
        ...d.camera,
        cx: d.camera.cx - (x - d.start[0]) / d.camera.zoom,
        cy: d.camera.cy + (y - d.start[1]) / d.camera.zoom,
      })
      return
    }
    if (d?.type === 'move') {
      const offset: P2 = [at[0] - d.start[0], at[1] - d.start[1]]
      if (!d.moved && Math.hypot(offset[0], offset[1]) * camera.zoom < 4) return
      d.moved = true
      d.offset = offset
      setDragOffset({ picks: d.picks, offset })
      return
    }
    if (target) {
      const s = snapAt(at)
      setSnap(s)
      target.onHover?.(s ?? at)
      setHover(target.kinds.some((k) => k !== 'point') ? pickAt(at) : null)
      return
    }
    const p = pickAt(at)
    setHover(p.type === 'point' ? null : p)
  }

  const onPointerUp = async () => {
    const d = drag.current
    drag.current = null
    if (d?.type !== 'move' || !d.moved) return
    const [dx, dy] = d.offset
    const views = [
      ...new Set(d.picks.filter((p) => p.type === 'view').map((p) => (p as { view: number }).view)),
    ]
    const notes = d.picks
      .filter((p) => p.type === 'annotation')
      .map((p) => (p as { id: number }).id)
    const s = useStore.getState()
    for (const id of views) {
      const v = s.drawing?.drawing.sheets.flatMap((x) => x.views).find((x) => x.id === id)
      if (!v) continue
      await s.run('drawing.view.update', { id, origin: [v.origin[0] + dx, v.origin[1] + dy] })
    }
    if (notes.length) await s.run('drawing.annotation.move', { ids: notes, by: [dx, dy] })
    setDragOffset(null)
  }

  const onWheel = (e: React.WheelEvent) => {
    const [x, y] = local(e)
    const before = toSheet(x, y)
    const zoom = Math.min(Math.max(camera.zoom * Math.exp(-e.deltaY * 0.0015), 0.05), 200)
    // Keep the point under the cursor fixed.
    setCamera({
      zoom,
      cx: before[0] - (x - size.current[0] / 2) / zoom,
      cy: before[1] + (y - size.current[1] / 2) / zoom,
    })
  }

  const onDoubleClick = (e: React.MouseEvent) => {
    if (target) return
    const [x, y] = local(e)
    const pick = pickAt(toSheet(x, y))
    const s = useStore.getState()
    if (s.panel) return
    if (pick.type === 'annotation') s.openPanel({ type: 'annotation_properties', editing: pick.id })
    else if (pick.type === 'view' || pick.type === 'entity')
      s.openPanel({ type: 'view_properties', editing: pick.view })
    else if (pick.view !== null) s.openPanel({ type: 'view_properties', editing: pick.view })
  }

  // Keys: Escape cancels a placement, Delete removes the selection.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement
      if (['INPUT', 'TEXTAREA', 'SELECT'].includes(t.tagName)) return
      const ds = useDrawing.getState()
      if (e.key === 'Escape') {
        if (ds.target && !useStore.getState().panel) ds.setTarget(null)
        ds.select(null)
        ds.setOverlay(EMPTY_OVERLAY)
        return
      }
      if ((e.key === 'Delete' || e.key === 'Backspace') && !useStore.getState().panel) {
        const views = ds.selection
          .filter((p) => p.type === 'view')
          .map((p) => (p as { view: number }).view)
        const notes = ds.selection
          .filter((p) => p.type === 'annotation')
          .map((p) => (p as { id: number }).id)
        void deleteSelected(views, notes)
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  const errorViews = render?.views.filter((v) => v.error) ?? []
  const stale = render?.views.some((v) => v.stale) ?? false

  return (
    <div className="sheet-canvas" ref={wrap}>
      <canvas
        ref={canvas}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={() => void onPointerUp()}
        onPointerLeave={() => setHover(null)}
        onWheel={onWheel}
        onDoubleClick={onDoubleClick}
        onContextMenu={(e) => e.preventDefault()}
        style={{ cursor: target ? 'crosshair' : hover ? 'pointer' : 'default' }}
      />
      {target && <div className="sheet-prompt">{target.prompt}</div>}
      {(errorViews.length > 0 || stale || (render?.dangling?.length ?? 0) > 0) && (
        <div className="sheet-notices">
          {stale && (
            <button onClick={() => void useStore.getState().run('drawing.update')}>
              A model changed since this drawing was saved — mark views up to date
            </button>
          )}
          {errorViews.map((v) => (
            <div key={v.id} className="sheet-error">
              {sheet?.views.find((x) => x.id === v.id)?.name ?? `View ${v.id}`}: {v.error}
            </div>
          ))}
          {(render?.dangling ?? []).map((d) => (
            <div key={d.annotation} className="sheet-error">
              Annotation {d.annotation}: {d.reason}
            </div>
          ))}
        </div>
      )}
      {cursor && (
        <div className="sheet-cursor">
          {cursor[0].toFixed(1)}, {cursor[1].toFixed(1)} mm
        </div>
      )}
    </div>
  )
}

/** Delete views (with their derived views and annotations) and annotations. */
export async function deleteSelected(views: number[], notes: number[]): Promise<void> {
  const s = useStore.getState()
  if (views.length) await s.run('drawing.view.delete', { ids: views })
  // Annotations of deleted views went with them.
  const left = notes.filter((id) =>
    useStore
      .getState()
      .drawing?.drawing.sheets.some((x) => (x.annotations ?? []).some((a) => a.id === id)),
  )
  if (left.length) await s.run('drawing.annotation.delete', { ids: left })
  useDrawing.getState().select(null)
}

export type { SheetRender }

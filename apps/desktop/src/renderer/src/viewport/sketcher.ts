import * as THREE from 'three'
import { LineGeometry } from 'three/examples/jsm/lines/LineGeometry.js'
import { Line2 } from 'three/examples/jsm/lines/Line2.js'
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js'
import type { SketchConstraint, SketchCurve, SketchView, Units, Vec2 } from '../protocol'
import type { SketchTool } from '../store'
import { formatAngle, formatLength } from '../units'
import { type Engine, planeMatrix } from './engine'

const C = {
  under: 0x1d4ed8,
  fully: 0x111827,
  over: 0xdc2626,
  external: 0xa21caf,
  selected: 0x0ea5e9,
  hover: 0xf59e0b,
  rubber: 0xf59e0b,
}

type Snap =
  | { type: 'point'; id: number; at: Vec2 }
  | { type: 'mid'; line: number; at: Vec2 }
  | { type: 'curve'; id: number; at: Vec2 }
  | { type: 'free'; at: Vec2; inferred?: 'h' | 'v' }

type Hit = { type: 'point'; id: number } | { type: 'curve'; id: number } | null

/** A label drawn over the viewport for a dimension or relation. */
export interface Label {
  id: number
  x: number
  y: number
  text: string
  kind: 'dim' | 'rel'
  status: SketchConstraint['status']
  driving: boolean
  selected: boolean
}

export interface SketcherHost {
  edit(op: Record<string, unknown>): Promise<SketchView | null>
  drag(point: number, to: Vec2): void
  selection(): number[]
  select(ids: number[]): void
  status(text: string): void
  /** Ask for a dimension value at a screen position; resolves to mm/rad or null. */
  askValue(x: number, y: number, initial: number, angle: boolean): Promise<number | null>
  /** Convert a picked model edge, or every loop of a picked face (Convert Entities). */
  convertAt(clientX: number, clientY: number): void
  filletRadius(): number
  polygonSides(): number
}

const sub = (a: Vec2, b: Vec2): Vec2 => [a[0] - b[0], a[1] - b[1]]
const len = (a: Vec2) => Math.hypot(a[0], a[1])
const dist = (a: Vec2, b: Vec2) => len(sub(a, b))

function closestOnPolyline(samples: Vec2[], p: Vec2): { at: Vec2; d: number } {
  let best: Vec2 = samples[0] ?? [0, 0]
  let bd = Infinity
  for (let i = 0; i + 1 < samples.length; i++) {
    const a = samples[i]!
    const b = samples[i + 1]!
    const ab = sub(b, a)
    const l2 = ab[0] * ab[0] + ab[1] * ab[1]
    const t =
      l2 > 0 ? Math.max(0, Math.min(1, ((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / l2)) : 0
    const q: Vec2 = [a[0] + ab[0] * t, a[1] + ab[1] * t]
    const d = dist(q, p)
    if (d < bd) {
      bd = d
      best = q
    }
  }
  return { at: best, d: bd }
}

export class Sketcher {
  private group = new THREE.Group()
  private rubber = new THREE.Group()
  private view: SketchView | null = null
  private units: Units | null = null
  private tool: SketchTool = 'select'
  private clicks: Snap[] = []
  /** Chained line start (point id) for the line tool. */
  private chainStart: number | null = null
  private mouse: Vec2 = [0, 0]
  private hover: Hit = null
  private dragging: { point: number; started: boolean } | null = null
  private boxStart: { x: number; y: number; at: Vec2 } | null = null
  private down: { x: number; y: number } | null = null
  private dimFirst: Hit = null
  private arcSweep = 0
  private lastArcAngle = 0

  constructor(
    private readonly engine: Engine,
    private readonly host: SketcherHost,
  ) {
    this.group.matrixAutoUpdate = false
    this.rubber.matrixAutoUpdate = false
    engine.scene.add(this.group, this.rubber)
  }

  dispose(): void {
    this.engine.scene.remove(this.group, this.rubber)
  }

  get active(): boolean {
    return !!this.view
  }

  setView(view: SketchView | null, units: Units): void {
    const planeChanged = JSON.stringify(view?.plane) !== JSON.stringify(this.view?.plane)
    this.view = view
    this.units = units
    if (view && planeChanged) {
      const m = planeMatrix(view.plane)
      this.group.matrix.copy(m)
      this.rubber.matrix.copy(m)
    }
    this.redraw()
  }

  setTool(tool: SketchTool): void {
    this.tool = tool
    this.cancel()
  }

  /** Abandon the operation in progress. */
  cancel(): void {
    this.clicks = []
    this.chainStart = null
    this.dimFirst = null
    this.boxStart = null
    this.rubber.clear()
    this.engine.requestRender()
  }

  // ---- Coordinates ----

  /** Mouse position in sketch coordinates. */
  local(clientX: number, clientY: number): Vec2 | null {
    if (!this.view) return null
    const ray = this.engine.ray(clientX, clientY)
    const m = planeMatrix(this.view.plane)
    const n = new THREE.Vector3(...this.view.plane.normal).normalize()
    const o = new THREE.Vector3(...this.view.plane.origin)
    const plane = new THREE.Plane().setFromNormalAndCoplanarPoint(n, o)
    const hit = ray.intersectPlane(plane, new THREE.Vector3())
    if (!hit) return null
    const l = hit.applyMatrix4(m.clone().invert())
    return [l.x, l.y]
  }

  world(p: Vec2): THREE.Vector3 {
    return new THREE.Vector3(p[0], p[1], 0).applyMatrix4(this.group.matrix)
  }

  private screen(p: Vec2): { x: number; y: number } {
    return this.engine.toScreen(this.world(p))
  }

  private get px(): number {
    return this.engine.pixel
  }

  // ---- Lookup ----

  private point(id: number): Vec2 | null {
    const p = this.view?.points.find((q) => q.id === id)
    return p ? [p.x, p.y] : null
  }

  private curve(id: number): SketchCurve | undefined {
    return this.view?.curves.find((c) => c.id === id)
  }

  private hitTest(p: Vec2): Hit {
    if (!this.view) return null
    let best: Hit = null
    let bd = 9 * this.px
    for (const q of this.view.points) {
      const d = dist([q.x, q.y], p)
      if (d < bd) {
        bd = d
        best = { type: 'point', id: q.id }
      }
    }
    if (best) return best
    bd = 6 * this.px
    for (const c of this.view.curves) {
      const { d } = closestOnPolyline(c.samples, p)
      if (d < bd) {
        bd = d
        best = { type: 'curve', id: c.id }
      }
    }
    return best
  }

  private snap(p: Vec2, from?: Vec2): Snap {
    if (!this.view) return { type: 'free', at: p }
    const r = 10 * this.px
    let best: Snap | null = null
    let bd = r
    for (const q of this.view.points) {
      const d = dist([q.x, q.y], p)
      if (d < bd) {
        bd = d
        best = { type: 'point', id: q.id, at: [q.x, q.y] }
      }
    }
    if (best) return best
    for (const c of this.view.curves) {
      if (c.type !== 'line') continue
      const a = this.point(c.points[0]!)
      const b = this.point(c.points[1]!)
      if (!a || !b) continue
      const m: Vec2 = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2]
      const d = dist(m, p)
      if (d < bd * 0.8) {
        bd = d
        best = { type: 'mid', line: c.id, at: m }
      }
    }
    if (best) return best
    for (const c of this.view.curves) {
      const { at, d } = closestOnPolyline(c.samples, p)
      if (d < 6 * this.px) return { type: 'curve', id: c.id, at }
    }
    if (from) {
      const v = sub(p, from)
      const ang = Math.abs(Math.atan2(v[1], v[0]))
      const tol = (3 * Math.PI) / 180
      if (len(v) > 4 * this.px) {
        if (ang < tol || Math.PI - ang < tol)
          return { type: 'free', at: [p[0], from[1]], inferred: 'h' }
        if (Math.abs(ang - Math.PI / 2) < tol)
          return { type: 'free', at: [from[0], p[1]], inferred: 'v' }
      }
    }
    return { type: 'free', at: p }
  }

  private pointArg(s: Snap): Record<string, unknown> {
    return s.type === 'point' ? { id: s.id } : { at: s.at }
  }

  /** Relations implied by where a new point landed. */
  private async attach(s: Snap, newPoint: number | undefined): Promise<void> {
    if (newPoint === undefined) return
    if (s.type === 'curve')
      await this.host.edit({
        op: 'constraint',
        constraint: { type: 'point_on_curve', point: newPoint, curve: s.id },
      })
    if (s.type === 'mid')
      await this.host.edit({
        op: 'constraint',
        constraint: { type: 'midpoint', point: newPoint, line: s.line },
      })
  }

  // ---- Input ----

  onPointerDown(e: PointerEvent): boolean {
    if (!this.view || e.button !== 0) return false
    const p = this.local(e.clientX, e.clientY)
    if (!p) return false
    this.down = { x: e.clientX, y: e.clientY }
    if (this.tool === 'select') {
      const hit = this.hitTest(p)
      if (hit?.type === 'point') {
        const pt = this.view.points.find((q) => q.id === hit.id)
        if (pt && !pt.external && hit.id !== 0) this.dragging = { point: hit.id, started: false }
      } else if (!hit) {
        this.boxStart = { x: e.clientX, y: e.clientY, at: p }
      }
      return true
    }
    return true
  }

  onPointerMove(e: PointerEvent): void {
    if (!this.view) return
    const p = this.local(e.clientX, e.clientY)
    if (!p) return
    this.mouse = p
    const u = this.units!
    this.host.status(`${formatLength(p[0], u)}, ${formatLength(p[1], u)}`)
    if (this.dragging) {
      if (!this.dragging.started) {
        const moved = this.down
          ? Math.hypot(e.clientX - this.down.x, e.clientY - this.down.y) > 4
          : false
        if (!moved) return
        // A no-op edit first, so the drag can be undone in one step.
        this.dragging.started = true
        void this.host.edit({ op: 'move', ids: [], delta: [0, 0] })
      }
      this.host.drag(this.dragging.point, p)
      return
    }
    if (this.boxStart) {
      this.drawBox(this.boxStart.at, p)
      return
    }
    const prevHover = JSON.stringify(this.hover)
    this.hover = this.tool === 'convert' ? null : this.hitTest(p)
    if (JSON.stringify(this.hover) !== prevHover) this.redraw()
    this.drawRubber()
  }

  async onPointerUp(e: PointerEvent): Promise<void> {
    if (!this.view || e.button !== 0) return
    const p = this.local(e.clientX, e.clientY)
    const moved = this.down
      ? Math.hypot(e.clientX - this.down.x, e.clientY - this.down.y) > 4
      : false
    this.down = null
    if (this.dragging) {
      const started = this.dragging.started
      this.dragging = null
      if (started) return
    }
    if (this.boxStart) {
      const start = this.boxStart
      this.boxStart = null
      this.rubber.clear()
      if (moved && p) {
        this.boxSelect(start.at, p, e.shiftKey || e.metaKey || e.ctrlKey)
        return
      }
    }
    if (!p || moved) return
    await this.click(p, e)
  }

  async onDoubleClick(): Promise<void> {
    if (this.tool === 'line' || this.tool === 'centerline') this.cancel()
    if (this.tool === 'spline') await this.finishSpline()
  }

  async onKey(e: KeyboardEvent): Promise<boolean> {
    if (!this.view) return false
    if (e.key === 'Escape') {
      if (this.clicks.length || this.chainStart !== null || this.dimFirst) {
        this.cancel()
        return true
      }
      return false
    }
    if (e.key === 'Enter' && this.tool === 'spline') {
      await this.finishSpline()
      return true
    }
    if ((e.key === 'Delete' || e.key === 'Backspace') && this.host.selection().length) {
      await this.host.edit({ op: 'delete', ids: this.host.selection() })
      this.host.select([])
      return true
    }
    return false
  }

  private boxSelect(a: Vec2, b: Vec2, additive: boolean): void {
    if (!this.view) return
    const lo: Vec2 = [Math.min(a[0], b[0]), Math.min(a[1], b[1])]
    const hi: Vec2 = [Math.max(a[0], b[0]), Math.max(a[1], b[1])]
    const inside = (q: Vec2) => q[0] >= lo[0] && q[0] <= hi[0] && q[1] >= lo[1] && q[1] <= hi[1]
    const ids = new Set(additive ? this.host.selection() : [])
    for (const c of this.view.curves) if (c.samples.every(inside)) ids.add(c.id)
    for (const q of this.view.points) if (q.standalone && inside([q.x, q.y])) ids.add(q.id)
    this.host.select([...ids])
  }

  private async click(p: Vec2, e: PointerEvent): Promise<void> {
    const prev =
      this.clicks[this.clicks.length - 1]?.at ??
      (this.chainStart !== null ? (this.point(this.chainStart) ?? undefined) : undefined)
    const s = this.snap(p, prev)
    switch (this.tool) {
      case 'select': {
        const hit = this.hitTest(p)
        const additive = e.shiftKey || e.metaKey || e.ctrlKey
        if (!hit) {
          if (!additive) this.host.select([])
          return
        }
        const cur = this.host.selection()
        if (additive)
          this.host.select(
            cur.includes(hit.id) ? cur.filter((x) => x !== hit.id) : [...cur, hit.id],
          )
        else this.host.select([hit.id])
        return
      }
      case 'line':
      case 'centerline': {
        if (this.chainStart === null && !this.clicks.length) {
          this.clicks = [s]
          return
        }
        const a =
          this.chainStart !== null ? { id: this.chainStart } : this.pointArg(this.clicks[0]!)
        const startSnap = this.clicks[0]
        const r = await this.host.edit({ op: 'add_line', a, b: this.pointArg(s) })
        if (!r?.created) return
        const [line, pa, pb] = r.created
        if (startSnap && this.chainStart === null) await this.attach(startSnap, pa)
        await this.attach(s, pb)
        if (s.type === 'free' && s.inferred) {
          await this.host.edit({
            op: 'constraint',
            constraint: { type: s.inferred === 'h' ? 'horizontal' : 'vertical', line },
          })
        }
        if (this.tool === 'centerline') {
          await this.host.edit({ op: 'construction', ids: [line], construction: true })
          this.cancel()
          return
        }
        // Closing the loop or landing on an existing point ends the chain.
        if (s.type === 'point') {
          this.cancel()
          return
        }
        this.clicks = []
        this.chainStart = pb ?? null
        return
      }
      case 'rectangle':
      case 'center_rectangle': {
        this.clicks.push(s)
        if (this.clicks.length < 2) return
        const [a, b] = this.clicks
        this.cancel()
        await this.host.edit({
          op: 'rectangle',
          a: a!.at,
          b: b!.at,
          center: this.tool === 'center_rectangle',
        })
        return
      }
      case 'circle': {
        this.clicks.push(s)
        if (this.clicks.length < 2) return
        const [c, r] = this.clicks
        this.cancel()
        const radius = dist(c!.at, r!.at)
        if (radius < 1e-9) return
        const out = await this.host.edit({ op: 'add_circle', center: this.pointArg(c!), radius })
        if (out?.created && r!.type === 'point') {
          await this.host.edit({
            op: 'constraint',
            constraint: { type: 'point_on_curve', point: r!.id, curve: out.created[0] },
          })
        }
        return
      }
      case 'center_arc': {
        this.clicks.push(s)
        if (this.clicks.length === 2) {
          const c = this.clicks[0]!.at
          this.lastArcAngle = Math.atan2(s.at[1] - c[1], s.at[0] - c[0])
          this.arcSweep = 0
        }
        if (this.clicks.length < 3) return
        const [c, st, en] = this.clicks
        this.cancel()
        await this.host.edit({
          op: 'add_center_arc',
          center: this.pointArg(c!),
          start: this.pointArg(st!),
          end: en!.at,
          clockwise: this.arcSweep < 0,
        })
        return
      }
      case 'arc3': {
        this.clicks.push(s)
        if (this.clicks.length < 3) return
        const [a, b, via] = this.clicks
        this.cancel()
        await this.host.edit({
          op: 'add_arc3',
          start: this.pointArg(a!),
          end: this.pointArg(b!),
          via: via!.at,
        })
        return
      }
      case 'tangent_arc': {
        if (!this.clicks.length) {
          if (s.type !== 'point' || !this.tangentAt(s.id)) {
            this.host.status('Start a tangent arc at the end of a line or arc')
            return
          }
          this.clicks = [s]
          return
        }
        const start = this.clicks[0] as Extract<Snap, { type: 'point' }>
        const t = this.tangentAt(start.id)!
        this.cancel()
        const r = await this.host.edit({
          op: 'add_tangent_arc',
          start: start.id,
          tangent: t,
          end: this.pointArg(s),
        })
        if (r?.created) {
          const owner = this.ownerCurve(start.id)
          if (owner !== undefined)
            await this.host.edit({
              op: 'constraint',
              constraint: { type: 'tangent', a: owner, b: r.created[0] },
            })
        }
        return
      }
      case 'slot': {
        this.clicks.push(s)
        if (this.clicks.length < 3) return
        const [a, b, w] = this.clicks
        this.cancel()
        const axis = sub(b!.at, a!.at)
        const l = len(axis)
        if (l < 1e-9) return
        const width =
          (2 * Math.abs(axis[0] * (w!.at[1] - a!.at[1]) - axis[1] * (w!.at[0] - a!.at[0]))) / l
        await this.host.edit({ op: 'slot', a: a!.at, b: b!.at, width })
        return
      }
      case 'polygon': {
        this.clicks.push(s)
        if (this.clicks.length < 2) return
        const [c, vtx] = this.clicks
        this.cancel()
        await this.host.edit({
          op: 'polygon',
          center: c!.at,
          vertex: vtx!.at,
          sides: this.host.polygonSides(),
        })
        return
      }
      case 'spline':
        this.clicks.push(s)
        return
      case 'point': {
        const r = await this.host.edit({ op: 'add_point', at: s.at })
        await this.attach(s, r?.created?.[0])
        return
      }
      case 'trim': {
        const hit = this.hitTest(p)
        if (hit?.type === 'curve') await this.host.edit({ op: 'trim', entity: hit.id, pick: p })
        return
      }
      case 'extend': {
        const hit = this.hitTest(p)
        if (hit?.type === 'curve') await this.host.edit({ op: 'extend', entity: hit.id, pick: p })
        return
      }
      case 'fillet': {
        const hit = this.hitTest(p)
        if (hit?.type === 'point')
          await this.host.edit({ op: 'fillet', point: hit.id, radius: this.host.filletRadius() })
        else this.host.status('Click a corner where two curves meet')
        return
      }
      case 'convert':
        this.host.convertAt(e.clientX, e.clientY)
        return
      case 'dimension':
        await this.dimensionClick(p, e)
        return
    }
  }

  private ownerCurve(point: number): number | undefined {
    return this.view?.curves.find(
      (c) =>
        (c.type === 'line' || c.type === 'arc') &&
        (c.points[0] === point ||
          c.points[c.points.length - 1] === point ||
          (c.type === 'arc' && c.points[2] === point)),
    )?.id
  }

  /** Direction leaving a curve at one of its endpoints. */
  private tangentAt(point: number): Vec2 | null {
    const c = this.view?.curves.find(
      (x) =>
        (x.type === 'line' && x.points.includes(point)) ||
        (x.type === 'arc' && (x.points[1] === point || x.points[2] === point)),
    )
    if (!c) return null
    const p = this.point(point)!
    if (c.type === 'line') {
      const other = this.point(c.points[0] === point ? c.points[1]! : c.points[0]!)!
      return sub(p, other)
    }
    const center = c.center!
    const r = sub(p, center)
    // Arcs run counter-clockwise from start (points[1]) to end (points[2]).
    const ccw: Vec2 = [-r[1], r[0]]
    return c.points[2] === point ? ccw : [-ccw[0], -ccw[1]]
  }

  private async finishSpline(): Promise<void> {
    if (this.clicks.length < 2) {
      this.cancel()
      return
    }
    const pts = this.clicks.map((c) => this.pointArg(c))
    const closed =
      this.clicks.length > 2 &&
      this.clicks[0]!.type === 'point' &&
      JSON.stringify(this.clicks[0]) === JSON.stringify(this.clicks[this.clicks.length - 1])
    this.cancel()
    await this.host.edit({ op: 'add_spline', points: closed ? pts.slice(0, -1) : pts, closed })
  }

  private async dimensionClick(p: Vec2, e: PointerEvent): Promise<void> {
    const hit = this.hitTest(p)
    if (!this.dimFirst) {
      if (!hit) return
      this.dimFirst = hit
      this.redraw()
      return
    }
    const first = this.dimFirst
    const second = hit && hit.id !== first.id ? hit : null
    this.dimFirst = null
    const spec = this.dimensionFor(first, second)
    if (!spec) {
      this.host.status('These items can’t be dimensioned together')
      this.redraw()
      return
    }
    const value = await this.host.askValue(e.clientX, e.clientY, spec.measured, spec.angle)
    this.redraw()
    if (value === null) return
    // Signed dimensions keep the geometry's current sign in the core.
    await this.host.edit({
      op: 'dimension',
      constraint: { ...spec.constraint, value: 0 },
      value: Math.abs(value),
    })
  }

  /** Which dimension two picks make, and its current value. */
  private dimensionFor(
    a: Hit,
    b: Hit,
  ): { constraint: Record<string, unknown>; measured: number; angle: boolean } | null {
    if (!a) return null
    const ca = a.type === 'curve' ? this.curve(a.id) : undefined
    const cb = b?.type === 'curve' ? this.curve(b.id) : undefined
    const locus = (h: Hit, c?: SketchCurve): Vec2 | null =>
      h?.type === 'point' ? this.point(h.id) : (c?.center ?? null)
    const lineEnds = (c: SketchCurve): [Vec2, Vec2] => [
      this.point(c.points[0]!)!,
      this.point(c.points[1]!)!,
    ]
    if (!b) {
      if (ca?.type === 'line') {
        const [p, q] = lineEnds(ca)
        return { constraint: { type: 'length', line: ca.id }, measured: dist(p, q), angle: false }
      }
      if (ca?.type === 'circle')
        return {
          constraint: { type: 'diameter', curve: ca.id },
          measured: 2 * ca.radius!,
          angle: false,
        }
      if (ca?.type === 'arc')
        return { constraint: { type: 'radius', curve: ca.id }, measured: ca.radius!, angle: false }
      return null
    }
    if (ca?.type === 'line' && cb?.type === 'line') {
      const [p, q] = lineEnds(ca)
      const [r, s] = lineEnds(cb)
      const d1 = sub(q, p)
      const d2 = sub(s, r)
      const cross = d1[0] * d2[1] - d1[1] * d2[0]
      const dot = d1[0] * d2[0] + d1[1] * d2[1]
      const ang = Math.atan2(cross, dot)
      if (Math.abs(Math.sin(ang)) < 0.02) {
        const n = len(d1)
        return {
          constraint: { type: 'distance', a: ca.id, b: cb.id },
          measured: Math.abs(d1[0] * (r[1] - p[1]) - d1[1] * (r[0] - p[0])) / n,
          angle: false,
        }
      }
      return { constraint: { type: 'angle', a: ca.id, b: cb.id }, measured: ang, angle: true }
    }
    const lineAndOther =
      ca?.type === 'line'
        ? ([ca, b, cb] as const)
        : cb?.type === 'line'
          ? ([cb, a, ca] as const)
          : null
    if (lineAndOther) {
      const [line, other, oc] = lineAndOther
      const pt = locus(other, oc)
      if (!pt) return null
      const [p, q] = lineEnds(line)
      const d = sub(q, p)
      const measured = Math.abs(d[0] * (pt[1] - p[1]) - d[1] * (pt[0] - p[0])) / len(d)
      return { constraint: { type: 'distance', a: line.id, b: other!.id }, measured, angle: false }
    }
    const pa = locus(a, ca)
    const pb = locus(b, cb)
    if (!pa || !pb) return null
    return {
      constraint: { type: 'distance', a: a.id, b: b.id },
      measured: dist(pa, pb),
      angle: false,
    }
  }

  // ---- Drawing ----

  private fat(points: Vec2[], color: number, width: number, dashed = false): Line2 {
    const pos: number[] = []
    for (const p of points) pos.push(p[0], p[1], 0)
    const g = new LineGeometry().setPositions(pos)
    const m = new LineMaterial({
      color,
      linewidth: width,
      depthTest: false,
      dashed,
      dashSize: 6 * this.px,
      gapSize: 4 * this.px,
      worldUnits: false,
    })
    m.resolution.set(this.engine.canvas.clientWidth, this.engine.canvas.clientHeight)
    const l = new Line2(g, m)
    if (dashed) l.computeLineDistances()
    l.renderOrder = 20
    return l
  }

  private colorOf(status: string, external: boolean, id: number): number {
    if (this.host.selection().includes(id) || (this.dimFirst && this.dimFirst.id === id))
      return C.selected
    if (this.hover?.id === id) return C.hover
    if (external) return C.external
    return status === 'fully' ? C.fully : status === 'over' ? C.over : C.under
  }

  redraw(): void {
    this.group.traverse((o) => (o as THREE.Mesh).geometry?.dispose())
    this.group.clear()
    const v = this.view
    if (!v) {
      this.rubber.clear()
      this.engine.requestRender()
      return
    }
    for (const c of v.curves) {
      this.group.add(
        this.fat(c.samples, this.colorOf(c.status, c.external, c.id), 2, c.construction),
      )
    }
    const pts: THREE.Vector3[] = []
    const cols: number[] = []
    for (const p of v.points) {
      pts.push(new THREE.Vector3(p.x, p.y, 0))
      const col = new THREE.Color(p.id === 0 ? 0xdc2626 : this.colorOf(p.status, p.external, p.id))
      cols.push(col.r, col.g, col.b)
    }
    const g = new THREE.BufferGeometry().setFromPoints(pts)
    g.setAttribute('color', new THREE.Float32BufferAttribute(cols, 3))
    const points = new THREE.Points(
      g,
      new THREE.PointsMaterial({
        size: 6,
        sizeAttenuation: false,
        vertexColors: true,
        depthTest: false,
      }),
    )
    points.renderOrder = 21
    this.group.add(points)
    // Sketch axes through the origin.
    const ext = Math.max(20, this.engine.size * 0.15)
    for (const [a, b] of [
      [
        [-ext, 0],
        [ext, 0],
      ],
      [
        [0, -ext],
        [0, ext],
      ],
    ] as [Vec2, Vec2][]) {
      const l = this.fat([a, b], 0x9ca3af, 1, true)
      l.renderOrder = 19
      this.group.add(l)
    }
    this.drawRubber()
    this.engine.requestRender()
  }

  private drawBox(a: Vec2, b: Vec2): void {
    this.rubber.clear()
    this.rubber.add(this.fat([a, [b[0], a[1]], b, [a[0], b[1]], a], C.selected, 1, true))
    this.engine.requestRender()
  }

  private drawRubber(): void {
    this.rubber.traverse((o) => (o as THREE.Mesh).geometry?.dispose())
    this.rubber.clear()
    const v = this.view
    if (!v) return
    const m = this.mouse
    const prevAt =
      this.clicks[this.clicks.length - 1]?.at ??
      (this.chainStart !== null ? (this.point(this.chainStart) ?? undefined) : undefined)
    const s =
      this.tool === 'select' || this.tool === 'trim'
        ? ({ type: 'free', at: m } as Snap)
        : this.snap(m, prevAt)
    const at = s.at
    const add = (pts: Vec2[], dashed = true) =>
      this.rubber.add(this.fat(pts, C.rubber, 1.5, dashed))
    const circle = (c: Vec2, r: number, a0 = 0, sweep = Math.PI * 2): Vec2[] => {
      const n = 64
      return Array.from(
        { length: n + 1 },
        (_, i) =>
          [
            c[0] + r * Math.cos(a0 + (sweep * i) / n),
            c[1] + r * Math.sin(a0 + (sweep * i) / n),
          ] as Vec2,
      )
    }
    const k = this.clicks
    switch (this.tool) {
      case 'line':
      case 'centerline':
        if (prevAt) add([prevAt, at], this.tool === 'centerline')
        break
      case 'rectangle':
        if (k[0]) {
          const a = k[0].at
          add([a, [at[0], a[1]], at, [a[0], at[1]], a], false)
        }
        break
      case 'center_rectangle':
        if (k[0]) {
          const c = k[0].at
          const h = [Math.abs(at[0] - c[0]), Math.abs(at[1] - c[1])]
          add(
            [
              [c[0] - h[0]!, c[1] - h[1]!],
              [c[0] + h[0]!, c[1] - h[1]!],
              [c[0] + h[0]!, c[1] + h[1]!],
              [c[0] - h[0]!, c[1] + h[1]!],
              [c[0] - h[0]!, c[1] - h[1]!],
            ],
            false,
          )
        }
        break
      case 'circle':
        if (k[0]) add(circle(k[0].at, dist(k[0].at, at)), false)
        break
      case 'center_arc':
        if (k.length === 1) add([k[0]!.at, at])
        if (k.length === 2) {
          const c = k[0]!.at
          const st = k[1]!.at
          const a = Math.atan2(at[1] - c[1], at[0] - c[0])
          let d = a - this.lastArcAngle
          if (d > Math.PI) d -= 2 * Math.PI
          if (d < -Math.PI) d += 2 * Math.PI
          this.arcSweep += d
          this.lastArcAngle = a
          const a0 = Math.atan2(st[1] - c[1], st[0] - c[0])
          add(circle(c, dist(c, st), a0, this.arcSweep), false)
        }
        break
      case 'arc3':
        if (k.length === 1) add([k[0]!.at, at])
        if (k.length === 2) add([k[0]!.at, at, k[1]!.at])
        break
      case 'tangent_arc':
        if (k[0]) add([k[0].at, at])
        break
      case 'slot':
        if (k.length >= 1) add([k[0]!.at, k.length === 2 ? k[1]!.at : at])
        if (k.length === 2) add([k[1]!.at, at])
        break
      case 'polygon':
        if (k[0]) {
          const c = k[0].at
          const r = dist(c, at)
          const a0 = Math.atan2(at[1] - c[1], at[0] - c[0])
          const n = this.host.polygonSides()
          add(
            Array.from(
              { length: n + 1 },
              (_, i) =>
                [
                  c[0] + r * Math.cos(a0 + (2 * Math.PI * i) / n),
                  c[1] + r * Math.sin(a0 + (2 * Math.PI * i) / n),
                ] as Vec2,
            ),
            false,
          )
        }
        break
      case 'spline':
        if (k.length) add([...k.map((c) => c.at), at])
        break
    }
    // Snap marker.
    if (
      this.tool !== 'select' &&
      this.tool !== 'trim' &&
      this.tool !== 'convert' &&
      s.type !== 'free'
    ) {
      const r = 5 * this.px
      add(circle(at, r), false)
    }
    if (s.type === 'free' && s.inferred && prevAt) {
      this.rubber.add(this.fat([prevAt, at], 0x6b7280, 1, true))
    }
    this.engine.requestRender()
  }

  // ---- Labels ----

  labels(): Label[] {
    const v = this.view
    const u = this.units
    if (!v || !u) return []
    const sel = this.host.selection()
    const out: Label[] = []
    const px = this.px
    const relCount = new Map<number, number>()
    for (const c of v.constraints) {
      const anchor = this.anchor(c, px)
      if (!anchor) continue
      const isDim = c.value !== undefined
      let text: string
      if (isDim) {
        const val = c.value as number
        text =
          c.type === 'angle'
            ? formatAngle(Math.abs(val), u, true)
            : c.type === 'diameter'
              ? `⌀${formatLength(val, u, false)}`
              : c.type === 'radius'
                ? `R${formatLength(val, u, false)}`
                : formatLength(Math.abs(val), u, false)
        if (!c.driving) text = `(${text})`
      } else {
        text = GLYPH[c.type] ?? c.type
        // Stack several relations on the same entity.
        const key = c.refs[0] ?? 0
        const n = relCount.get(key) ?? 0
        relCount.set(key, n + 1)
        anchor[1] -= n * 14 * px
      }
      const s = this.screen(anchor)
      out.push({
        id: c.id,
        x: s.x,
        y: s.y,
        text,
        kind: isDim ? 'dim' : 'rel',
        status: c.status,
        driving: c.driving,
        selected: sel.includes(c.id),
      })
    }
    return out
  }

  private anchor(c: SketchConstraint, px: number): Vec2 | null {
    const curve = (id: number) => this.curve(id)
    const loc = (id: number): Vec2 | null => this.point(id) ?? curve(id)?.center ?? null
    const mid = (id: number): Vec2 | null => {
      const cv = curve(id)
      if (!cv) return this.point(id)
      const s = cv.samples
      return s[Math.floor(s.length / 2)] ?? null
    }
    const off = (p: Vec2 | null, dx: number, dy: number): Vec2 | null =>
      p ? [p[0] + dx * px, p[1] + dy * px] : null
    const r = c.refs
    switch (c.type) {
      case 'length': {
        const cv = curve(r[0]!)
        if (!cv) return null
        const a = this.point(cv.points[0]!)!
        const b = this.point(cv.points[1]!)!
        const d = sub(b, a)
        const n = len(d) || 1
        return [(a[0] + b[0]) / 2 - (d[1] / n) * 16 * px, (a[1] + b[1]) / 2 + (d[0] / n) * 16 * px]
      }
      case 'radius':
      case 'diameter': {
        const cv = curve(r[0]!)
        if (!cv?.center || !cv.radius) return null
        const s =
          cv.type === 'arc'
            ? (cv.samples[Math.floor(cv.samples.length / 2)] ?? cv.center)
            : [cv.center[0] + cv.radius * 0.707, cv.center[1] + cv.radius * 0.707]
        return off(s as Vec2, 14, 14)
      }
      case 'distance':
      case 'horizontal_distance':
      case 'vertical_distance': {
        const a = loc(r[0]!) ?? mid(r[0]!)
        const b = loc(r[1]!) ?? mid(r[1]!)
        if (!a || !b) return null
        return off([(a[0] + b[0]) / 2, (a[1] + b[1]) / 2], 0, 12)
      }
      case 'angle': {
        const a = mid(r[0]!)
        const b = mid(r[1]!)
        if (!a || !b) return null
        return [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2]
      }
      case 'coincident':
        return off(loc(r[0]!), 8, -8)
      default:
        return off(mid(r[0]!), -12, -12)
    }
  }
}

const GLYPH: Record<string, string> = {
  coincident: '●',
  point_on_curve: '◦',
  midpoint: 'M',
  horizontal: 'H',
  vertical: 'V',
  horizontal_points: 'H',
  vertical_points: 'V',
  parallel: '∥',
  perpendicular: '⊥',
  collinear: '⫽',
  equal: '=',
  tangent: 'T',
  concentric: '◎',
  symmetric: 'S',
  fix: '⚓',
}

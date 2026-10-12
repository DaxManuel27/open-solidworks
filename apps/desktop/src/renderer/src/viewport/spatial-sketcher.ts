import * as THREE from 'three'
import type { Entity3 } from '../../../shared/tools.gen'
import type { Vec3 } from '../protocol'
import { useStore } from '../store'
import { formatLength } from '../units'
import type { Engine } from './engine'
import type { Label } from './sketcher'
import { type SpatialSketch, spatialSamples, useSpatial } from './spatial-state'

const vector = (p: Vec3) => new THREE.Vector3(...p)
const axes = [new THREE.Vector3(1, 0, 0), new THREE.Vector3(0, 1, 0), new THREE.Vector3(0, 0, 1)]
const colors = [0xd44747, 0x269747, 0x327dd1]

export class SpatialSketcher {
  private group = new THREE.Group()
  private unsubscribe: () => void
  private clicks: number[] = []
  private lastTool = 'select'
  private drag: { id: number; anchor: Vec3; to: Vec3; axis: number | null; moved: boolean } | null =
    null
  constructor(
    private engine: Engine,
    private changed: () => void,
  ) {
    engine.overlayGroup.add(this.group)
    this.unsubscribe = useSpatial.subscribe(() => this.draw())
  }
  get active() {
    return !!useSpatial.getState().sketch
  }
  dispose() {
    this.unsubscribe()
    this.clear()
    this.group.removeFromParent()
  }
  private clear() {
    this.group.traverse((o) => {
      const mesh = o as THREE.Mesh
      mesh.geometry?.dispose()
      const materials = mesh.material
        ? Array.isArray(mesh.material)
          ? mesh.material
          : [mesh.material]
        : []
      for (const m of materials) m.dispose()
    })
    this.group.clear()
  }
  private line(points: Vec3[], color: number) {
    const geometry = new THREE.BufferGeometry().setFromPoints(points.map(vector))
    const material = new THREE.LineBasicMaterial({ color, depthTest: false })
    const line = new THREE.Line(geometry, material)
    line.renderOrder = 20
    this.group.add(line)
  }
  draw() {
    this.clear()
    const s = useSpatial.getState(),
      sk = s.sketch
    if (s.tool !== this.lastTool || !sk) {
      this.clicks = []
      this.lastTool = s.tool
    }
    if (!sk) {
      this.engine.requestRender()
      this.changed()
      return
    }
    const display = this.drag?.moved
      ? {
          ...sk,
          entities: sk.entities.map((e): Entity3 =>
            e.id === this.drag?.id ? { ...e, kind: 'point', at: this.drag.to } : e,
          ),
        }
      : sk
    for (const e of display.entities) {
      const selected = s.selection.includes(e.id)
      const color = selected
        ? 0xef8c13
        : e.construction
          ? 0x9299a7
          : s.report?.entities[e.id] === 'fully'
            ? 0x242934
            : 0x2365c4
      if (e.kind === 'point') {
        const points = new THREE.Points(
          new THREE.BufferGeometry().setFromPoints([vector(e.at)]),
          new THREE.PointsMaterial({
            color,
            size: selected ? 10 : 7,
            sizeAttenuation: false,
            depthTest: false,
          }),
        )
        points.renderOrder = 21
        this.group.add(points)
        if (selected)
          for (let i = 0; i < 3; i++)
            this.line(
              [
                e.at,
                vector(e.at)
                  .addScaledVector(axes[i]!, this.engine.pixel * 38)
                  .toArray() as Vec3,
              ],
              colors[i]!,
            )
      } else this.line(spatialSamples(display, e), color)
    }
    // Outline makes the active depth/workplane visible without obscuring solids.
    const normal = s.plane === 'xy' ? 2 : s.plane === 'xz' ? 1 : 0
    const tangent = [0, 1, 2].filter((i) => i !== normal)
    const extent = Math.max(10, this.engine.pixel * 120)
    this.line(
      [
        [-1, -1],
        [1, -1],
        [1, 1],
        [-1, 1],
        [-1, -1],
      ].map(([a, b]) => {
        const p: Vec3 = [0, 0, 0]
        p[normal] = s.depth
        p[tangent[0]!] = a! * extent
        p[tangent[1]!] = b! * extent
        return p
      }),
      0xa8aeb9,
    )
    this.engine.requestRender()
    this.changed()
  }
  labels(): Label[] {
    const s = useSpatial.getState(),
      sk = s.sketch,
      u = useStore.getState().snapshot?.units
    if (!sk || !u) return []
    const labels: Label[] = []
    for (const c of sk.constraints)
      if (c.kind === 'distance') {
        const a = sk.entities.find((e) => e.id === c.a),
          b = sk.entities.find((e) => e.id === c.b)
        if (a?.kind !== 'point' || b?.kind !== 'point') continue
        const at = this.engine.toScreen(vector(a.at).lerp(vector(b.at), 0.5))
        labels.push({
          id: c.id,
          x: at.x,
          y: at.y - 14,
          text: formatLength(c.value, u),
          kind: 'dim',
          status: s.report?.conflicting.includes(c.id) ? 'conflicting' : 'ok',
          driving: true,
          selected: s.selection.includes(c.id),
        })
      }
    return labels
  }
  private screen(e: PointerEvent) {
    const r = this.engine.canvas.getBoundingClientRect()
    return { x: e.clientX - r.left, y: e.clientY - r.top }
  }
  private hit(e: PointerEvent, pointsOnly = false): number | null {
    const sk = useSpatial.getState().sketch
    if (!sk) return null
    const mouse = this.screen(e)
    let best: { id: number; d: number } | null = null
    for (const entity of sk.entities) {
      if (pointsOnly && entity.kind !== 'point') continue
      const samples = spatialSamples(sk, entity).map((p) => this.engine.toScreen(vector(p)))
      let distance = Infinity
      for (let i = 0; i < samples.length; i++) {
        const a = samples[i]!,
          b = samples[i + 1] ?? a
        const dx = b.x - a.x,
          dy = b.y - a.y,
          l = dx * dx + dy * dy
        const t = l
          ? Math.max(0, Math.min(1, ((mouse.x - a.x) * dx + (mouse.y - a.y) * dy) / l))
          : 0
        distance = Math.min(distance, Math.hypot(mouse.x - a.x - t * dx, mouse.y - a.y - t * dy))
      }
      // Points are preferentially selected over curves sharing their endpoint.
      if (entity.kind !== 'point') distance += 3
      if (distance < 9 && (!best || distance < best.d)) best = { id: entity.id, d: distance }
    }
    return best?.id ?? null
  }
  private position(e: PointerEvent, anchor?: Vec3, axis?: number | null): Vec3 | null {
    const s = useSpatial.getState(),
      ray = this.engine.ray(e.clientX, e.clientY)
    const q = new THREE.Vector3()
    if (anchor && axis !== null && axis !== undefined) {
      const origin = vector(anchor),
        direction = axes[axis]!
      // When the axis points into the camera there is no unique screen drag.
      if (Math.abs(ray.direction.dot(direction)) > 0.9999) return null
      ray.distanceSqToSegment(
        origin.clone().addScaledVector(direction, -1e7),
        origin.clone().addScaledVector(direction, 1e7),
        undefined,
        q,
      )
    } else {
      const normal = s.plane === 'xy' ? 2 : s.plane === 'xz' ? 1 : 0
      if (!ray.intersectPlane(new THREE.Plane(axes[normal]!, -(anchor?.[normal] ?? s.depth)), q))
        return null
    }
    const out = q.toArray() as Vec3
    const fixed = s.plane === 'xy' ? 2 : s.plane === 'xz' ? 1 : 0
    if (s.grid > 0)
      for (let i = 0; i < 3; i++)
        if (axis !== null && axis !== undefined ? i === axis : i !== fixed)
          out[i] = Math.round(out[i]! / s.grid) * s.grid
    return out
  }
  onDown(e: PointerEvent): boolean {
    if (!this.active || e.button !== 0 || e.altKey) return false
    const s = useSpatial.getState(),
      sk = s.sketch!
    if (s.tool === 'select') {
      let hit = this.hit(e),
        axis: number | null = s.lock === 'plane' ? null : ['x', 'y', 'z'].indexOf(s.lock)
      // Axis handles can be grabbed away from the point itself.
      const point = sk.entities.find((p) => p.kind === 'point' && s.selection.includes(p.id))
      if (point?.kind === 'point') {
        const mouse = this.screen(e)
        for (let i = 0; i < 3; i++) {
          const p = this.engine.toScreen(
            vector(point.at).addScaledVector(axes[i]!, this.engine.pixel * 38),
          )
          if (Math.hypot(mouse.x - p.x, mouse.y - p.y) < 10) {
            hit = point.id
            axis = i
            break
          }
        }
      }
      const entity = sk.entities.find((p) => p.id === hit)
      useSpatial.setState({
        selection: hit === null ? [] : e.shiftKey ? [...new Set([...s.selection, hit])] : [hit],
      })
      if (entity?.kind === 'point')
        this.drag = { id: entity.id, anchor: entity.at, to: entity.at, axis, moved: false }
      return true
    }
    const pointHit = this.hit(e, true),
      at = this.position(e)
    if (pointHit === null && !at) return true
    const next: SpatialSketch = structuredClone(sk)
    const id = pointHit ?? next.next_id++
    if (pointHit === null) next.entities.push({ kind: 'point', id, at: at! })
    if (s.tool !== 'point') this.clicks.push(id)
    const count = s.tool === 'line' ? 2 : 4
    if (this.clicks.length === count) {
      if (s.tool === 'line')
        next.entities.push({
          kind: 'line',
          id: next.next_id++,
          a: this.clicks[0]!,
          b: this.clicks[1]!,
        })
      else
        next.entities.push({
          kind: 'bezier',
          id: next.next_id++,
          points: [...this.clicks] as [number, number, number, number],
        })
      this.clicks = s.tool === 'line' ? [id] : []
    }
    useSpatial.setState({ selection: [id] })
    void s.change(next)
    return true
  }
  onMove(e: PointerEvent) {
    if (!this.drag) return
    const to = this.position(e, this.drag.anchor, this.drag.axis)
    if (to) {
      this.drag.to = to
      this.drag.moved = true
      this.draw()
    }
  }
  onUp() {
    const s = useSpatial.getState(),
      d = this.drag
    this.drag = null
    if (d?.moved && s.sketch) void s.change(s.sketch, { point: d.id, to: d.to })
    this.draw()
  }
  onKey(e: KeyboardEvent): boolean {
    if (!this.active) return false
    if (e.key === 'Escape') {
      this.clicks = []
      this.drag = null
      useSpatial.setState({ tool: 'select', selection: [] })
      return true
    }
    return false
  }
}

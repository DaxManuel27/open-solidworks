import * as THREE from 'three'
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js'
import { LineSegments2 } from 'three/examples/jsm/lines/LineSegments2.js'
import { LineSegmentsGeometry } from 'three/examples/jsm/lines/LineSegmentsGeometry.js'
import type { AsmInstance, AsmScene, Kind, MeshPayload } from '../protocol'
import { decodeF32, decodeU32 } from '../protocol'
import type { AsmMarks, AsmPick } from '../store'
import { COLORS, type Engine } from './engine'

/** A part's tessellation, uploaded once and shared by its instances. */
interface PartGeometry {
  mesh: THREE.BufferGeometry
  edges: THREE.BufferGeometry
  faceRanges: Uint32Array
  edgePoints: Float32Array
  edgeRanges: Uint32Array
  vertices: Float32Array
  faceCount: number
}

interface Placed {
  key: string[]
  id: string
  part: PartGeometry
  matrix: THREE.Matrix4
  mesh: THREE.Mesh
  edges: THREE.LineSegments
  material: THREE.MeshStandardMaterial
  derived: boolean
}

export interface AsmHighlights {
  hover: AsmPick | null
  selected: AsmPick[]
  components: string[][]
  marks: AsmMarks
}

export type TriadHandle = { type: 'move' | 'rotate'; axis: 0 | 1 | 2 }

const AXIS_COLORS = [0xdc2626, 0x16a34a, 0x2563eb]
const keyOf = (k: string[]) => k.join('/')

function partGeometry(p: MeshPayload): PartGeometry {
  const mesh = new THREE.BufferGeometry()
  mesh.setAttribute('position', new THREE.BufferAttribute(decodeF32(p.positions), 3))
  mesh.setAttribute('normal', new THREE.BufferAttribute(decodeF32(p.normals), 3))
  mesh.setIndex(new THREE.BufferAttribute(decodeU32(p.indices), 1))
  mesh.computeBoundingSphere()
  const edgePoints = decodeF32(p.edge_points)
  const edgeRanges = decodeU32(p.edge_ranges)
  const segs: number[] = []
  for (let e = 0; e + 1 < edgeRanges.length; e++) {
    for (let i = edgeRanges[e]!; i + 1 < edgeRanges[e + 1]!; i++) {
      for (let k = 0; k < 6; k++) segs.push(edgePoints[3 * i + k]!)
    }
  }
  const edges = new THREE.BufferGeometry().setAttribute(
    'position',
    new THREE.Float32BufferAttribute(segs, 3),
  )
  return {
    mesh,
    edges,
    faceRanges: decodeU32(p.face_ranges),
    edgePoints,
    edgeRanges,
    vertices: decodeF32(p.vertices),
    faceCount: p.face_count,
  }
}

/** Draws an assembly: shared part meshes placed per instance. */
export class AssemblyView {
  private group = new THREE.Group()
  private overlay = new THREE.Group()
  private lines = new THREE.Group()
  private triad = new THREE.Group()
  private parts = new Map<string, PartGeometry>()
  private placed: Placed[] = []
  private highlights: AsmHighlights = {
    hover: null,
    selected: [],
    components: [],
    marks: { faces: [], meshes: [] },
  }
  private triadHandles: { object: THREE.Object3D; handle: TriadHandle }[] = []
  /** Triad origin and axes, while one is shown. */
  triadFrame: THREE.Matrix4 | null = null

  constructor(private readonly engine: Engine) {
    engine.scene.add(this.group, this.lines, this.overlay, this.triad)
  }

  get active(): boolean {
    return this.placed.length > 0 || this.group.visible
  }

  clear(): void {
    for (const p of this.parts.values()) {
      p.mesh.dispose()
      p.edges.dispose()
    }
    this.parts.clear()
    this.group.clear()
    this.placed = []
    this.disposeGroup(this.lines)
    this.disposeGroup(this.overlay)
    this.engine.requestRender()
  }

  private disposeGroup(g: THREE.Group): void {
    g.traverse((o) => {
      ;(o as THREE.Mesh).geometry?.dispose()
    })
    g.clear()
  }

  /** Show an assembly scene; `preview` replaces instance positions. */
  setScene(scene: AsmScene | null, preview: AsmInstance[] | null): void {
    if (!scene) {
      this.clear()
      return
    }
    // Upload new part meshes; drop ones no instance uses any more.
    for (const [key, payload] of Object.entries(scene.parts)) {
      if (!payload || this.parts.has(key)) continue
      this.parts.set(key, partGeometry(payload))
    }
    for (const key of [...this.parts.keys()]) {
      if (!(key in scene.parts)) {
        const p = this.parts.get(key)!
        p.mesh.dispose()
        p.edges.dispose()
        this.parts.delete(key)
      }
    }
    this.group.clear()
    this.placed = []
    const positions = new Map((preview ?? []).map((i) => [keyOf(i.key), i.transform]))
    for (const inst of scene.instances) {
      if (inst.hidden || !inst.part) continue
      const part = this.parts.get(inst.part)
      if (!part) continue
      const matrix = new THREE.Matrix4().fromArray(positions.get(keyOf(inst.key)) ?? inst.transform)
      const material = new THREE.MeshStandardMaterial({
        color: inst.derived !== null ? 0xc9d1d9 : COLORS.body,
        metalness: 0.05,
        roughness: 0.55,
        polygonOffset: true,
        polygonOffsetFactor: 1,
        polygonOffsetUnits: 1,
        side: THREE.DoubleSide,
      })
      const mesh = new THREE.Mesh(part.mesh, material)
      mesh.matrixAutoUpdate = false
      mesh.matrix.copy(matrix)
      const edges = new THREE.LineSegments(
        part.edges,
        new THREE.LineBasicMaterial({ color: COLORS.edge }),
      )
      edges.matrixAutoUpdate = false
      edges.matrix.copy(matrix)
      mesh.userData.key = inst.key
      this.group.add(mesh, edges)
      this.placed.push({
        key: inst.key,
        id: keyOf(inst.key),
        part,
        matrix,
        mesh,
        edges,
        material,
        derived: inst.derived !== null,
      })
    }
    this.disposeGroup(this.lines)
    for (const [a, b] of scene.explode_lines) {
      const g = new THREE.BufferGeometry().setFromPoints([
        new THREE.Vector3(...a),
        new THREE.Vector3(...b),
      ])
      const line = new THREE.Line(
        g,
        new THREE.LineDashedMaterial({ color: 0x64748b, dashSize: 3, gapSize: 2 }),
      )
      line.computeLineDistances()
      this.lines.add(line)
    }
    this.applyHighlights()
  }

  /** Move instances without rebuilding (drag previews). */
  setPositions(instances: AsmInstance[]): void {
    const byKey = new Map(instances.map((i) => [keyOf(i.key), i.transform]))
    for (const p of this.placed) {
      const t = byKey.get(p.id)
      if (!t) continue
      p.matrix.fromArray(t)
      p.mesh.matrix.copy(p.matrix)
      p.edges.matrix.copy(p.matrix)
      p.mesh.matrixWorldNeedsUpdate = true
      p.edges.matrixWorldNeedsUpdate = true
    }
    this.applyHighlights()
  }

  setHighlights(h: AsmHighlights): void {
    this.highlights = h
    this.applyHighlights()
  }

  private applyHighlights(): void {
    this.disposeGroup(this.overlay)
    const h = this.highlights
    const chosen = new Set(h.components.map(keyOf))
    for (const p of this.placed) {
      const inSelection = [...chosen].some((c) => p.id === c || p.id.startsWith(`${c}/`))
      p.material.color.set(inSelection ? 0x93b4f5 : p.derived ? 0xc9d1d9 : COLORS.body)
    }
    const faceOverlay = (pick: AsmPick, color: number, opacity = 1) => {
      const p = this.placed.find((x) => x.id === keyOf(pick.component))
      if (!p) return
      if (pick.kind === 'face') {
        if (pick.index >= p.part.faceCount) return
        const start = p.part.faceRanges[pick.index]! * 3
        const end = p.part.faceRanges[pick.index + 1]! * 3
        const index = (p.part.mesh.index!.array as Uint32Array).slice(start, end)
        const g = new THREE.BufferGeometry()
        g.setAttribute('position', p.part.mesh.getAttribute('position'))
        g.setAttribute('normal', p.part.mesh.getAttribute('normal'))
        g.setIndex(new THREE.BufferAttribute(index, 1))
        const m = new THREE.Mesh(
          g,
          new THREE.MeshStandardMaterial({
            color,
            transparent: opacity < 1,
            opacity,
            polygonOffset: true,
            polygonOffsetFactor: -1,
            polygonOffsetUnits: -1,
            side: THREE.DoubleSide,
          }),
        )
        m.matrixAutoUpdate = false
        m.matrix.copy(p.matrix)
        m.renderOrder = 5
        this.overlay.add(m)
      } else if (pick.kind === 'edge') {
        const r = p.part.edgeRanges
        if (pick.index + 1 >= r.length) return
        const pos: number[] = []
        const v = new THREE.Vector3()
        for (let i = r[pick.index]!; i + 1 < r[pick.index + 1]!; i++) {
          for (const j of [i, i + 1]) {
            v.set(
              p.part.edgePoints[3 * j]!,
              p.part.edgePoints[3 * j + 1]!,
              p.part.edgePoints[3 * j + 2]!,
            )
            v.applyMatrix4(p.matrix)
            pos.push(v.x, v.y, v.z)
          }
        }
        const { width, height } = this.engine.viewSize
        const lm = new LineMaterial({ color, linewidth: 3, depthTest: false })
        lm.resolution.set(width, height)
        const line = new LineSegments2(new LineSegmentsGeometry().setPositions(pos), lm)
        line.renderOrder = 10
        this.overlay.add(line)
      } else {
        const vs = p.part.vertices
        if (3 * pick.index + 2 >= vs.length) return
        const pt = new THREE.Vector3(
          vs[3 * pick.index],
          vs[3 * pick.index + 1],
          vs[3 * pick.index + 2],
        )
        pt.applyMatrix4(p.matrix)
        const pts = new THREE.Points(
          new THREE.BufferGeometry().setFromPoints([pt]),
          new THREE.PointsMaterial({ color, size: 9, sizeAttenuation: false, depthTest: false }),
        )
        pts.renderOrder = 11
        this.overlay.add(pts)
      }
    }
    for (const mark of h.marks.faces) {
      for (const f of mark.faces)
        faceOverlay({ component: mark.component, kind: 'face', index: f }, 0xef4444)
    }
    for (const payload of h.marks.meshes) {
      const g = partGeometry(payload)
      const m = new THREE.Mesh(
        g.mesh,
        new THREE.MeshStandardMaterial({
          color: 0xef4444,
          transparent: true,
          opacity: 0.75,
          depthTest: false,
        }),
      )
      m.renderOrder = 12
      this.overlay.add(m)
    }
    for (const s of h.selected) faceOverlay(s, COLORS.selected)
    if (h.hover) faceOverlay(h.hover, COLORS.hover)
    this.engine.requestRender()
  }

  /** What's under the cursor, preferring vertices, then edges, then faces. */
  pick(clientX: number, clientY: number, kinds: Kind[]): AsmPick | null {
    const hit = this.hit(clientX, clientY)
    const rect = this.engine.canvas.getBoundingClientRect()
    const mx = clientX - rect.left
    const my = clientY - rect.top
    const camDir = this.engine.camera.getWorldDirection(new THREE.Vector3())
    const depth = (p: THREE.Vector3) => p.clone().sub(this.engine.camera.position).dot(camDir)
    const tol = this.engine.size * 2e-3 + this.engine.pixel * 2
    const faceDepth = hit ? depth(hit.point) : Infinity
    // Only the hit component and its neighbours on screen are scanned.
    const candidates = hit
      ? [hit.placed, ...this.placed.filter((p) => p !== hit.placed)]
      : this.placed
    if (kinds.includes('vertex')) {
      let best: AsmPick | null = null
      let bestD = 8
      for (const p of candidates) {
        const vs = p.part.vertices
        for (let i = 0; i < vs.length / 3; i++) {
          const v = new THREE.Vector3(vs[3 * i], vs[3 * i + 1], vs[3 * i + 2]).applyMatrix4(
            p.matrix,
          )
          if (depth(v) > faceDepth + tol) continue
          const s = this.engine.toScreen(v)
          const d = Math.hypot(s.x - mx, s.y - my)
          if (d < bestD) {
            bestD = d
            best = { component: p.key, kind: 'vertex', index: i }
          }
        }
      }
      if (best) return best
    }
    if (kinds.includes('edge')) {
      let best: AsmPick | null = null
      let bestD = 6
      const a = new THREE.Vector3()
      const b = new THREE.Vector3()
      for (const p of candidates) {
        const r = p.part.edgeRanges
        const pts = p.part.edgePoints
        for (let e = 0; e + 1 < r.length; e++) {
          for (let i = r[e]!; i + 1 < r[e + 1]!; i++) {
            a.set(pts[3 * i]!, pts[3 * i + 1]!, pts[3 * i + 2]!).applyMatrix4(p.matrix)
            b.set(pts[3 * i + 3]!, pts[3 * i + 4]!, pts[3 * i + 5]!).applyMatrix4(p.matrix)
            const sa = this.engine.toScreen(a)
            const sb = this.engine.toScreen(b)
            const dx = sb.x - sa.x
            const dy = sb.y - sa.y
            const len2 = dx * dx + dy * dy
            const t =
              len2 > 0 ? Math.max(0, Math.min(1, ((mx - sa.x) * dx + (my - sa.y) * dy) / len2)) : 0
            const d = Math.hypot(sa.x + t * dx - mx, sa.y + t * dy - my)
            if (d < bestD && depth(a.clone().lerp(b, t)) <= faceDepth + tol) {
              bestD = d
              best = { component: p.key, kind: 'edge', index: e }
            }
          }
        }
      }
      if (best) return best
    }
    if (hit && kinds.includes('face'))
      return { component: hit.placed.key, kind: 'face', index: hit.face }
    return null
  }

  /** The nearest component surface under the cursor. */
  hit(
    clientX: number,
    clientY: number,
  ): { placed: Placed; face: number; point: THREE.Vector3; local: THREE.Vector3 } | null {
    const ray = this.engine.raycastFrom(clientX, clientY)
    for (const p of this.placed) p.mesh.updateMatrixWorld()
    const hits = ray.intersectObjects(
      this.placed.map((p) => p.mesh),
      false,
    )
    const h = hits[0]
    if (!h || h.faceIndex == null) return null
    const placed = this.placed.find((p) => p.mesh === h.object)!
    const r = placed.part.faceRanges
    let lo = 0
    let hi = r.length - 2
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1
      if (r[mid]! <= h.faceIndex) lo = mid
      else hi = mid - 1
    }
    const local = h.point.clone().applyMatrix4(placed.matrix.clone().invert())
    return { placed, face: lo, point: h.point.clone(), local }
  }

  /** World matrix of a component instance. */
  matrixOf(key: string[]): THREE.Matrix4 | null {
    return this.placed.find((p) => p.id === keyOf(key))?.matrix.clone() ?? null
  }

  /** Show the move/rotate triad at a frame (origin and axes), or hide it. */
  setTriad(frame: THREE.Matrix4 | null): void {
    this.disposeGroup(this.triad)
    this.triadHandles = []
    this.triadFrame = frame
    if (!frame) {
      this.engine.requestRender()
      return
    }
    const size = Math.max(10, this.engine.size * 0.12)
    const origin = new THREE.Vector3().setFromMatrixPosition(frame)
    const axes = [new THREE.Vector3(), new THREE.Vector3(), new THREE.Vector3()]
    frame.extractBasis(axes[0]!, axes[1]!, axes[2]!)
    axes.forEach((axis, i) => {
      axis.normalize()
      const color = AXIS_COLORS[i]!
      const material = new THREE.MeshBasicMaterial({ color, depthTest: false })
      const shaft = new THREE.Mesh(
        new THREE.CylinderGeometry(size * 0.02, size * 0.02, size, 8),
        material,
      )
      const head = new THREE.Mesh(new THREE.ConeGeometry(size * 0.07, size * 0.2, 12), material)
      const arrow = new THREE.Group()
      shaft.position.y = size / 2
      head.position.y = size
      arrow.add(shaft, head)
      arrow.position.copy(origin)
      arrow.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), axis)
      arrow.renderOrder = 20
      this.triad.add(arrow)
      this.triadHandles.push({ object: arrow, handle: { type: 'move', axis: i as 0 | 1 | 2 } })
      const ring = new THREE.Mesh(
        new THREE.TorusGeometry(size * 0.7, size * 0.015, 6, 48),
        new THREE.MeshBasicMaterial({ color, depthTest: false, transparent: true, opacity: 0.8 }),
      )
      ring.position.copy(origin)
      ring.quaternion.setFromUnitVectors(new THREE.Vector3(0, 0, 1), axis)
      ring.renderOrder = 20
      this.triad.add(ring)
      this.triadHandles.push({ object: ring, handle: { type: 'rotate', axis: i as 0 | 1 | 2 } })
    })
    this.engine.requestRender()
  }

  /** The triad handle under the cursor. */
  pickTriad(clientX: number, clientY: number): TriadHandle | null {
    if (!this.triadFrame) return null
    const ray = this.engine.raycastFrom(clientX, clientY)
    this.triad.updateMatrixWorld(true)
    const hits = ray.intersectObjects(
      this.triadHandles.map((h) => h.object),
      true,
    )
    const h = hits[0]
    if (!h) return null
    const owner = this.triadHandles.find(
      (t) => t.object === h.object || t.object.children.includes(h.object),
    )
    return owner?.handle ?? null
  }
}

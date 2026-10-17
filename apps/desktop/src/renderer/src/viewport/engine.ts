import * as THREE from 'three'
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js'
import { LineSegments2 } from 'three/examples/jsm/lines/LineSegments2.js'
import { LineSegmentsGeometry } from 'three/examples/jsm/lines/LineSegmentsGeometry.js'
import type { DatumGeometry, Kind, MeshBodyView, PlaneData, Ref, Scene, Vec3 } from '../protocol'
import { decodeF32, decodeU32 } from '../protocol'
import type { AnalysisOverlay, FacetSelection, Pick, PickKind, ViewName } from '../store'

export const COLORS = {
  body: 0xb9c3cd,
  edge: 0x1f262e,
  hover: 0xf59e0b,
  selected: 0x2563eb,
  aux: 0x0ea5e9,
  preview: 0xfbbf24,
  plane: 0x5b8def,
  sketch: 0x1d4ed8,
  datum: 0x64748b,
  /** Edges of surface bodies, so open boundaries stand out. */
  surfaceEdge: 0x7c3aed,
  mesh: 0xa9b7a4,
  handle: 0xdc2626,
}

/** A graphics mesh body in the viewport. */
interface MeshBody {
  id: string
  mesh: THREE.Mesh
  positions: Float32Array
  indices: Uint32Array
}

/** A body uploaded to the GPU, with lookups for picking. */
interface BodyMesh {
  hidden: Record<Kind, Set<number>>
  mesh: THREE.Mesh
  edges: THREE.LineSegments
  faceRanges: Uint32Array
  edgePoints: Float32Array
  edgeRanges: Uint32Array
  vertices: Float32Array
  faceCount: number
  materials: THREE.Material[]
}

function hiddenTopology(scene: Scene): Record<Kind, Set<number>> {
  const result = { face: new Set<number>(), edge: new Set<number>(), vertex: new Set<number>() }
  for (const [kind, key] of [
    ['face', 'faces'],
    ['edge', 'edges'],
    ['vertex', 'vertices'],
  ] as const) {
    for (const body of scene.bodies)
      if (body.hidden) for (const index of body[key]) result[kind].add(index)
    for (const body of scene.bodies)
      if (!body.hidden) for (const index of body[key]) result[kind].delete(index)
  }
  return result
}

export interface HighlightSet {
  hover: Pick | null
  selected: Pick[]
  aux: Pick[]
}

const STD_PLANES: { ref: Ref; name: string; plane: PlaneData }[] = [
  {
    ref: { type: 'front_plane' },
    name: 'Front Plane',
    plane: { origin: [0, 0, 0], x_dir: [1, 0, 0], normal: [0, 0, 1] },
  },
  {
    ref: { type: 'top_plane' },
    name: 'Top Plane',
    plane: { origin: [0, 0, 0], x_dir: [1, 0, 0], normal: [0, 1, 0] },
  },
  {
    ref: { type: 'right_plane' },
    name: 'Right Plane',
    plane: { origin: [0, 0, 0], x_dir: [0, 0, -1], normal: [1, 0, 0] },
  },
]

const v3 = (a: Vec3) => new THREE.Vector3(a[0], a[1], a[2])

/** Matrix taking plane-local (x, y, along normal) to world. */
export function planeMatrix(p: PlaneData): THREE.Matrix4 {
  const x = v3(p.x_dir).normalize()
  const n = v3(p.normal).normalize()
  const y = new THREE.Vector3().crossVectors(n, x)
  return new THREE.Matrix4().makeBasis(x, y, n).setPosition(v3(p.origin))
}

export class Engine {
  readonly renderer: THREE.WebGLRenderer
  readonly scene = new THREE.Scene()
  readonly camera: THREE.OrthographicCamera
  /** Point the camera orbits and looks at. */
  private target = new THREE.Vector3()
  private halfHeight = 100
  private body: BodyMesh | null = null
  private bodyGroup = new THREE.Group()
  private previewGroup = new THREE.Group()
  private datumGroup = new THREE.Group()
  private planeGroup = new THREE.Group()
  private sketchGroup = new THREE.Group()
  private meshGroup = new THREE.Group()
  private meshBodies: MeshBody[] = []
  private facetGroup = new THREE.Group()
  private handleGroup = new THREE.Group()
  private surfaceEdges = new Set<number>()
  private analysis: AnalysisOverlay | null = null
  /** Extra face materials by colour, for analysis overlays. */
  private analysisMaterials = new Map<number, number>()
  readonly overlayGroup = new THREE.Group()
  private highlightGroup = new THREE.Group()
  private stdPlaneMeshes: { mesh: THREE.Mesh; ref: Ref }[] = []
  private datumMeshes: { object: THREE.Object3D; id: number; ref?: Ref; curve?: boolean }[] = []
  private bounds = new THREE.Box3(new THREE.Vector3(-50, -50, -50), new THREE.Vector3(50, 50, 50))
  private raycaster = new THREE.Raycaster()
  private frame = 0
  private anim: {
    from: THREE.Quaternion
    to: THREE.Quaternion
    fromTarget: THREE.Vector3
    toTarget: THREE.Vector3
    fromH: number
    toH: number
    t0: number
  } | null = null
  private highlights: HighlightSet = { hover: null, selected: [], aux: [] }
  private showStdPlanes = false
  /** Called after the camera moves, for HTML overlays. */
  onCamera: () => void = () => {}

  constructor(private readonly container: HTMLElement) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true })
    this.renderer.setPixelRatio(window.devicePixelRatio)
    container.appendChild(this.renderer.domElement)
    this.camera = new THREE.OrthographicCamera(-1, 1, 1, -1, -1e5, 1e5)
    this.camera.position.set(1, 1, 1).multiplyScalar(500)
    this.camera.up.set(0, 1, 0)
    this.camera.lookAt(0, 0, 0)

    this.scene.add(new THREE.AmbientLight(0xffffff, 0.55))
    const key = new THREE.DirectionalLight(0xffffff, 1.6)
    key.position.set(0.4, 0.7, 1)
    this.camera.add(key)
    const fill = new THREE.DirectionalLight(0xffffff, 0.5)
    fill.position.set(-0.6, -0.3, 0.5)
    this.camera.add(fill)
    this.scene.add(this.camera)
    for (const g of [
      this.bodyGroup,
      this.previewGroup,
      this.datumGroup,
      this.planeGroup,
      this.sketchGroup,
      this.meshGroup,
      this.facetGroup,
      this.highlightGroup,
      this.handleGroup,
      this.overlayGroup,
    ]) {
      this.scene.add(g)
    }
    this.buildStdPlanes()
    this.resize()
    new ResizeObserver(() => this.resize()).observe(container)
  }

  dispose(): void {
    cancelAnimationFrame(this.frame)
    this.renderer.dispose()
    this.renderer.domElement.remove()
  }

  get canvas(): HTMLCanvasElement {
    return this.renderer.domElement
  }

  /** Set the model box used for fitting and on-screen sizes. */
  setBounds(min: Vec3 | null, max: Vec3 | null): void {
    if (min && max) this.bounds.set(v3(min), v3(max))
    else this.bounds.set(new THREE.Vector3(-50, -50, -50), new THREE.Vector3(50, 50, 50))
    this.buildStdPlanes()
  }

  /** Viewport size in pixels. */
  get viewSize(): { width: number; height: number } {
    return { width: this.container.clientWidth, height: this.container.clientHeight }
  }

  /** A raycaster aimed through a screen point. */
  raycastFrom(clientX: number, clientY: number): THREE.Raycaster {
    this.raycaster.setFromCamera(this.ndc(clientX, clientY), this.camera)
    return this.raycaster
  }

  /** Model size, for scaling things drawn on screen. */
  get size(): number {
    return this.bounds.getSize(new THREE.Vector3()).length() || 100
  }

  /** World units per screen pixel. */
  get pixel(): number {
    return (2 * this.halfHeight) / this.camera.zoom / Math.max(1, this.container.clientHeight)
  }

  private resize(): void {
    const w = this.container.clientWidth
    const h = this.container.clientHeight
    if (!w || !h) return
    this.renderer.setSize(w, h)
    this.updateFrustum()
    this.highlightGroup.traverse((o) => {
      const m = (o as THREE.Mesh).material
      if (m instanceof LineMaterial) m.resolution.set(w, h)
    })
    this.requestRender()
  }

  private updateFrustum(): void {
    const w = this.container.clientWidth || 1
    const h = this.container.clientHeight || 1
    const aspect = w / h
    this.camera.top = this.halfHeight
    this.camera.bottom = -this.halfHeight
    this.camera.left = -this.halfHeight * aspect
    this.camera.right = this.halfHeight * aspect
    this.camera.updateProjectionMatrix()
  }

  requestRender(): void {
    if (this.frame) return
    this.frame = requestAnimationFrame(() => {
      this.frame = 0
      this.render()
    })
  }

  private render(): void {
    if (this.anim) {
      const t = Math.min(1, (performance.now() - this.anim.t0) / 260)
      const e = t * (2 - t)
      const q = this.anim.from.clone().slerp(this.anim.to, e)
      this.target.lerpVectors(this.anim.fromTarget, this.anim.toTarget, e)
      this.halfHeight = this.anim.fromH + (this.anim.toH - this.anim.fromH) * e
      this.placeCamera(q)
      if (t >= 1) this.anim = null
      else this.requestRender()
    }
    this.renderer.render(this.scene, this.camera)
    this.onCamera()
  }

  private placeCamera(q: THREE.Quaternion): void {
    const dist = this.size * 10 + 1000
    const back = new THREE.Vector3(0, 0, 1).applyQuaternion(q)
    this.camera.position.copy(this.target).addScaledVector(back, dist)
    this.camera.quaternion.copy(q)
    this.camera.up.set(0, 1, 0).applyQuaternion(q)
    this.updateFrustum()
  }

  // ---- Scene content ----

  setScene(scene: Scene | null): void {
    this.clearBody()
    this.disposeThreadLines(this.datumGroup)
    this.datumGroup.clear()
    this.datumMeshes = []
    this.sketchGroup.clear()
    if (!scene) {
      this.requestRender()
      return
    }
    if (scene.bbox) {
      this.bounds.set(v3(scene.bbox.min), v3(scene.bbox.max))
    } else {
      this.bounds.set(new THREE.Vector3(-50, -50, -50), new THREE.Vector3(50, 50, 50))
    }
    this.surfaceEdges = new Set(
      scene.bodies.filter((b) => b.kind === 'surface' && !b.hidden).flatMap((b) => b.edges),
    )
    if (scene.body)
      this.body = this.uploadBody(scene.body, this.bodyGroup, false, hiddenTopology(scene))
    this.setMeshes(scene.meshes ?? [])
    for (const d of scene.datums) this.addDatum(d.id, d.geometry)
    for (const curve of [
      ...scene.curves,
      ...scene.sketches3d.map((s) => ({ ...s, segments: [] })),
    ]) {
      const addCurves = (polylines: number[][], ref?: Ref) => {
        const group = new THREE.Group()
        for (const polyline of polylines) {
          const geometry = new THREE.BufferGeometry().setAttribute(
            'position',
            new THREE.Float32BufferAttribute(polyline, 3),
          )
          group.add(new THREE.Line(geometry, new THREE.LineBasicMaterial({ color: COLORS.sketch })))
        }
        this.datumGroup.add(group)
        this.datumMeshes.push({ object: group, id: curve.id, ref, curve: true })
      }
      const segments = curve.segments ?? []
      for (const segment of segments) addCurves(segment.polylines, segment.reference)
      // Preserve display and whole-curve picking for pieces without unique provenance.
      if (
        segments.reduce((n, segment) => n + segment.polylines.length, 0) !== curve.polylines.length
      )
        addCurves(curve.polylines)
    }
    for (const annotation of scene.cosmetic_threads ?? []) {
      if (annotation.hidden) continue
      this.addThreadLines(annotation.polylines, this.datumGroup)
    }
    for (const bead of scene.weld_beads ?? []) {
      const geometry = new THREE.BufferGeometry().setAttribute(
        'position',
        new THREE.Float32BufferAttribute(bead.polyline, 3),
      )
      const line = new THREE.Line(
        geometry,
        new THREE.LineBasicMaterial({ color: 0xd97706, linewidth: 3, depthTest: false }),
      )
      line.renderOrder = 4
      line.userData.cosmeticThread = true
      line.userData.weldBead = bead.feature
      this.datumGroup.add(line)
    }
    for (const bend of scene.bend_lines ?? []) {
      const geometry = new THREE.BufferGeometry().setAttribute(
        'position',
        new THREE.Float32BufferAttribute(bend.polyline, 3),
      )
      const material = new THREE.LineDashedMaterial({
        color: bend.up ? 0x15803d : 0x1d4ed8,
        dashSize: 3,
        gapSize: 1.5,
        depthTest: false,
      })
      const line = new THREE.Line(geometry, material)
      line.computeLineDistances()
      line.renderOrder = 4
      // Disposed with the cosmetic thread lines.
      line.userData.cosmeticThread = true
      line.userData.bendLine = bend.up ? 'up' : 'down'
      this.datumGroup.add(line)
    }
    for (const s of scene.sketches) {
      const m = new THREE.LineBasicMaterial({ color: COLORS.sketch })
      for (const pl of s.polylines) {
        const g = new THREE.BufferGeometry().setAttribute(
          'position',
          new THREE.Float32BufferAttribute(pl, 3),
        )
        this.sketchGroup.add(new THREE.Line(g, m))
      }
    }
    this.buildStdPlanes()
    this.applyHighlights()
  }

  setPreview(scene: Scene | null): void {
    this.disposeThreadLines(this.previewGroup)
    this.previewGroup.traverse((o) => (o as THREE.Mesh).geometry?.dispose())
    this.previewGroup.clear()
    if (scene?.body) this.uploadBody(scene.body, this.previewGroup, true, hiddenTopology(scene))
    for (const annotation of scene?.cosmetic_threads ?? []) {
      if (!annotation.hidden) this.addThreadLines(annotation.polylines, this.previewGroup)
    }
    // Dim the real body so the preview reads clearly.
    if (this.body) {
      const m = this.body.materials[0] as THREE.MeshStandardMaterial
      m.transparent = !!scene?.body
      m.opacity = scene?.body ? 0.35 : 1
      m.depthWrite = !scene?.body
      m.needsUpdate = true
    }
    this.requestRender()
  }

  setStdPlanesVisible(v: boolean): void {
    this.showStdPlanes = v
    this.planeGroup.visible = v
    this.requestRender()
  }

  private disposeThreadLines(group: THREE.Group): void {
    group.traverse((object) => {
      if (object.userData.cosmeticThread && object instanceof THREE.Line) {
        object.geometry.dispose()
        const materials = Array.isArray(object.material) ? object.material : [object.material]
        for (const material of materials) material.dispose()
      }
    })
  }

  private addThreadLines(polylines: number[][], group: THREE.Group): void {
    for (const polyline of polylines) {
      const geometry = new THREE.BufferGeometry().setAttribute(
        'position',
        new THREE.Float32BufferAttribute(polyline, 3),
      )
      const material = new THREE.LineBasicMaterial({ color: 0xc78d36, depthTest: false })
      const line = new THREE.Line(geometry, material)
      line.renderOrder = 4
      line.userData.cosmeticThread = true
      group.add(line)
    }
  }

  private clearBody(): void {
    this.bodyGroup.traverse((o) => (o as THREE.Mesh).geometry?.dispose())
    this.bodyGroup.clear()
    this.body = null
  }

  private uploadBody(
    p: Scene['body'] & object,
    group: THREE.Group,
    preview: boolean,
    hidden: Record<Kind, Set<number>>,
  ): BodyMesh {
    const geom = new THREE.BufferGeometry()
    geom.setAttribute('position', new THREE.BufferAttribute(decodeF32(p.positions), 3))
    geom.setAttribute('normal', new THREE.BufferAttribute(decodeF32(p.normals), 3))
    geom.setIndex(new THREE.BufferAttribute(decodeU32(p.indices), 1))
    const faceRanges = decodeU32(p.face_ranges)
    const base = new THREE.MeshStandardMaterial({
      color: preview ? COLORS.preview : COLORS.body,
      metalness: 0.05,
      roughness: 0.55,
      polygonOffset: true,
      polygonOffsetFactor: 1,
      polygonOffsetUnits: 1,
      transparent: preview,
      opacity: preview ? 0.75 : 1,
      side: THREE.DoubleSide,
    })
    const mat = (c: number) =>
      new THREE.MeshStandardMaterial({
        color: c,
        metalness: 0.05,
        roughness: 0.6,
        polygonOffset: true,
        polygonOffsetFactor: 1,
        polygonOffsetUnits: 1,
        side: THREE.DoubleSide,
      })
    const materials = [base, mat(COLORS.hover), mat(COLORS.selected), mat(COLORS.aux)]
    for (let f = 0; f < p.face_count; f++) {
      if (!hidden.face.has(f))
        geom.addGroup(faceRanges[f]! * 3, (faceRanges[f + 1]! - faceRanges[f]!) * 3, 0)
    }
    const mesh = new THREE.Mesh(geom, materials)
    group.add(mesh)

    const edgePoints = decodeF32(p.edge_points)
    const edgeRanges = decodeU32(p.edge_ranges)
    const segs: number[] = []
    for (let e = 0; e + 1 < edgeRanges.length; e++) {
      if (hidden.edge.has(e)) continue
      for (let i = edgeRanges[e]!; i + 1 < edgeRanges[e + 1]!; i++) {
        segs.push(edgePoints[3 * i]!, edgePoints[3 * i + 1]!, edgePoints[3 * i + 2]!)
        segs.push(edgePoints[3 * i + 3]!, edgePoints[3 * i + 4]!, edgePoints[3 * i + 5]!)
      }
    }
    const eg = new THREE.BufferGeometry().setAttribute(
      'position',
      new THREE.Float32BufferAttribute(segs, 3),
    )
    const edges = new THREE.LineSegments(
      eg,
      new THREE.LineBasicMaterial({ color: preview ? 0x92400e : COLORS.edge }),
    )
    group.add(edges)
    if (!preview && this.surfaceEdges.size) {
      const open: number[] = []
      for (const e of this.surfaceEdges) {
        if (hidden.edge.has(e) || e + 1 >= edgeRanges.length) continue
        for (let i = edgeRanges[e]!; i + 1 < edgeRanges[e + 1]!; i++)
          for (let k = 0; k < 6; k++) open.push(edgePoints[3 * i + k]!)
      }
      const og = new THREE.BufferGeometry().setAttribute(
        'position',
        new THREE.Float32BufferAttribute(open, 3),
      )
      group.add(
        new THREE.LineSegments(og, new THREE.LineBasicMaterial({ color: COLORS.surfaceEdge })),
      )
    }
    this.requestRender()
    return {
      hidden,
      mesh,
      edges,
      faceRanges,
      edgePoints,
      edgeRanges,
      vertices: decodeF32(p.vertices),
      faceCount: p.face_count,
      materials,
    }
  }

  private addDatum(id: number, d: DatumGeometry): void {
    const size = this.size * 0.35
    const color = COLORS.datum
    if (d.type === 'plane') {
      const { mesh, outline } = this.planeQuad(d.plane, size, COLORS.plane)
      this.datumGroup.add(mesh, outline)
      this.datumMeshes.push({ object: mesh, id })
    } else if (d.type === 'axis') {
      const o = v3(d.origin)
      const dir = v3(d.dir).normalize().multiplyScalar(size)
      const g = new THREE.BufferGeometry().setFromPoints([o.clone().sub(dir), o.clone().add(dir)])
      const line = new THREE.Line(
        g,
        new THREE.LineDashedMaterial({ color, dashSize: size / 20, gapSize: size / 40 }),
      )
      line.computeLineDistances()
      this.datumGroup.add(line)
      this.datumMeshes.push({ object: line, id })
    } else if (d.type === 'point') {
      const g = new THREE.BufferGeometry().setFromPoints([v3(d.point)])
      const pts = new THREE.Points(
        g,
        new THREE.PointsMaterial({ color, size: 7, sizeAttenuation: false }),
      )
      this.datumGroup.add(pts)
      this.datumMeshes.push({ object: pts, id })
    } else {
      const o = v3(d.origin)
      const axes = new THREE.Group()
      for (const [dir, c] of [
        [d.x, 0xdc2626],
        [d.y, 0x16a34a],
        [d.z, 0x2563eb],
      ] as [Vec3, number][]) {
        axes.add(new THREE.ArrowHelper(v3(dir).normalize(), o, size * 0.4, c))
      }
      this.datumGroup.add(axes)
      this.datumMeshes.push({ object: axes, id })
    }
  }

  private planeQuad(
    p: PlaneData,
    size: number,
    color: number,
  ): { mesh: THREE.Mesh; outline: THREE.LineLoop } {
    const g = new THREE.PlaneGeometry(size, size)
    const m = new THREE.MeshBasicMaterial({
      color,
      transparent: true,
      opacity: 0.12,
      side: THREE.DoubleSide,
      depthWrite: false,
    })
    const mesh = new THREE.Mesh(g, m)
    mesh.matrixAutoUpdate = false
    mesh.matrix.copy(planeMatrix(p))
    const h = size / 2
    const og = new THREE.BufferGeometry().setFromPoints([
      new THREE.Vector3(-h, -h, 0),
      new THREE.Vector3(h, -h, 0),
      new THREE.Vector3(h, h, 0),
      new THREE.Vector3(-h, h, 0),
    ])
    const outline = new THREE.LineLoop(
      og,
      new THREE.LineBasicMaterial({ color, transparent: true, opacity: 0.6 }),
    )
    outline.matrixAutoUpdate = false
    outline.matrix.copy(mesh.matrix)
    return { mesh, outline }
  }

  private buildStdPlanes(): void {
    this.planeGroup.clear()
    this.stdPlaneMeshes = []
    const size = Math.max(60, this.size * 0.6)
    for (const sp of STD_PLANES) {
      const { mesh, outline } = this.planeQuad(sp.plane, size, COLORS.plane)
      this.planeGroup.add(mesh, outline)
      this.stdPlaneMeshes.push({ mesh, ref: sp.ref })
    }
    // Origin marker.
    const axes = new THREE.AxesHelper(Math.max(8, this.size * 0.06))
    this.planeGroup.add(axes)
    this.planeGroup.visible = this.showStdPlanes
  }

  // ---- Highlights ----

  setHighlights(h: HighlightSet): void {
    this.highlights = h
    this.applyHighlights()
  }

  private applyHighlights(): void {
    for (const o of [...this.highlightGroup.children]) {
      ;(o as THREE.Mesh).geometry?.dispose()
      this.highlightGroup.remove(o)
    }
    // Datum/curve highlighting also works when the document has no solid body.
    const sel = [
      ...this.highlights.selected,
      ...this.highlights.aux,
      ...(this.highlights.hover ? [this.highlights.hover] : []),
    ]
    for (const { object, id, ref, curve } of this.datumMeshes) {
      const on = sel.some(
        (p) =>
          p.kind === 'datum' &&
          p.id === id &&
          (!p.ref || JSON.stringify(p.ref) === JSON.stringify(ref)),
      )
      if (curve)
        object.traverse((child) => {
          const material = (child as THREE.Line).material
          if (material instanceof THREE.LineBasicMaterial)
            material.color.set(on ? COLORS.hover : COLORS.sketch)
        })
      else setOpacity(object, on ? 0.35 : 0.12)
    }
    for (const { mesh, ref } of this.stdPlaneMeshes) {
      const on = sel.some((p) => p.kind === 'std_plane' && p.ref.type === ref.type)
      setOpacity(mesh, on ? 0.35 : 0.12)
    }
    const b = this.body
    if (!b) {
      this.requestRender()
      return
    }
    // Faces: material per face via geometry groups.
    const style = new Uint8Array(b.faceCount)
    const mark = (p: { kind: string; index?: number }, s: number) => {
      if (p.kind === 'face' && p.index !== undefined && p.index < b.faceCount) style[p.index] = s
    }
    if (this.analysis) {
      for (const [face, color] of Object.entries(this.analysis.faces)) {
        const f = Number(face)
        if (f < b.faceCount) style[f] = this.analysisMaterial(b, color)
      }
    }
    for (const a of this.highlights.aux) mark(a, 3)
    for (const s of this.highlights.selected) mark(s as { kind: string; index?: number }, 2)
    if (this.highlights.hover) mark(this.highlights.hover as { kind: string; index?: number }, 1)
    const g = b.mesh.geometry
    g.clearGroups()
    for (let f = 0; f < b.faceCount; f++) {
      if (b.hidden.face.has(f)) continue
      const start = b.faceRanges[f]! * 3
      const end = b.faceRanges[f + 1]! * 3
      if (end > start) g.addGroup(start, end - start, style[f]!)
    }
    // Edges and vertices: fat lines and dots.
    const edgeSets: [number, number[]][] = [
      [COLORS.aux, []],
      [COLORS.selected, []],
      [COLORS.hover, []],
    ]
    const vertexSets: [number, number[]][] = [
      [COLORS.selected, []],
      [COLORS.hover, []],
    ]
    const add = (p: Pick, edgeIdx: number, vertexIdx: number) => {
      if (p.kind === 'edge' && !b.hidden.edge.has(p.index)) edgeSets[edgeIdx]![1].push(p.index)
      if (p.kind === 'vertex' && vertexIdx >= 0 && !b.hidden.vertex.has(p.index))
        vertexSets[vertexIdx]![1].push(p.index)
    }
    for (const a of this.highlights.aux) add(a as Pick, 0, 0)
    for (const s of this.highlights.selected) add(s, 1, 0)
    if (this.highlights.hover) add(this.highlights.hover, 2, 1)
    const w = this.container.clientWidth
    const h = this.container.clientHeight
    for (const [color, ids] of edgeSets) {
      if (!ids.length) continue
      const pos: number[] = []
      for (const e of ids) {
        for (let i = b.edgeRanges[e]!; i + 1 < b.edgeRanges[e + 1]!; i++) {
          for (let k = 0; k < 6; k++) pos.push(b.edgePoints[3 * i + k]!)
        }
      }
      const lg = new LineSegmentsGeometry().setPositions(pos)
      const lm = new LineMaterial({ color, linewidth: 3, depthTest: false })
      lm.resolution.set(w, h)
      const line = new LineSegments2(lg, lm)
      line.renderOrder = 10
      this.highlightGroup.add(line)
    }
    for (const [color, ids] of vertexSets) {
      if (!ids.length) continue
      const pts = ids.map(
        (i) => new THREE.Vector3(b.vertices[3 * i], b.vertices[3 * i + 1], b.vertices[3 * i + 2]),
      )
      const pg = new THREE.BufferGeometry().setFromPoints(pts)
      const pm = new THREE.Points(
        pg,
        new THREE.PointsMaterial({ color, size: 9, sizeAttenuation: false, depthTest: false }),
      )
      pm.renderOrder = 11
      this.highlightGroup.add(pm)
    }
    this.requestRender()
  }

  // ---- Picking ----

  private ndc(clientX: number, clientY: number): THREE.Vector2 {
    const r = this.canvas.getBoundingClientRect()
    return new THREE.Vector2(
      ((clientX - r.left) / r.width) * 2 - 1,
      -((clientY - r.top) / r.height) * 2 + 1,
    )
  }

  /** Ray from the camera through a screen point. */
  ray(clientX: number, clientY: number): THREE.Ray {
    this.raycaster.setFromCamera(this.ndc(clientX, clientY), this.camera)
    return this.raycaster.ray.clone()
  }

  toScreen(p: THREE.Vector3): { x: number; y: number } {
    const v = p.clone().project(this.camera)
    const w = this.container.clientWidth
    const h = this.container.clientHeight
    return { x: ((v.x + 1) / 2) * w, y: ((1 - v.y) / 2) * h }
  }

  /** Nearest face hit under the cursor: face index and world point. */
  hitFace(clientX: number, clientY: number): { index: number; point: THREE.Vector3 } | null {
    if (!this.body) return null
    this.raycaster.setFromCamera(this.ndc(clientX, clientY), this.camera)
    const hits = this.raycaster.intersectObject(this.body.mesh)
    const hit = hits[0]
    if (!hit || hit.faceIndex == null) return null
    return { index: this.faceOfTriangle(hit.faceIndex), point: hit.point }
  }

  private faceOfTriangle(tri: number): number {
    const r = this.body!.faceRanges
    let lo = 0
    let hi = r.length - 2
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1
      if (r[mid]! <= tri) lo = mid
      else hi = mid - 1
    }
    return lo
  }

  /** Pick what's under the cursor, preferring vertices, then edges, then faces. */
  pick(clientX: number, clientY: number, kinds: PickKind[]): Pick | null {
    const rect = this.canvas.getBoundingClientRect()
    const mx = clientX - rect.left
    const my = clientY - rect.top
    const face = this.hitFace(clientX, clientY)
    const camDir = this.camera.getWorldDirection(new THREE.Vector3())
    const depth = (p: THREE.Vector3) => p.clone().sub(this.camera.position).dot(camDir)
    const tol = this.size * 2e-3 + this.pixel * 2
    const faceDepth = face ? depth(face.point) : Infinity
    const b = this.body
    if (b && kinds.includes('vertex')) {
      let best = -1
      let bestD = 8
      for (let i = 0; i < b.vertices.length / 3; i++) {
        if (b.hidden.vertex.has(i)) continue
        const p = new THREE.Vector3(b.vertices[3 * i], b.vertices[3 * i + 1], b.vertices[3 * i + 2])
        if (depth(p) > faceDepth + tol) continue
        const s = this.toScreen(p)
        const d = Math.hypot(s.x - mx, s.y - my)
        if (d < bestD) {
          bestD = d
          best = i
        }
      }
      if (best >= 0) return { kind: 'vertex', index: best }
    }
    if (b && kinds.includes('edge')) {
      let best = -1
      let bestD = 6
      for (let e = 0; e + 1 < b.edgeRanges.length; e++) {
        if (b.hidden.edge.has(e)) continue
        for (let i = b.edgeRanges[e]!; i + 1 < b.edgeRanges[e + 1]!; i++) {
          const p = new THREE.Vector3(
            b.edgePoints[3 * i],
            b.edgePoints[3 * i + 1],
            b.edgePoints[3 * i + 2],
          )
          const q = new THREE.Vector3(
            b.edgePoints[3 * i + 3],
            b.edgePoints[3 * i + 4],
            b.edgePoints[3 * i + 5],
          )
          const sp = this.toScreen(p)
          const sq = this.toScreen(q)
          const dx = sq.x - sp.x
          const dy = sq.y - sp.y
          const len2 = dx * dx + dy * dy
          const t =
            len2 > 0 ? Math.max(0, Math.min(1, ((mx - sp.x) * dx + (my - sp.y) * dy) / len2)) : 0
          const d = Math.hypot(sp.x + t * dx - mx, sp.y + t * dy - my)
          if (d < bestD && depth(p.lerp(q, t)) <= faceDepth + tol) {
            bestD = d
            best = e
          }
        }
      }
      if (best >= 0) return { kind: 'edge', index: best }
    }
    if (face && kinds.includes('face')) return { kind: 'face', index: face.index }
    this.raycaster.setFromCamera(this.ndc(clientX, clientY), this.camera)
    if (kinds.includes('datum') && this.datumMeshes.length) {
      this.raycaster.params.Line = { threshold: this.pixel * 5 }
      this.raycaster.params.Points = { threshold: this.pixel * 5 }
      const hits = this.raycaster.intersectObjects(
        this.datumMeshes.map((d) => d.object),
        true,
      )
      const hit = hits[0]
      if (hit) {
        const d = this.datumMeshes.find(
          (x) => x.object === hit.object || x.object.children.includes(hit.object),
        )
        if (d) return { kind: 'datum', id: d.id, ...(d.ref ? { ref: d.ref } : {}) }
      }
    }
    if (kinds.includes('std_plane') && this.planeGroup.visible) {
      for (const { mesh } of this.stdPlaneMeshes) mesh.updateMatrixWorld()
      const hits = this.raycaster.intersectObjects(this.stdPlaneMeshes.map((s) => s.mesh))
      const hit = hits[0]
      if (hit) {
        const sp = this.stdPlaneMeshes.find((s) => s.mesh === hit.object)
        if (sp) return { kind: 'std_plane', ref: sp.ref }
      }
    }
    return null
  }

  // ---- Phase 6: meshes, analysis, painting and handles ----

  private analysisMaterial(b: BodyMesh, color: number): number {
    const known = this.analysisMaterials.get(color)
    if (known !== undefined && b.materials[known]) return known
    const m = new THREE.MeshStandardMaterial({
      color,
      metalness: 0.05,
      roughness: 0.6,
      polygonOffset: true,
      polygonOffsetFactor: 1,
      polygonOffsetUnits: 1,
      side: THREE.DoubleSide,
    })
    b.materials.push(m)
    b.mesh.material = b.materials
    const index = b.materials.length - 1
    this.analysisMaterials.set(color, index)
    return index
  }

  /** Colour faces (and a mesh body's vertices) by an analysis, or clear it. */
  setAnalysis(a: AnalysisOverlay | null): void {
    this.analysis = a
    for (const m of this.meshBodies) {
      const geom = m.mesh.geometry
      const colors = a?.mesh?.body === m.id ? a.mesh.colors : null
      if (colors) {
        const c = new Float32Array(colors.length * 3)
        colors.forEach((v, i) => {
          c[3 * i] = ((v >> 16) & 255) / 255
          c[3 * i + 1] = ((v >> 8) & 255) / 255
          c[3 * i + 2] = (v & 255) / 255
        })
        geom.setAttribute('color', new THREE.BufferAttribute(c, 3))
        ;(m.mesh.material as THREE.MeshStandardMaterial).vertexColors = true
      } else if (!m.mesh.userData.fileColors) {
        geom.deleteAttribute('color')
        ;(m.mesh.material as THREE.MeshStandardMaterial).vertexColors = false
      }
      ;(m.mesh.material as THREE.Material).needsUpdate = true
    }
    this.applyHighlights()
  }

  private setMeshes(meshes: MeshBodyView[]): void {
    for (const m of this.meshBodies) {
      m.mesh.geometry.dispose()
      ;(m.mesh.material as THREE.Material).dispose()
    }
    this.meshGroup.clear()
    this.meshBodies = []
    for (const view of meshes) {
      if (view.hidden) continue
      const positions = decodeF32(view.positions)
      const indices = decodeU32(view.indices)
      const geom = new THREE.BufferGeometry()
      geom.setAttribute('position', new THREE.BufferAttribute(positions, 3))
      geom.setIndex(new THREE.BufferAttribute(indices, 1))
      geom.computeVertexNormals()
      const material = new THREE.MeshStandardMaterial({
        color: COLORS.mesh,
        metalness: 0.02,
        roughness: 0.7,
        side: THREE.DoubleSide,
        flatShading: true,
      })
      const mesh = new THREE.Mesh(geom, material)
      if (view.colors) {
        const raw = decodeU32(view.colors)
        const c = new Float32Array(raw.length * 3)
        raw.forEach((v, i) => {
          c[3 * i] = ((v >> 16) & 255) / 255
          c[3 * i + 1] = ((v >> 8) & 255) / 255
          c[3 * i + 2] = (v & 255) / 255
        })
        geom.setAttribute('color', new THREE.BufferAttribute(c, 3))
        material.vertexColors = true
        mesh.userData.fileColors = true
      }
      mesh.userData.meshBody = view.id
      this.meshGroup.add(mesh)
      this.meshBodies.push({ id: view.id, mesh, positions, indices })
    }
    if (this.analysis) this.setAnalysis(this.analysis)
  }

  /** The mesh body triangle under the cursor. */
  pickFacet(
    clientX: number,
    clientY: number,
  ): { body: string; triangle: number; point: THREE.Vector3 } | null {
    if (!this.meshBodies.length) return null
    this.raycaster.setFromCamera(this.ndc(clientX, clientY), this.camera)
    const hit = this.raycaster.intersectObjects(this.meshBodies.map((m) => m.mesh))[0]
    if (!hit || hit.faceIndex == null) return null
    return {
      body: hit.object.userData.meshBody as string,
      triangle: hit.faceIndex,
      point: hit.point,
    }
  }

  /** Triangles of a mesh body with their centre within `radius` of `point`. */
  trianglesNear(body: string, point: THREE.Vector3, radius: number): number[] {
    const m = this.meshBodies.find((x) => x.id === body)
    if (!m) return []
    const out: number[] = []
    const p = m.positions
    const r2 = radius * radius
    for (let t = 0; t < m.indices.length / 3; t++) {
      let cx = 0
      let cy = 0
      let cz = 0
      for (let k = 0; k < 3; k++) {
        const v = m.indices[3 * t + k]!
        cx += p[3 * v]!
        cy += p[3 * v + 1]!
        cz += p[3 * v + 2]!
      }
      const dx = cx / 3 - point.x
      const dy = cy / 3 - point.y
      const dz = cz / 3 - point.z
      if (dx * dx + dy * dy + dz * dz <= r2) out.push(t)
    }
    return out
  }

  /** Highlight painted triangles of a mesh body. */
  setFacets(sel: FacetSelection | null): void {
    this.facetGroup.traverse((o) => (o as THREE.Mesh).geometry?.dispose())
    this.facetGroup.clear()
    const m = sel && this.meshBodies.find((x) => x.id === sel.body)
    if (m && sel.triangles.length) {
      const idx = new Uint32Array(sel.triangles.length * 3)
      sel.triangles.forEach((t, i) => {
        idx[3 * i] = m.indices[3 * t]!
        idx[3 * i + 1] = m.indices[3 * t + 1]!
        idx[3 * i + 2] = m.indices[3 * t + 2]!
      })
      const geom = new THREE.BufferGeometry()
      geom.setAttribute('position', new THREE.BufferAttribute(m.positions, 3))
      geom.setIndex(new THREE.BufferAttribute(idx, 1))
      const mat = new THREE.MeshBasicMaterial({
        color: COLORS.selected,
        transparent: true,
        opacity: 0.55,
        side: THREE.DoubleSide,
        polygonOffset: true,
        polygonOffsetFactor: -2,
        polygonOffsetUnits: -2,
      })
      this.facetGroup.add(new THREE.Mesh(geom, mat))
    }
    this.requestRender()
  }

  /** A face's area-weighted centre and its normal there. */
  faceFrame(face: number): { center: THREE.Vector3; normal: THREE.Vector3 } | null {
    const b = this.body
    if (!b || face >= b.faceCount) return null
    const pos = b.mesh.geometry.getAttribute('position') as THREE.BufferAttribute
    const index = b.mesh.geometry.getIndex()!
    const center = new THREE.Vector3()
    const normal = new THREE.Vector3()
    let area = 0
    const a = new THREE.Vector3()
    const c = new THREE.Vector3()
    const d = new THREE.Vector3()
    for (let t = b.faceRanges[face]!; t < b.faceRanges[face + 1]!; t++) {
      a.fromBufferAttribute(pos, index.getX(3 * t))
      c.fromBufferAttribute(pos, index.getX(3 * t + 1))
      d.fromBufferAttribute(pos, index.getX(3 * t + 2))
      const n = new THREE.Vector3().crossVectors(c.clone().sub(a), d.clone().sub(a))
      const w = n.length() / 2
      center.addScaledVector(a.clone().add(c).add(d).divideScalar(3), w)
      normal.add(n)
      area += w
    }
    if (area <= 0) return null
    return { center: center.divideScalar(area), normal: normal.normalize() }
  }

  /** Show (or hide) a drag arrow along `dir` from `origin`. */
  setHandle(h: { origin: THREE.Vector3; dir: THREE.Vector3 } | null): void {
    this.handleGroup.clear()
    if (h) {
      const arrow = new THREE.ArrowHelper(
        h.dir.clone().normalize(),
        h.origin,
        this.size * 0.12,
        COLORS.handle,
        this.size * 0.035,
        this.size * 0.02,
      )
      arrow.renderOrder = 10
      arrow.traverse((o) => {
        const m = (o as THREE.Mesh).material as THREE.Material | undefined
        if (m) m.depthTest = false
      })
      this.handleGroup.add(arrow)
    }
    this.requestRender()
  }

  /** Whether the cursor is on the drag arrow. */
  onHandle(clientX: number, clientY: number): boolean {
    if (!this.handleGroup.children.length) return false
    this.raycaster.setFromCamera(this.ndc(clientX, clientY), this.camera)
    this.raycaster.params.Line = { threshold: this.pixel * 6 }
    return this.raycaster.intersectObjects(this.handleGroup.children, true).length > 0
  }

  /** How far along the line (origin, dir) the cursor is, in model units. */
  along(origin: THREE.Vector3, dir: THREE.Vector3, clientX: number, clientY: number): number {
    this.raycaster.setFromCamera(this.ndc(clientX, clientY), this.camera)
    const ray = this.raycaster.ray
    const d = dir.clone().normalize()
    // Closest points between the ray and the line.
    const w0 = ray.origin.clone().sub(origin)
    const a = ray.direction.dot(ray.direction)
    const b = ray.direction.dot(d)
    const c = d.dot(d)
    const dd = ray.direction.dot(w0)
    const e = d.dot(w0)
    const den = a * c - b * b
    if (Math.abs(den) < 1e-12) return 0
    return (a * e - b * dd) / den
  }

  // ---- Camera ----

  rotate(dx: number, dy: number, pivot?: THREE.Vector3): void {
    const q = this.camera.quaternion.clone()
    const up = new THREE.Vector3(0, 1, 0).applyQuaternion(q)
    const right = new THREE.Vector3(1, 0, 0).applyQuaternion(q)
    const rot = new THREE.Quaternion()
      .setFromAxisAngle(up, -dx * 0.008)
      .multiply(new THREE.Quaternion().setFromAxisAngle(right, -dy * 0.008))
    if (pivot) {
      this.target.sub(pivot).applyQuaternion(rot).add(pivot)
    }
    this.placeCamera(rot.multiply(q))
    this.requestRender()
  }

  pan(dx: number, dy: number): void {
    const q = this.camera.quaternion
    const s = this.pixel
    this.target
      .addScaledVector(new THREE.Vector3(1, 0, 0).applyQuaternion(q), -dx * s)
      .addScaledVector(new THREE.Vector3(0, 1, 0).applyQuaternion(q), dy * s)
    this.placeCamera(q.clone())
    this.requestRender()
  }

  /** Zoom about the cursor. `factor` < 1 zooms in. */
  zoom(factor: number, clientX: number, clientY: number): void {
    const before = this.ray(clientX, clientY).origin
    this.halfHeight = Math.min(1e6, Math.max(1e-3, this.halfHeight * factor))
    this.placeCamera(this.camera.quaternion.clone())
    this.camera.updateMatrixWorld()
    const after = this.ray(clientX, clientY).origin
    this.target.add(before.sub(after))
    this.placeCamera(this.camera.quaternion.clone())
    this.requestRender()
  }

  /** Animate to look along `dir` (from the camera toward the model) with `up`. */
  private orient(back: THREE.Vector3, up: THREE.Vector3, fit = true): void {
    const z = back.clone().normalize()
    const x = new THREE.Vector3().crossVectors(up, z).normalize()
    if (x.lengthSq() < 1e-9) x.set(1, 0, 0)
    const y = new THREE.Vector3().crossVectors(z, x)
    const to = new THREE.Quaternion().setFromRotationMatrix(new THREE.Matrix4().makeBasis(x, y, z))
    const center = this.bounds.getCenter(new THREE.Vector3())
    let toH = this.halfHeight
    if (fit) {
      // Fit the bounding box as seen from the new direction.
      const corners: THREE.Vector3[] = []
      for (const cx of [this.bounds.min.x, this.bounds.max.x])
        for (const cy of [this.bounds.min.y, this.bounds.max.y])
          for (const cz of [this.bounds.min.z, this.bounds.max.z])
            corners.push(new THREE.Vector3(cx, cy, cz).sub(center))
      let hw = 1
      let hh = 1
      for (const c of corners) {
        hw = Math.max(hw, Math.abs(c.dot(x)))
        hh = Math.max(hh, Math.abs(c.dot(y)))
      }
      const aspect = (this.container.clientWidth || 1) / (this.container.clientHeight || 1)
      toH = Math.max(hh, hw / aspect) * 1.25
    }
    this.anim = {
      from: this.camera.quaternion.clone(),
      to,
      fromTarget: this.target.clone(),
      toTarget: fit ? center : this.target.clone(),
      fromH: this.halfHeight / this.camera.zoom,
      toH,
      t0: performance.now(),
    }
    this.camera.zoom = 1
    this.requestRender()
  }

  view(name: ViewName, plane?: PlaneData): void {
    const V = (x: number, y: number, z: number) => new THREE.Vector3(x, y, z)
    switch (name) {
      case 'front':
        return this.orient(V(0, 0, 1), V(0, 1, 0))
      case 'back':
        return this.orient(V(0, 0, -1), V(0, 1, 0))
      case 'left':
        return this.orient(V(-1, 0, 0), V(0, 1, 0))
      case 'right':
        return this.orient(V(1, 0, 0), V(0, 1, 0))
      case 'top':
        return this.orient(V(0, 1, 0), V(0, 0, -1))
      case 'bottom':
        return this.orient(V(0, -1, 0), V(0, 0, 1))
      case 'iso':
        return this.orient(V(1, 1, 1), V(0, 1, 0))
      case 'normal':
        if (plane) {
          const n = v3(plane.normal)
          const x = v3(plane.x_dir)
          return this.orient(n, new THREE.Vector3().crossVectors(n, x))
        }
        return
      case 'fit':
        return this.orient(
          new THREE.Vector3(0, 0, 1).applyQuaternion(this.camera.quaternion),
          this.camera.up.clone(),
        )
    }
  }
}

function setOpacity(o: THREE.Object3D, opacity: number): void {
  o.traverse((x) => {
    const m = (x as THREE.Mesh).material as THREE.Material | undefined
    if (m && 'opacity' in m && m.transparent) m.opacity = opacity
  })
}

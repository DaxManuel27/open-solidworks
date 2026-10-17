import { useEffect, useRef, useState } from 'react'
import * as THREE from 'three'
import { call } from '../core'
import type { AsmScene, PlaneData, Ref, Scene, SketchView, Transform } from '../protocol'
import { type AsmPick, type Pick, type PickKind, useStore } from '../store'
import { formatAngle, formatLength, parseAngle, parseLength } from '../units'
import { AssemblyView, type TriadHandle } from './assembly-view'
import { Engine } from './engine'
import { instantEdit } from './instant3d'
import { useMove } from './move-state'
import { type Label, Sketcher } from './sketcher'
import { SpatialSketcher } from './spatial-sketcher'
import { useSpatial } from './spatial-state'

interface Prompt {
  x: number
  y: number
  text: string
  angle: boolean
  resolve: (v: number | null) => void
}

const DEFAULT_KINDS: PickKind[] = ['vertex', 'edge', 'face', 'datum', 'std_plane']

/** The part-scene shell that carries an assembly's own planes and axes. */
function referenceScene(a: AsmScene): Scene {
  return {
    cosmetic_threads: [],
    sketches3d: [],
    curves: [],
    bodies: [],
    body: null,
    bbox: a.bbox,
    datums: a.datums,
    sketches: [],
  }
}

function transformOf(m: THREE.Matrix4): Transform {
  const t = new THREE.Vector3()
  const q = new THREE.Quaternion()
  m.decompose(t, q, new THREE.Vector3())
  return { translation: [t.x, t.y, t.z], rotation: [q.x, q.y, q.z, q.w] }
}

/** The component a drag or triad acts on: the top-level key, or a flexible child. */
function dragKey(key: string[]): string[] {
  const a = useStore.getState().assembly
  const top = a?.components.find((c) => c.key[0] === key[0])
  return top?.flexible && key.length > 1 ? key.slice(0, 2) : key.slice(0, 1)
}

function isFixed(key: string[]): boolean {
  const a = useStore.getState().assembly
  return a?.components.find((c) => c.key[0] === key[0])?.fixed ?? true
}

interface ComponentDrag {
  key: string[]
  kind: 'free' | 'axis' | 'rotate' | 'triad_move' | 'triad_rotate'
  start: THREE.Matrix4
  /** Grab point: world and component frame. */
  point: THREE.Vector3
  local: THREE.Vector3
  axis?: THREE.Vector3
  /** Screen position at start, for rotation. */
  sx: number
  sy: number
  /** Axis parameter or angle at start. */
  t0: number
  moved: boolean
}

export function Viewport() {
  const host = useRef<HTMLDivElement>(null)
  const engineRef = useRef<Engine | null>(null)
  const sketcherRef = useRef<Sketcher | null>(null)
  const spatialRef = useRef<SpatialSketcher | null>(null)
  const [labels, setLabels] = useState<Label[]>([])
  const [prompt, setPrompt] = useState<Prompt | null>(null)
  const [hover, setHover] = useState<Pick | null>(null)

  const scene = useStore((s) => s.scene)
  const preview = useStore((s) => s.previewScene)
  const selection = useStore((s) => s.selection)
  const highlights = useStore((s) => s.highlights)
  const sketch = useStore((s) => s.sketch)
  const sketchTool = useStore((s) => s.sketchTool)
  const sketchSelection = useStore((s) => s.sketchSelection)
  const view = useStore((s) => s.view)
  const units = useStore((s) => s.snapshot?.units)
  const pickTarget = useStore((s) => s.pickTarget)
  const hoverFeature = useStore((s) => s.hoverFeature)
  const asmScene = useStore((s) => s.asmScene)
  const asmPreview = useStore((s) => s.asmPreview)
  const asmSelection = useStore((s) => s.asmSelection)
  const asmComponents = useStore((s) => s.asmComponents)
  const asmMarks = useStore((s) => s.asmMarks)
  const assemblyComponents = useStore((s) => s.assembly?.components)
  const asmRef = useRef<AssemblyView | null>(null)
  const [asmHover, setAsmHover] = useState<AsmPick | null>(null)
  const analysis = useStore((s) => s.analysis)
  const facets = useStore((s) => s.facets)
  const instant3d = useStore((s) => s.instant3d)

  // Create the engine and sketcher once.
  useEffect(() => {
    const el = host.current!
    const engine = new Engine(el)
    // Clicking the canvas restores sketch shortcuts after keyboard toolbar navigation.
    engine.canvas.tabIndex = -1
    engineRef.current = engine
    const askValue = (x: number, y: number, initial: number, angle: boolean) =>
      new Promise<number | null>((resolve) => {
        const u = useStore.getState().snapshot!.units
        const text = angle
          ? formatAngle(Math.abs(initial), u, false)
          : formatLength(initial, u, false)
        const r = el.getBoundingClientRect()
        setPrompt({ x: x - r.left, y: y - r.top, text, angle, resolve })
      })
    const sketcher = new Sketcher(engine, {
      edit: (op) => useStore.getState().sketchEdit(op),
      drag: (point, to) => void useStore.getState().sketchDrag(point, to),
      selection: () => useStore.getState().sketchSelection,
      select: (ids) => useStore.getState().setSketchSelection(ids),
      status: (t) => useStore.getState().setStatus(t),
      askValue,
      convertAt: (x, y) => {
        // A picked face converts all of its boundary loops; edges win near a boundary.
        const p = engine.pick(x, y, ['edge', 'face'])
        if (p?.kind !== 'edge' && p?.kind !== 'face') return
        void call<{ ref: Ref }>('select.ref', { kind: p.kind, index: p.index }).then((r) =>
          useStore.getState().sketchEdit({ op: 'convert', refs: [r.ref] }),
        )
      },
      filletRadius: () => useStore.getState().sketchFilletRadius,
      polygonSides: () => useStore.getState().polygonSides,
    })
    sketcherRef.current = sketcher
    const spatial = new SpatialSketcher(engine, () => engine.onCamera())
    spatialRef.current = spatial
    asmRef.current = new AssemblyView(engine)
    let labelFrame = 0
    engine.onCamera = () => {
      if (labelFrame) return
      labelFrame = requestAnimationFrame(() => {
        labelFrame = 0
        setLabels(spatial.active ? spatial.labels() : sketcher.active ? sketcher.labels() : [])
      })
    }
    return () => {
      sketcher.dispose()
      spatial.dispose()
      engine.dispose()
    }
  }, [])

  useEffect(() => {
    if (asmScene) return
    engineRef.current?.setScene(scene)
  }, [scene, asmScene])

  // Assemblies: the reference planes go through the part scene shell, the
  // components through the assembly view.
  useEffect(() => {
    const engine = engineRef.current
    const view = asmRef.current
    if (!engine || !view) return
    if (!asmScene) {
      view.clear()
      return
    }
    engine.setScene(referenceScene(asmScene))
    engine.setBounds(asmScene.bbox?.min ?? null, asmScene.bbox?.max ?? null)
    view.setScene(asmScene, null)
  }, [asmScene])

  useEffect(() => {
    if (asmPreview) asmRef.current?.setPositions(asmPreview)
    else if (asmScene) asmRef.current?.setPositions(asmScene.instances)
  }, [asmPreview, asmScene])

  useEffect(() => {
    asmRef.current?.setHighlights({
      hover: asmHover,
      selected: asmSelection.flatMap((s) => (s.pick ? [s.pick] : [])),
      components: asmComponents,
      marks: asmMarks,
    })
  }, [asmHover, asmSelection, asmComponents, asmMarks])

  // A triad on the one selected floating component.
  useEffect(() => {
    const view = asmRef.current
    if (!view) return
    const key = asmComponents.length === 1 ? asmComponents[0]! : null
    const frame = key && !isFixed(key) ? view.matrixOf(key) : null
    view.setTriad(frame)
  }, [asmComponents, asmScene, asmPreview, assemblyComponents])

  useEffect(() => {
    engineRef.current?.setPreview(preview)
  }, [preview])

  useEffect(() => {
    engineRef.current?.setAnalysis(analysis)
  }, [analysis, scene])

  useEffect(() => {
    engineRef.current?.setFacets(facets)
  }, [facets, scene])

  // Instant3D: an arrow on the one selected face.
  useEffect(() => {
    const e = engineRef.current
    if (!e) return
    const only = selection.length === 1 ? selection[0]! : null
    const frame =
      instant3d && only && only.pick.kind === 'face' && !sketch
        ? e.faceFrame(only.pick.index)
        : null
    e.setHandle(frame ? { origin: frame.center, dir: frame.normal } : null)
  }, [instant3d, selection, scene, sketch])

  useEffect(() => {
    const e = engineRef.current
    if (!e) return
    e.setHighlights({ hover, selected: selection.map((s) => s.pick), aux: highlights })
  }, [hover, selection, highlights])

  useEffect(() => {
    const e = engineRef.current
    if (!e) return
    const wantsPlane =
      (pickTarget?.kinds.includes('std_plane') ?? false) ||
      (useStore.getState().asmTarget?.kinds.includes('face') ?? false)
    const empty = asmScene ? asmScene.instances.length === 0 : !scene?.body && !sketch
    e.setStdPlanesVisible(wantsPlane || empty || (hoverFeature !== null && hoverFeature < 0))
  }, [pickTarget, scene, sketch, hoverFeature, asmScene])

  useEffect(() => {
    if (units) sketcherRef.current?.setView(sketch, units)
    engineRef.current?.onCamera()
  }, [sketch, units, sketchSelection])

  useEffect(() => {
    sketcherRef.current?.setTool(sketchTool)
  }, [sketchTool])

  // View commands.
  useEffect(() => {
    const e = engineRef.current
    if (!e || view.seq === 0) return
    if (view.name !== 'normal') {
      e.view(view.name)
      return
    }
    const s = useStore.getState()
    if (s.sketch) {
      e.view('normal', s.sketch.plane)
      return
    }
    const sel = s.selection[0]?.ref
    if (sel) {
      void call<PlaneData | null>('select.plane', { ref: sel }).then((p) => {
        if (p) e.view('normal', p)
      })
    }
  }, [view])

  // Pointer handling: navigation, picking and sketch input.
  useEffect(() => {
    const engine = engineRef.current!
    const sketcher = sketcherRef.current!
    const spatial = spatialRef.current!
    const canvas = engine.canvas
    let nav: { mode: 'rotate' | 'pan'; x: number; y: number; pivot?: THREE.Vector3 } | null = null
    let down: { x: number; y: number; button: number } | null = null
    let sketchOwns = false
    let spatialOwns = false
    let hoverFrame = 0
    let drag: ComponentDrag | null = null
    let painting = false
    let handle: {
      origin: THREE.Vector3
      dir: THREE.Vector3
      t0: number
      ref: Ref
      last: number
    } | null = null
    const assemblyOpen = () => !!useStore.getState().asmScene
    const paint = (x: number, y: number) => {
      const s = useStore.getState()
      const f = s.facets
      if (!f) return
      const hit = engine.pickFacet(x, y)
      if (!hit || hit.body !== f.body) return
      const near = engine.trianglesNear(f.body, hit.point, f.radius)
      const set = new Set(f.triangles)
      let grew = false
      for (const t of [hit.triangle, ...near])
        if (!set.has(t)) {
          set.add(t)
          grew = true
        }
      if (grew) s.setFacets({ ...f, triangles: [...set] })
    }

    /** Start dragging a component (or a triad handle) at the cursor. */
    const startDrag = (e: PointerEvent): boolean => {
      const view = asmRef.current
      if (!view || !assemblyOpen() || e.button !== 0) return false
      const handle: TriadHandle | null = view.pickTriad(e.clientX, e.clientY)
      const move = useMove.getState()
      if (handle && view.triadFrame) {
        const key = useStore.getState().asmComponents[0]!
        const frame = view.triadFrame
        const origin = new THREE.Vector3().setFromMatrixPosition(frame)
        const axes = [new THREE.Vector3(), new THREE.Vector3(), new THREE.Vector3()]
        frame.extractBasis(axes[0]!, axes[1]!, axes[2]!)
        const axis = axes[handle.axis]!.normalize()
        const kind = handle.type === 'move' ? 'triad_move' : 'triad_rotate'
        drag = {
          key: dragKey(key),
          kind,
          start: frame.clone(),
          point: origin,
          local: new THREE.Vector3(),
          axis,
          sx: e.clientX,
          sy: e.clientY,
          t0: 0,
          moved: false,
        }
        drag.t0 = kind === 'triad_move' ? axisParam(drag, e) : ringAngle(drag, e)
        return true
      }
      if (!(e.altKey || move.active)) return false
      const hit = view.hit(e.clientX, e.clientY)
      if (!hit) return false
      const key = dragKey(hit.placed.key)
      if (isFixed(key)) {
        useStore.getState().toast('That component is fixed; float it to move it', 'info')
        return false
      }
      const start = view.matrixOf(key) ?? hit.placed.matrix.clone()
      const axisIndex = { x: 0, y: 1, z: 2 } as const
      const kind = move.mode === 'rotate' ? 'rotate' : move.mode === 'free' ? 'free' : 'axis'
      drag = {
        key,
        kind,
        start,
        point: hit.point.clone(),
        local: hit.point.clone().applyMatrix4(start.clone().invert()),
        axis:
          move.mode === 'x' || move.mode === 'y' || move.mode === 'z'
            ? new THREE.Vector3().setComponent(axisIndex[move.mode], 1)
            : undefined,
        sx: e.clientX,
        sy: e.clientY,
        t0: 0,
        moved: false,
      }
      return true
    }

    /** Closest parameter along the drag axis to the cursor ray. */
    const axisParam = (d: ComponentDrag, e: PointerEvent): number => {
      const ray = engine.ray(e.clientX, e.clientY)
      const w = d.point.clone().sub(ray.origin)
      const a = d.axis!
      const b = ray.direction
      const ab = a.dot(b)
      const den = 1 - ab * ab
      if (Math.abs(den) < 1e-9) return 0
      return (ab * w.dot(b) - w.dot(a)) / den
    }

    /** Cursor angle about the drag axis, in the plane through the origin. */
    const ringAngle = (d: ComponentDrag, e: PointerEvent): number => {
      const ray = engine.ray(e.clientX, e.clientY)
      const n = d.axis!
      const denom = ray.direction.dot(n)
      if (Math.abs(denom) < 1e-9) return d.t0
      const t = d.point.clone().sub(ray.origin).dot(n) / denom
      const p = ray.origin.clone().addScaledVector(ray.direction, t).sub(d.point)
      const u = new THREE.Vector3(1, 0, 0)
      if (Math.abs(u.dot(n)) > 0.9) u.set(0, 1, 0)
      u.sub(n.clone().multiplyScalar(u.dot(n))).normalize()
      const v = new THREE.Vector3().crossVectors(n, u)
      return Math.atan2(p.dot(v), p.dot(u))
    }

    /** The motion request for the current cursor position. */
    const dragMotion = (d: ComponentDrag, e: PointerEvent): Record<string, unknown> => {
      const rotate = (axis: THREE.Vector3, angle: number, about: THREE.Vector3) => {
        const r = new THREE.Matrix4().makeRotationAxis(axis, angle)
        const m = new THREE.Matrix4()
          .makeTranslation(about.x, about.y, about.z)
          .multiply(r)
          .multiply(new THREE.Matrix4().makeTranslation(-about.x, -about.y, -about.z))
          .multiply(d.start)
        return { mode: 'pose', transform: transformOf(m) }
      }
      switch (d.kind) {
        case 'free': {
          const ray = engine.ray(e.clientX, e.clientY)
          const n = engine.camera.getWorldDirection(new THREE.Vector3())
          const t = d.point.clone().sub(ray.origin).dot(n) / ray.direction.dot(n)
          const p = ray.origin.clone().addScaledVector(ray.direction, t)
          return { mode: 'drag', local: d.local.toArray(), point: p.toArray() }
        }
        case 'axis':
        case 'triad_move': {
          const delta = axisParam(d, e) - d.t0
          const m = new THREE.Matrix4()
            .makeTranslation(d.axis!.x * delta, d.axis!.y * delta, d.axis!.z * delta)
            .multiply(d.start)
          return { mode: 'pose', transform: transformOf(m) }
        }
        case 'triad_rotate':
          return rotate(d.axis!, ringAngle(d, e) - d.t0, d.point)
        case 'rotate': {
          // Horizontal motion turns about the view's up axis, vertical about its right.
          const q = engine.camera.quaternion
          const up = new THREE.Vector3(0, 1, 0).applyQuaternion(q)
          const right = new THREE.Vector3(1, 0, 0).applyQuaternion(q)
          const origin = new THREE.Vector3().setFromMatrixPosition(d.start)
          const dx = (e.clientX - d.sx) * 0.01
          const dy = (e.clientY - d.sy) * 0.01
          const m = new THREE.Matrix4()
            .makeTranslation(origin.x, origin.y, origin.z)
            .multiply(new THREE.Matrix4().makeRotationAxis(up, dx))
            .multiply(new THREE.Matrix4().makeRotationAxis(right, dy))
            .multiply(new THREE.Matrix4().makeTranslation(-origin.x, -origin.y, -origin.z))
            .multiply(d.start)
          return { mode: 'pose', transform: transformOf(m) }
        }
      }
    }

    const collision = () => {
      const m = useMove.getState()
      return m.collision ? { dragged_only: m.draggedOnly } : null
    }

    const onDown = (e: PointerEvent) => {
      canvas.setPointerCapture(e.pointerId)
      down = { x: e.clientX, y: e.clientY, button: e.button }
      sketchOwns = false
      const st = useStore.getState()
      if (e.button === 0 && st.facets?.painting && engine.pickFacet(e.clientX, e.clientY)) {
        painting = true
        paint(e.clientX, e.clientY)
        return
      }
      if (e.button === 0 && st.instant3d && engine.onHandle(e.clientX, e.clientY)) {
        const sel = st.selection[0]
        const frame = sel && sel.pick.kind === 'face' ? engine.faceFrame(sel.pick.index) : null
        if (sel && frame) {
          const t0 = engine.along(frame.center, frame.normal, e.clientX, e.clientY)
          handle = { origin: frame.center, dir: frame.normal, t0, ref: sel.ref, last: 0 }
          return
        }
      }
      if (startDrag(e)) return
      spatialOwns = spatial.onDown(e)
      if (spatialOwns) return
      if (e.button === 0 && sketcher.active && !e.altKey) {
        sketchOwns = sketcher.onPointerDown(e)
        if (sketchOwns) return
      }
      const pan = e.button === 2 || e.shiftKey
      const hit = engine.hitFace(e.clientX, e.clientY)
      nav = { mode: pan ? 'pan' : 'rotate', x: e.clientX, y: e.clientY, pivot: hit?.point }
    }

    const onMove = (e: PointerEvent) => {
      if (painting) {
        paint(e.clientX, e.clientY)
        return
      }
      if (handle) {
        const delta = engine.along(handle.origin, handle.dir, e.clientX, e.clientY) - handle.t0
        const step = Math.round(delta * 10) / 10
        if (step !== handle.last) {
          handle.last = step
          const edit = instantEdit(handle.ref, step)
          void useStore.getState().preview(edit?.params ?? null, edit?.editing)
          useStore.getState().setStatus(`${step.toFixed(1)} mm`)
        }
        return
      }
      if (drag) {
        if (!drag.moved && Math.hypot(e.clientX - drag.sx, e.clientY - drag.sy) < 3) return
        const s = useStore.getState()
        if (!drag.moved) {
          drag.moved = true
          void s.asmMove(drag.key, dragMotion(drag, e), 'start', collision())
          return
        }
        void s.asmMove(drag.key, dragMotion(drag, e), 'move', collision())
        return
      }
      if (spatialOwns) {
        spatial.onMove(e)
        return
      }
      if (sketchOwns || (sketcher.active && !down)) sketcher.onPointerMove(e)
      if (nav && down) {
        const moved = Math.hypot(e.clientX - down.x, e.clientY - down.y) > 3
        if (!moved) return
        const dx = e.clientX - nav.x
        const dy = e.clientY - nav.y
        nav.x = e.clientX
        nav.y = e.clientY
        if (nav.mode === 'rotate' && !sketcher.active) engine.rotate(dx, dy, nav.pivot)
        else if (nav.mode === 'rotate' && sketcher.active && down.button === 1)
          engine.rotate(dx, dy, nav.pivot)
        else engine.pan(dx, dy)
        return
      }
      if (sketcher.active && useStore.getState().sketchTool !== 'convert') return
      if (hoverFrame) return
      const x = e.clientX
      const y = e.clientY
      hoverFrame = requestAnimationFrame(() => {
        hoverFrame = 0
        if (assemblyOpen()) {
          const target = useStore.getState().asmTarget
          const kinds = (target?.kinds.filter((k) => k !== 'component') ?? [
            'vertex',
            'edge',
            'face',
          ]) as ('vertex' | 'edge' | 'face')[]
          const p = asmRef.current?.pick(x, y, kinds.length ? kinds : ['face']) ?? null
          setAsmHover((h) => (JSON.stringify(h) === JSON.stringify(p) ? h : p))
          return
        }
        const kinds = sketcher.active
          ? (['edge'] as PickKind[])
          : (useStore.getState().pickTarget?.kinds ?? DEFAULT_KINDS)
        const p = engine.pick(x, y, kinds)
        setHover((h) => (JSON.stringify(h) === JSON.stringify(p) ? h : p))
      })
    }

    const onUp = (e: PointerEvent) => {
      canvas.releasePointerCapture(e.pointerId)
      const wasDown = down
      down = null
      nav = null
      if (painting) {
        painting = false
        return
      }
      if (handle) {
        const h = handle
        handle = null
        const s = useStore.getState()
        void s.preview(null)
        const edit = instantEdit(h.ref, h.last)
        if (edit) void s.commit(edit.params, edit.editing)
        return
      }
      if (drag) {
        const d = drag
        drag = null
        if (d.moved) {
          void useStore.getState().asmMove(d.key, dragMotion(d, e), 'end', collision())
          return
        }
      }
      if (spatialOwns) {
        spatialOwns = false
        spatial.onUp()
        return
      }
      if (sketchOwns) {
        sketchOwns = false
        void sketcher.onPointerUp(e)
        return
      }
      if (!wasDown || Math.hypot(e.clientX - wasDown.x, e.clientY - wasDown.y) > 3) return
      if (wasDown.button !== 0) return
      if (sketcher.active) {
        if (useStore.getState().sketchTool === 'convert') void sketcher.onPointerUp(e)
        return
      }
      const additive = e.shiftKey || e.metaKey || e.ctrlKey
      if (assemblyOpen()) {
        const s = useStore.getState()
        const target = s.asmTarget
        const kinds = (target?.kinds.filter((k) => k !== 'component') ?? [
          'vertex',
          'edge',
          'face',
        ]) as ('vertex' | 'edge' | 'face')[]
        const p = asmRef.current?.pick(e.clientX, e.clientY, kinds.length ? kinds : ['face'])
        if (p) {
          void s.asmPick(p, additive)
          return
        }
        // Assembly planes and axes.
        const ref = engine.pick(e.clientX, e.clientY, ['datum', 'std_plane'])
        if (ref && target && target.kinds.includes('face')) {
          const entity: Ref =
            ref.kind === 'std_plane'
              ? ref.ref
              : ref.kind === 'datum'
                ? { type: 'feature', id: ref.id }
                : { type: 'origin' }
          target.onPick({ pick: null, ref: { component: [], entity }, geometry: null })
          return
        }
        if (!target) s.selectComponents([])
        return
      }
      const kinds = useStore.getState().pickTarget?.kinds ?? DEFAULT_KINDS
      const p = engine.pick(e.clientX, e.clientY, kinds)
      if (p) void useStore.getState().pick(p, additive)
      else if (!useStore.getState().pickTarget) useStore.getState().clearSelection()
    }

    const onWheel = (e: WheelEvent) => {
      e.preventDefault()
      const factor = Math.exp(
        (e.ctrlKey ? e.deltaY * 0.01 : e.deltaY * 0.0015) * (e.deltaMode === 1 ? 20 : 1),
      )
      engine.zoom(factor, e.clientX, e.clientY)
      if (sketcher.active) sketcher.redraw()
    }

    const onDbl = () => void sketcher.onDoubleClick()
    const onContext = (e: MouseEvent) => {
      e.preventDefault()
      if (sketcher.active) sketcher.cancel()
    }

    canvas.addEventListener('pointerdown', onDown)
    canvas.addEventListener('pointermove', onMove)
    canvas.addEventListener('pointerup', onUp)
    canvas.addEventListener('wheel', onWheel, { passive: false })
    canvas.addEventListener('dblclick', onDbl)
    canvas.addEventListener('contextmenu', onContext)
    const onKey = (e: KeyboardEvent) => {
      if (['INPUT', 'TEXTAREA', 'SELECT'].includes((e.target as HTMLElement).tagName)) return
      if (spatial.onKey(e)) return
      void sketcher.onKey(e).then((handled) => {
        if (handled) return
        if (e.key === 'Escape') {
          const s = useStore.getState()
          if (s.sketch && s.sketchTool !== 'select') s.setSketchTool('select')
          else if (!s.sketch) s.clearSelection()
        }
      })
    }
    window.addEventListener('keydown', onKey)
    return () => {
      canvas.removeEventListener('pointerdown', onDown)
      canvas.removeEventListener('pointermove', onMove)
      canvas.removeEventListener('pointerup', onUp)
      canvas.removeEventListener('wheel', onWheel)
      canvas.removeEventListener('dblclick', onDbl)
      canvas.removeEventListener('contextmenu', onContext)
      window.removeEventListener('keydown', onKey)
    }
  }, [])

  const editDimension = (l: Label) => {
    if (l.kind !== 'dim') return
    const spatial = useSpatial.getState()
    if (spatial.sketch) {
      const c = spatial.sketch.constraints.find((c) => c.id === l.id)
      if (c?.kind !== 'distance') return
      const text = formatLength(c.value, useStore.getState().snapshot!.units, false)
      new Promise<number | null>((resolve) =>
        setPrompt({ x: l.x, y: l.y, text, angle: false, resolve }),
      ).then((value) => {
        const state = useSpatial.getState()
        if (value !== null && state.sketch)
          void state.change({
            ...state.sketch,
            constraints: state.sketch.constraints.map((x) => (x.id === c.id ? { ...c, value } : x)),
          })
      })
      return
    }
    const s = useStore.getState()
    const c = s.sketch?.constraints.find((x) => x.id === l.id)
    if (!c || c.value === undefined) return
    const u = s.snapshot!.units
    const angle = c.type === 'angle'
    const text = angle
      ? formatAngle(Math.abs(c.value), u, false)
      : formatLength(Math.abs(c.value), u, false)
    new Promise<number | null>((resolve) =>
      setPrompt({ x: l.x, y: l.y, text, angle, resolve }),
    ).then((v) => {
      if (v !== null) void s.sketchEdit({ op: 'set_value', id: c.id, value: v })
    })
  }

  const toggleLabel = (l: Label, additive: boolean) => {
    const spatial = useSpatial.getState()
    if (spatial.sketch) {
      useSpatial.setState({
        selection: additive ? [...new Set([...spatial.selection, l.id])] : [l.id],
      })
      return
    }
    const s = useStore.getState()
    const cur = s.sketchSelection
    if (additive)
      s.setSketchSelection(cur.includes(l.id) ? cur.filter((x) => x !== l.id) : [...cur, l.id])
    else s.setSketchSelection([l.id])
  }

  return (
    <div className="viewport" ref={host}>
      {labels.map((l) => (
        <div
          key={l.id}
          className={`label ${l.kind} ${l.status} ${l.selected ? 'selected' : ''} ${l.driving ? '' : 'driven'}`}
          style={{ transform: `translate(${l.x}px, ${l.y}px) translate(-50%, -50%)` }}
          onClick={(e) => toggleLabel(l, e.shiftKey || e.metaKey || e.ctrlKey)}
          onDoubleClick={() => editDimension(l)}
          title={
            l.kind === 'dim'
              ? 'Double-click to edit'
              : 'Relation — click to select, Delete to remove'
          }
        >
          {l.text}
        </div>
      ))}
      {prompt && <ValuePrompt prompt={prompt} onDone={() => setPrompt(null)} sketch={sketch} />}
    </div>
  )
}

function ValuePrompt({
  prompt,
  onDone,
}: {
  prompt: Prompt
  onDone: () => void
  sketch: SketchView | null
}) {
  const [text, setText] = useState(prompt.text)
  const [bad, setBad] = useState(false)
  const units = useStore((s) => s.snapshot!.units)
  const finish = (ok: boolean) => {
    if (!ok) {
      prompt.resolve(null)
      onDone()
      return
    }
    const v = prompt.angle ? parseAngle(text, units) : parseLength(text, units)
    if (v === null || (!prompt.angle && v <= 0)) {
      setBad(true)
      return
    }
    prompt.resolve(v)
    onDone()
  }
  return (
    <div className="value-prompt" style={{ left: prompt.x + 12, top: prompt.y - 14 }}>
      <input
        autoFocus
        className={bad ? 'bad' : ''}
        value={text}
        onFocus={(e) => e.currentTarget.select()}
        onChange={(e) => {
          setText(e.target.value)
          setBad(false)
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter') finish(true)
          if (e.key === 'Escape') finish(false)
          e.stopPropagation()
        }}
        onBlur={() => finish(false)}
      />
      <button onMouseDown={(e) => e.preventDefault()} onClick={() => finish(true)} title="OK">
        ✓
      </button>
    </div>
  )
}

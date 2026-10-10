import { useEffect, useRef, useState } from 'react'
import * as THREE from 'three'
import { call } from '../core'
import type { PlaneData, Ref, SketchView } from '../protocol'
import { type Pick, type PickKind, useStore } from '../store'
import { formatAngle, formatLength, parseAngle, parseLength } from '../units'
import { Engine } from './engine'
import { type Label, Sketcher } from './sketcher'

interface Prompt {
  x: number
  y: number
  text: string
  angle: boolean
  resolve: (v: number | null) => void
}

const DEFAULT_KINDS: PickKind[] = ['vertex', 'edge', 'face', 'datum', 'std_plane']

export function Viewport() {
  const host = useRef<HTMLDivElement>(null)
  const engineRef = useRef<Engine | null>(null)
  const sketcherRef = useRef<Sketcher | null>(null)
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
        const p = engine.pick(x, y, ['edge'])
        if (p?.kind !== 'edge') return
        void call<{ ref: Ref }>('select.ref', { kind: 'edge', index: p.index }).then((r) =>
          useStore.getState().sketchEdit({ op: 'convert', refs: [r.ref] }),
        )
      },
      filletRadius: () => useStore.getState().sketchFilletRadius,
      polygonSides: () => useStore.getState().polygonSides,
    })
    sketcherRef.current = sketcher
    let labelFrame = 0
    engine.onCamera = () => {
      if (labelFrame) return
      labelFrame = requestAnimationFrame(() => {
        labelFrame = 0
        setLabels(sketcher.active ? sketcher.labels() : [])
      })
    }
    return () => {
      sketcher.dispose()
      engine.dispose()
    }
  }, [])

  useEffect(() => {
    engineRef.current?.setScene(scene)
  }, [scene])

  useEffect(() => {
    engineRef.current?.setPreview(preview)
  }, [preview])

  useEffect(() => {
    const e = engineRef.current
    if (!e) return
    e.setHighlights({ hover, selected: selection.map((s) => s.pick), aux: highlights })
  }, [hover, selection, highlights])

  useEffect(() => {
    const e = engineRef.current
    if (!e) return
    const wantsPlane = pickTarget?.kinds.includes('std_plane') ?? false
    const empty = !scene?.body && !sketch
    e.setStdPlanesVisible(wantsPlane || empty || (hoverFeature !== null && hoverFeature < 0))
  }, [pickTarget, scene, sketch, hoverFeature])

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
    const canvas = engine.canvas
    let nav: { mode: 'rotate' | 'pan'; x: number; y: number; pivot?: THREE.Vector3 } | null = null
    let down: { x: number; y: number; button: number } | null = null
    let sketchOwns = false
    let hoverFrame = 0

    const onDown = (e: PointerEvent) => {
      canvas.setPointerCapture(e.pointerId)
      down = { x: e.clientX, y: e.clientY, button: e.button }
      sketchOwns = false
      if (e.button === 0 && sketcher.active && !e.altKey) {
        sketchOwns = sketcher.onPointerDown(e)
        if (sketchOwns) return
      }
      const pan = e.button === 2 || e.shiftKey
      const hit = engine.hitFace(e.clientX, e.clientY)
      nav = { mode: pan ? 'pan' : 'rotate', x: e.clientX, y: e.clientY, pivot: hit?.point }
    }

    const onMove = (e: PointerEvent) => {
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
      const kinds = useStore.getState().pickTarget?.kinds ?? DEFAULT_KINDS
      const p = engine.pick(e.clientX, e.clientY, kinds)
      if (p) void useStore.getState().pick(p, e.shiftKey || e.metaKey || e.ctrlKey)
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
      if ((e.target as HTMLElement).tagName === 'INPUT') return
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

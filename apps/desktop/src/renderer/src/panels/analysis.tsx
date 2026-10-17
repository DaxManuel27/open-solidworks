/**
 * Analysis panels (draft, undercut, deviation, body compare, projected
 * area), which colour the model without changing it, and the Phase 6 file
 * commands: save bodies, mirror part and assembly defeature.
 */
import { useEffect, useMemo, useState } from 'react'
import { call } from '../core'
import type { Ref } from '../protocol'
import { type AnalysisOverlay, type PickKind, useStore } from '../store'
import { formatLength } from '../units'
import { AngleField, Check, Choice, LengthField, PanelShell, RefBox, Section } from './fields'

const PLANE_KINDS: PickKind[] = ['face', 'datum', 'std_plane', 'edge']
const FACE_KINDS: PickKind[] = ['face']

function useClose() {
  const close = useStore((s) => s.closePanel)
  return () => void close()
}

/** Clear the overlay when an analysis panel closes. */
function useOverlay(): (a: AnalysisOverlay | null) => void {
  const set = useStore((s) => s.setAnalysis)
  useEffect(() => () => useStore.getState().setAnalysis(null), [])
  return set
}

function Legend(props: { overlay: AnalysisOverlay | null }) {
  if (!props.overlay) return null
  return (
    <div className="legend">
      {props.overlay.legend.map((l) => (
        <div key={l.label} className="legend-row">
          <span
            className="swatch"
            style={{ background: `#${l.color.toString(16).padStart(6, '0')}` }}
          />
          {l.label}: {l.count}
        </div>
      ))}
    </div>
  )
}

interface FaceClass {
  face: number
  ref: Ref | null
  class: string
}

function colour(
  faces: FaceClass[],
  colours: Record<string, [string, number]>,
  title: string,
): AnalysisOverlay {
  const counts = new Map<string, number>()
  const map: Record<number, number> = {}
  for (const f of faces) {
    const c = colours[f.class]
    if (!c) continue
    map[f.face] = c[1]
    counts.set(f.class, (counts.get(f.class) ?? 0) + 1)
  }
  return {
    title,
    faces: map,
    legend: Object.entries(colours)
      .filter(([k]) => counts.has(k))
      .map(([k, [label, color]]) => ({ label, color, count: counts.get(k)! })),
  }
}

const DRAFT: Record<string, [string, number]> = {
  positive: ['Positive draft', 0x16a34a],
  negative: ['Negative draft', 0xdc2626],
  requires_draft: ['Requires draft', 0xeab308],
  straddle: ['Straddle', 0x9333ea],
}

export function DraftAnalysisPanel() {
  const close = useClose()
  const setOverlay = useOverlay()
  const overlay = useStore((s) => s.analysis)
  const scene = useStore((s) => s.scene)
  const [pull, setPull] = useState<Ref[]>([{ type: 'front_plane' }])
  const [angle, setAngle] = useState((1 * Math.PI) / 180)
  const [reverse, setReverse] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [steep, setSteep] = useState(0)
  useEffect(() => {
    if (!pull[0]) return
    let live = true
    void call<{ faces: (FaceClass & { steep: boolean })[] }>('doc.analysis', {
      analysis: { type: 'draft', pull: pull[0], angle, reverse },
    })
      .then((r) => {
        if (!live) return
        setError(null)
        setSteep(r.faces.filter((f) => f.steep).length)
        setOverlay(colour(r.faces, DRAFT, 'Draft analysis'))
      })
      .catch((e: Error) => live && setError(e.message))
    return () => {
      live = false
    }
  }, [pull, angle, reverse, scene, setOverlay])
  return (
    <PanelShell title="Draft Analysis" onCancel={close} onOk={close} error={error}>
      <Section title="Analysis parameters">
        <RefBox
          label="Direction of pull"
          refs={pull}
          onChange={setPull}
          kinds={PLANE_KINDS}
          single
          active
          onActivate={() => {}}
        />
        <Check label="Reverse direction" value={reverse} onChange={setReverse} />
        <AngleField label="Draft angle" value={angle} onChange={setAngle} />
      </Section>
      <Legend overlay={overlay} />
      {steep > 0 && (
        <div className="field">{steep} faces are partly steeper than the draft angle.</div>
      )}
    </PanelShell>
  )
}

const UNDERCUT: Record<string, [string, number]> = {
  none: ['No undercut', 0x9ca3af],
  direction1: ['Direction 1 undercut', 0xdc2626],
  direction2: ['Direction 2 undercut', 0x2563eb],
  occluded: ['Occluded undercut', 0x9333ea],
  straddle: ['Straddle undercut', 0xea580c],
}

export function UndercutPanel() {
  const close = useClose()
  const setOverlay = useOverlay()
  const overlay = useStore((s) => s.analysis)
  const scene = useStore((s) => s.scene)
  const [pull, setPull] = useState<Ref[]>([{ type: 'front_plane' }])
  const [reverse, setReverse] = useState(false)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    if (!pull[0]) return
    let live = true
    void call<{ faces: FaceClass[] }>('doc.analysis', {
      analysis: { type: 'undercut', pull: pull[0], reverse },
    })
      .then((r) => {
        if (!live) return
        setError(null)
        setOverlay(colour(r.faces, UNDERCUT, 'Undercut analysis'))
      })
      .catch((e: Error) => live && setError(e.message))
    return () => {
      live = false
    }
  }, [pull, reverse, scene, setOverlay])
  return (
    <PanelShell title="Undercut Analysis" onCancel={close} onOk={close} error={error}>
      <Section title="Analysis parameters">
        <RefBox
          label="Direction of pull"
          refs={pull}
          onChange={setPull}
          kinds={PLANE_KINDS}
          single
          active
          onActivate={() => {}}
        />
        <Check label="Reverse direction" value={reverse} onChange={setReverse} />
      </Section>
      <Legend overlay={overlay} />
    </PanelShell>
  )
}

function BodyPick(props: {
  label: string
  value: string
  onChange: (v: string) => void
  kind?: string
  none?: string
}) {
  const bodies = useStore((s) => s.scene?.bodies ?? [])
  const list = bodies.filter((b) => !props.kind || (b.kind ?? 'solid') === props.kind)
  return (
    <Choice
      label={props.label}
      value={props.value}
      options={[
        { value: '', label: props.none ?? 'Choose a body…' },
        ...list.map((b) => ({ value: b.id, label: b.name })),
      ]}
      onChange={props.onChange}
    />
  )
}

/** Green within the tolerance, through yellow to red at twice it. */
function ramp(d: number, tolerance: number): number {
  const t = Math.min(1, Math.max(0, d / (2 * tolerance)))
  const r = Math.round(t < 0.5 ? 2 * t * 255 : 255)
  const g = Math.round(t < 0.5 ? 200 : (1 - (t - 0.5) * 2) * 200)
  return (r << 16) | (g << 8) | 40
}

export function DeviationPanel() {
  const close = useClose()
  const setOverlay = useOverlay()
  const units = useStore((s) => s.snapshot!.units)
  const scene = useStore((s) => s.scene)
  const meshes = useMemo(() => (scene?.bodies ?? []).filter((b) => b.kind === 'mesh'), [scene])
  const [mesh, setMesh] = useState(meshes[0]?.id ?? '')
  const [body, setBody] = useState('')
  const [tolerance, setTolerance] = useState(0.1)
  const [stats, setStats] = useState<{
    mean: number
    rms: number
    max: number
    within: { fraction: number }[]
  } | null>(null)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    if (!mesh) return
    let live = true
    void call<{
      mean: number
      rms: number
      max: number
      within: { fraction: number }[]
      distances: number[]
    }>('doc.analysis', {
      analysis: { type: 'deviation', mesh, body: body || null, limits: [tolerance] },
    })
      .then((r) => {
        if (!live) return
        setError(null)
        setStats(r)
        const ok = Math.round((r.within[0]?.fraction ?? 0) * r.distances.length)
        setOverlay({
          title: 'Deviation',
          faces: {},
          legend: [
            { label: `Within ${formatLength(tolerance, units)}`, color: 0x16a34a, count: ok },
            { label: 'Beyond', color: 0xdc2626, count: r.distances.length - ok },
          ],
          mesh: { body: mesh, colors: r.distances.map((d) => ramp(d, tolerance)) },
        })
      })
      .catch((e: Error) => live && setError(e.message))
    return () => {
      live = false
    }
  }, [mesh, body, tolerance, scene, units, setOverlay])
  return (
    <PanelShell title="Deviation Analysis" onCancel={close} onOk={close} error={error}>
      <Section title="Compare">
        <BodyPick label="Mesh body" value={mesh} onChange={setMesh} kind="mesh" />
        <BodyPick label="Against" value={body} onChange={setBody} none="Every solid and surface" />
        <LengthField label="Tolerance" value={tolerance} onChange={setTolerance} />
      </Section>
      {stats && (
        <Section title="Results">
          <div className="field">Mean {formatLength(stats.mean, units)}</div>
          <div className="field">RMS {formatLength(stats.rms, units)}</div>
          <div className="field">Largest {formatLength(stats.max, units)}</div>
          <div className="field">
            {((stats.within[0]?.fraction ?? 0) * 100).toFixed(1)} % within tolerance
          </div>
        </Section>
      )}
    </PanelShell>
  )
}

const COMPARE: Record<string, [string, number]> = {
  matching: ['Matching faces', 0x9ca3af],
  modified: ['Modified faces', 0xeab308],
  unique: ['Unique faces', 0xdc2626],
}

export function ComparePanel() {
  const close = useClose()
  const setOverlay = useOverlay()
  const overlay = useStore((s) => s.analysis)
  const units = useStore((s) => s.snapshot!.units)
  const scene = useStore((s) => s.scene)
  const [a, setA] = useState('')
  const [b, setB] = useState('')
  const [only, setOnly] = useState<{ a: number; b: number } | null>(null)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    if (!a || !b || a === b) return
    let live = true
    void call<{ a: FaceClass[]; b: FaceClass[]; only_a: number; only_b: number }>('doc.analysis', {
      analysis: { type: 'compare', a, b },
    })
      .then((r) => {
        if (!live) return
        setError(null)
        setOnly({ a: r.only_a, b: r.only_b })
        setOverlay(colour([...r.a, ...r.b], COMPARE, 'Body compare'))
      })
      .catch((e: Error) => live && setError(e.message))
    return () => {
      live = false
    }
  }, [a, b, scene, setOverlay])
  return (
    <PanelShell title="Body Compare" onCancel={close} onOk={close} error={error}>
      <Section title="Bodies">
        <BodyPick label="Body A" value={a} onChange={setA} />
        <BodyPick label="Body B" value={b} onChange={setB} />
      </Section>
      <Legend overlay={overlay} />
      {only && (
        <Section title="Volume">
          <div className="field">Only in A: {formatLength(only.a, units)}³</div>
          <div className="field">Only in B: {formatLength(only.b, units)}³</div>
        </Section>
      )}
    </PanelShell>
  )
}

export function ProjectedAreaPanel() {
  const close = useClose()
  const units = useStore((s) => s.snapshot!.units)
  const [faces, setFaces] = useState<Ref[]>([])
  const [plane, setPlane] = useState<Ref[]>([{ type: 'front_plane' }])
  const [hidden, setHidden] = useState(false)
  const [box, setBox] = useState<'faces' | 'plane'>('faces')
  const [area, setArea] = useState<{ area: number; error: number } | null>(null)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    if (!faces.length || !plane[0]) return
    let live = true
    void call<{ area: number; error: number }>('doc.analysis', {
      analysis: { type: 'projected_area', faces, plane: plane[0], hidden },
    })
      .then((r) => {
        if (live) {
          setError(null)
          setArea(r)
        }
      })
      .catch((e: Error) => live && setError(e.message))
    return () => {
      live = false
    }
  }, [faces, plane, hidden])
  return (
    <PanelShell title="Projected Area" onCancel={close} onOk={close} error={error}>
      <RefBox
        label="Faces"
        refs={faces}
        onChange={setFaces}
        kinds={FACE_KINDS}
        active={box === 'faces'}
        onActivate={() => setBox('faces')}
      />
      <RefBox
        label="Projection plane"
        refs={plane}
        onChange={setPlane}
        kinds={PLANE_KINDS}
        single
        active={box === 'plane'}
        onActivate={() => setBox('plane')}
      />
      <Check label="Count hidden faces (overlaps add up)" value={hidden} onChange={setHidden} />
      {area && faces.length > 0 && (
        <div className="field">
          Area: {formatLength(area.area, units)}²{' '}
          {area.error > 0 && `± ${formatLength(area.error, units)}²`}
        </div>
      )}
    </PanelShell>
  )
}

export function SaveBodiesPanel() {
  const close = useClose()
  const toast = useStore((s) => s.toast)
  const path = useStore((s) => s.snapshot?.path)
  const bodies = useStore((s) => s.scene?.bodies ?? []).filter((b) => b.kind !== 'mesh')
  const [chosen, setChosen] = useState<string[]>(bodies.map((b) => b.id))
  const [assembly, setAssembly] = useState(true)
  const go = async () => {
    if (!path) {
      toast('Save the part first: the new parts stay linked to it')
      return
    }
    const folder = await window.openforge.openDialog('folder')
    if (!folder) return
    try {
      const r = await call<{ parts: string[] }>('bodies.save', {
        folder,
        bodies: chosen,
        assembly: assembly ? 'Bodies' : null,
      })
      toast(`Saved ${r.parts.length} part${r.parts.length === 1 ? '' : 's'}`, 'info')
      close()
    } catch (e) {
      toast((e as Error).message)
    }
  }
  return (
    <PanelShell
      title="Save Bodies"
      onCancel={close}
      onOk={() => void go()}
      okDisabled={!chosen.length}
    >
      <Section title="Bodies">
        {bodies.map((b) => (
          <Check
            key={b.id}
            label={b.name}
            value={chosen.includes(b.id)}
            onChange={(on) => setChosen(on ? [...chosen, b.id] : chosen.filter((x) => x !== b.id))}
          />
        ))}
      </Section>
      <Check label="Create an assembly of them" value={assembly} onChange={setAssembly} />
    </PanelShell>
  )
}

export function MirrorPartPanel() {
  const close = useClose()
  const toast = useStore((s) => s.toast)
  const path = useStore((s) => s.snapshot?.path)
  const [plane, setPlane] = useState<Ref[]>([{ type: 'right_plane' }])
  const go = async () => {
    if (!path) {
      toast('Save the part first: the mirrored part stays linked to it')
      return
    }
    const base =
      path
        .split(/[\\/]/)
        .pop()
        ?.replace(/\.ofp$/i, '') ?? 'Part'
    const file = await window.openforge.saveDialog('part', `${base}-mirror.ofp`)
    if (!file) return
    try {
      await call('part.mirror', { path: file, plane: plane[0] ?? null })
      toast(`Wrote ${file.split(/[\\/]/).pop()}`, 'info')
      close()
    } catch (e) {
      toast((e as Error).message)
    }
  }
  return (
    <PanelShell title="Mirror Part" onCancel={close} onOk={() => void go()}>
      <RefBox
        label="Mirror plane"
        refs={plane}
        onChange={setPlane}
        kinds={PLANE_KINDS}
        single
        active
        onActivate={() => {}}
      />
    </PanelShell>
  )
}

export function AsmDefeaturePanel() {
  const close = useClose()
  const toast = useStore((s) => s.toast)
  const [mode, setMode] = useState<'bounding_box' | 'cylinder'>('bounding_box')
  const go = async () => {
    const file = await window.openforge.saveDialog('part', 'Defeatured.ofp')
    if (!file) return
    try {
      await call('asm.defeature', { path: file, mode })
      toast(`Wrote ${file.split(/[\\/]/).pop()}`, 'info')
      close()
    } catch (e) {
      toast((e as Error).message)
    }
  }
  return (
    <PanelShell title="Defeature" onCancel={close} onOk={() => void go()}>
      <Choice
        label="Replace each component by"
        value={mode}
        options={[
          { value: 'bounding_box', label: 'Its bounding box' },
          { value: 'cylinder', label: 'Its bounding cylinder' },
        ]}
        onChange={setMode}
      />
      <div className="panel-help">Writes a new part for sharing; the assembly is unchanged.</div>
    </PanelShell>
  )
}

import { useEffect, useMemo, useState } from 'react'
import { call } from '../core'
import type { BomRow, CompRef, HardwareEntry, MeshPayload } from '../protocol'
import { type Panel, useStore } from '../store'
import { formatLength } from '../units'
import { useMove } from '../viewport/move-state'
import { AngleField, Check, Choice, IntField, LengthField, PanelShell, Section } from './fields'
import {
  AsmRefBox,
  closeAsmPanel,
  commitAsm,
  componentIndex,
  componentName,
  type Picked,
} from './asm-common'

const ORIGIN: Picked['ref']['entity'] = { type: 'origin' }
const asComponents = (keys: string[][]): Picked[] =>
  keys.map((k) => ({ ref: { component: k.slice(0, 1), entity: ORIGIN }, geometry: null }))

/** Where to put a new component: beside what's already there. */
function besideAssembly():
  | { translation: [number, number, number]; rotation: [number, number, number, number] }
  | undefined {
  const s = useStore.getState()
  const box = s.asmScene?.bbox
  if (!box || !s.asmScene?.instances.length) return undefined
  return {
    translation: [box.max[0] + (box.max[0] - box.min[0]) * 0.25 + 10, box.min[1], box.min[2]],
    rotation: [0, 0, 0, 1],
  }
}

export function InsertComponentsPanel() {
  const [path, setPath] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  useEffect(() => {
    void (async () => {
      const p = await window.openforge.openDialog('component')
      if (p) setPath(p)
    })()
  }, [])
  const insert = async (close: boolean) => {
    if (!path) return
    setBusy(true)
    const ok = await commitAsm('asm.insert', { path, transform: besideAssembly() }, !close)
    setBusy(false)
    if (ok && !close) useStore.getState().toast('Inserted; insert another or close', 'info')
  }
  return (
    <PanelShell
      title="Insert Components"
      onOk={() => void insert(true)}
      onCancel={closeAsmPanel}
      okDisabled={!path || busy}
    >
      <Section title="Part/Assembly to Insert">
        <div className="hint">{path ?? 'Choose an .ofp part or .ofa assembly'}</div>
        <button
          onClick={async () => {
            const p = await window.openforge.openDialog('component')
            if (p) setPath(p)
          }}
        >
          Browse…
        </button>
      </Section>
      <Section title="Options">
        <div className="hint">
          The first component is fixed at the origin; others are placed beside the assembly. Drag
          them into place with Move Component or mate them.
        </div>
        <button disabled={!path || busy} onClick={() => void insert(false)}>
          Insert and keep open
        </button>
      </Section>
    </PanelShell>
  )
}

export function HardwarePanel() {
  const [catalog, setCatalog] = useState<HardwareEntry[]>([])
  const [kind, setKind] = useState<string>('socket_head_cap_screw')
  const [size, setSize] = useState<string>('M6')
  const [length, setLength] = useState<number>(20)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    void call<{ catalog: HardwareEntry[] }>('asm.hardware_catalog').then((r) =>
      setCatalog(r.catalog),
    )
  }, [])
  const entry = catalog.find((c) => c.kind === kind)
  // Sizes and lengths differ between standards: fall back to a typical one.
  const shownSize =
    entry && !entry.sizes.includes(size)
      ? (entry.sizes[Math.min(3, entry.sizes.length - 1)] ?? '')
      : size
  const shownLength =
    entry?.lengths.length && !entry.lengths.includes(length)
      ? (entry.lengths.find((l) => l >= 20) ?? entry.lengths[0]!)
      : length
  const units = useStore((s) => s.snapshot!.units)
  return (
    <PanelShell
      title="Hardware Library"
      onCancel={closeAsmPanel}
      okDisabled={!entry}
      error={error}
      onOk={async () => {
        const hardware = {
          kind,
          size: shownSize,
          ...(entry?.lengths.length ? { length: shownLength } : {}),
        }
        const ok = await commitAsm('asm.insert_hardware', { hardware, transform: besideAssembly() })
        if (!ok) setError('Could not insert that hardware')
      }}
    >
      <Section title="Standard part">
        <Choice
          label="Type"
          value={kind}
          options={catalog.map((c) => ({ value: c.kind, label: `${c.standard} ${c.name}` }))}
          onChange={setKind}
        />
        {entry && (
          <Choice
            label="Size"
            value={shownSize}
            options={entry.sizes.map((s) => ({ value: s, label: s }))}
            onChange={setSize}
          />
        )}
        {entry && entry.lengths.length > 0 && (
          <Choice
            label="Length"
            value={shownLength}
            options={entry.lengths.map((l) => ({ value: l, label: formatLength(l, units) }))}
            onChange={setLength}
          />
        )}
        <div className="hint">
          Axis along Z, bearing face on Z = 0, head toward +Z. Mate it concentric to a hole and
          coincident to a face.
        </div>
      </Section>
    </PanelShell>
  )
}

/** Move or Rotate Component: drag in the viewport, or type a motion. */
export function MovePanel({ panel }: { panel: Panel }) {
  const rotate = panel.type === 'rotate_component'
  const move = useMove()
  const chosen = useStore((s) => s.asmComponents)
  const assembly = useStore((s) => s.assembly)
  const [dx, setDx] = useState(0)
  const [dy, setDy] = useState(0)
  const [dz, setDz] = useState(0)
  const [ax, setAx] = useState(0)
  const [ay, setAy] = useState(0)
  const [az, setAz] = useState(0)
  const [to, setTo] = useState(false)
  useEffect(() => {
    useMove.setState({ active: true, mode: rotate ? 'rotate' : 'free' })
    return () => useMove.setState({ active: false })
  }, [rotate])
  useEffect(() => () => closeAsmPanel(), [])
  const target = chosen[0]
  const apply = async () => {
    if (!target) return
    const motion = rotate
      ? { mode: 'rotate', angles: [ax, ay, az] }
      : to
        ? { mode: 'to', local: [0, 0, 0], point: [dx, dy, dz] }
        : { mode: 'delta', translation: [dx, dy, dz] }
    await useStore
      .getState()
      .asmMove(target, motion, 'end', move.collision ? { dragged_only: move.draggedOnly } : null)
  }
  return (
    <PanelShell title={rotate ? 'Rotate Component' : 'Move Component'} onCancel={closeAsmPanel}>
      <Section title="Move">
        <div className="hint">
          Drag a component in the graphics area{rotate ? ' to rotate it' : ''}, or use the arrows
          and rings of the triad on a selected component. It moves within its mates.
        </div>
        {!rotate && (
          <Choice
            label="Drag"
            value={move.mode === 'rotate' ? 'free' : move.mode}
            options={[
              { value: 'free', label: 'Free drag' },
              { value: 'x', label: 'Along assembly X' },
              { value: 'y', label: 'Along assembly Y' },
              { value: 'z', label: 'Along assembly Z' },
            ]}
            onChange={(mode) => useMove.setState({ mode })}
          />
        )}
      </Section>
      <Section title={rotate ? 'By delta XYZ' : to ? 'To XYZ position' : 'By delta XYZ'}>
        <div className="hint">
          {target
            ? `Component: ${componentName(assembly, target)}`
            : 'Select a component in the tree or graphics area'}
        </div>
        {!rotate && (
          <Check label="To XYZ position (component origin)" value={to} onChange={setTo} />
        )}
        {rotate ? (
          <>
            <AngleField label="About X" value={ax} onChange={setAx} />
            <AngleField label="About Y" value={ay} onChange={setAy} />
            <AngleField label="About Z" value={az} onChange={setAz} />
          </>
        ) : (
          <>
            <LengthField label="X" value={dx} min={-1e9} onChange={setDx} />
            <LengthField label="Y" value={dy} min={-1e9} onChange={setDy} />
            <LengthField label="Z" value={dz} min={-1e9} onChange={setDz} />
          </>
        )}
        <button disabled={!target} onClick={() => void apply()}>
          Apply
        </button>
      </Section>
      <Section title="Options">
        <Check
          label="Collision detection (stop at collision)"
          value={move.collision}
          onChange={(collision) => useMove.setState({ collision })}
        />
        {move.collision && (
          <Check
            label="Dragged part only"
            value={move.draggedOnly}
            onChange={(draggedOnly) => useMove.setState({ draggedOnly })}
          />
        )}
      </Section>
    </PanelShell>
  )
}

type PatternType = 'linear' | 'circular' | 'pattern_driven' | 'sketch_driven' | 'curve_driven'

export function ComponentPatternPanel({ panel }: { panel: Panel }) {
  const assembly = useStore((s) => s.assembly)
  const existing =
    panel.editing !== undefined ? assembly?.patterns.find((p) => p.id === panel.editing) : undefined
  const [type, setType] = useState<PatternType>(
    (existing?.type as PatternType) ?? (panel.variant as PatternType) ?? 'linear',
  )
  const [seeds, setSeeds] = useState<Picked[]>(
    existing
      ? asComponents(existing.seeds.map((s) => [s]))
      : asComponents(useStore.getState().asmComponents),
  )
  const [direction, setDirection] = useState<Picked[]>([])
  const [direction2, setDirection2] = useState<Picked[]>([])
  const [feature, setFeature] = useState<string>('')
  const [curve, setCurve] = useState<Picked[]>([])
  const [spacing, setSpacing] = useState(20)
  const [count, setCount] = useState(3)
  const [spacing2, setSpacing2] = useState(20)
  const [count2, setCount2] = useState(1)
  const [angle, setAngle] = useState(2 * Math.PI)
  const [equal, setEqual] = useState(true)
  const [reverse, setReverse] = useState(false)
  const [tangent, setTangent] = useState(false)
  const [sketchComponent, setSketchComponent] = useState<Picked[]>([])
  const [sketchId, setSketchId] = useState(0)
  const [active, setActive] = useState('seeds')
  const [error, setError] = useState<string | null>(null)
  useEffect(() => () => closeAsmPanel(), [])

  const pattern = (() => {
    const dir = (p: Picked[], s: number, c: number) =>
      p[0] ? { reference: p[0].ref, spacing: s, count: c, reverse } : null
    switch (type) {
      case 'linear': {
        const d1 = dir(direction, spacing, count)
        if (!d1) return null
        const d2 = count2 > 1 ? dir(direction2, spacing2, count2) : null
        return { type, direction1: d1, ...(d2 ? { direction2: d2 } : {}) }
      }
      case 'circular':
        return direction[0]
          ? { type, axis: direction[0].ref, angle, count, equal_spacing: equal, reverse }
          : null
      case 'pattern_driven': {
        const [comp, id] = feature.split(':')
        return comp && id
          ? { type, feature: { component: [comp], entity: { type: 'feature', id: Number(id) } } }
          : null
      }
      case 'sketch_driven':
        return sketchId > 0
          ? {
              type,
              sketch: {
                component: sketchComponent[0]?.ref.component ?? [],
                entity: { type: 'feature', id: sketchId },
              },
            }
          : null
      case 'curve_driven':
        return curve.length
          ? { type, curve: curve.map((c) => c.ref), spacing, count, tangent, reverse }
          : null
    }
  })()
  const seedKeys = seeds.map((s) => s.ref.component[0]!).filter(Boolean)
  const ok = async () => {
    if (!pattern || !seedKeys.length) return
    const body = { pattern, seeds: seedKeys }
    const done = existing
      ? await commitAsm('asm.pattern.update', { id: existing.id, ...body, name: existing.name })
      : await commitAsm('asm.pattern.add', body)
    if (!done) setError('The pattern could not be made')
  }
  const index = componentIndex(assembly)
  return (
    <PanelShell
      title={existing ? `Edit ${existing.name}` : 'Component Pattern'}
      onOk={() => void ok()}
      onCancel={closeAsmPanel}
      okDisabled={!pattern || !seedKeys.length}
      error={error}
    >
      <Section title="Type">
        <Choice
          label="Pattern"
          value={type}
          options={[
            { value: 'linear', label: 'Linear' },
            { value: 'circular', label: 'Circular' },
            { value: 'pattern_driven', label: 'Pattern driven' },
            { value: 'sketch_driven', label: 'Sketch driven' },
            { value: 'curve_driven', label: 'Curve driven' },
          ]}
          onChange={setType}
        />
      </Section>
      <Section title="Components to pattern">
        <AsmRefBox
          label="Seed components"
          refs={seeds}
          onChange={setSeeds}
          kinds={['component']}
          active={active === 'seeds'}
          onActivate={() => setActive('seeds')}
        />
      </Section>
      {type === 'linear' && (
        <>
          <Section title="Direction 1">
            <AsmRefBox
              label="Direction (edge, axis or face)"
              refs={direction}
              onChange={setDirection}
              kinds={['edge', 'face']}
              single
              active={active === 'd1'}
              onActivate={() => setActive('d1')}
            />
            <LengthField label="Spacing" value={spacing} onChange={setSpacing} />
            <IntField label="Instances" value={count} onChange={setCount} />
            <Check label="Reverse" value={reverse} onChange={setReverse} />
          </Section>
          <Section title="Direction 2" open={false}>
            <AsmRefBox
              label="Direction 2"
              refs={direction2}
              onChange={setDirection2}
              kinds={['edge', 'face']}
              single
              active={active === 'd2'}
              onActivate={() => setActive('d2')}
            />
            <LengthField label="Spacing" value={spacing2} onChange={setSpacing2} />
            <IntField label="Instances" value={count2} onChange={setCount2} />
          </Section>
        </>
      )}
      {type === 'circular' && (
        <Section title="Axis">
          <AsmRefBox
            label="Axis (cylinder, edge or axis)"
            refs={direction}
            onChange={setDirection}
            kinds={['edge', 'face']}
            single
            active={active === 'd1'}
            onActivate={() => setActive('d1')}
          />
          <AngleField label="Angle" value={angle} onChange={setAngle} />
          <IntField label="Instances" value={count} onChange={setCount} />
          <Check label="Equal spacing" value={equal} onChange={setEqual} />
          <Check label="Reverse" value={reverse} onChange={setReverse} />
        </Section>
      )}
      {type === 'pattern_driven' && (
        <Section title="Driving feature">
          <label className="field">
            <span>Component:feature</span>
            <input
              value={feature}
              placeholder="e.g. 1:5 (component 1, feature 5)"
              onChange={(e) => setFeature(e.target.value)}
            />
          </label>
          <div className="hint">
            Components:{' '}
            {[...index.values()]
              .filter((c) => c.key.length === 1)
              .map((c) => `${c.key[0]} ${c.name}`)
              .join(', ')}
          </div>
        </Section>
      )}
      {type === 'sketch_driven' && (
        <Section title="Driving sketch">
          <AsmRefBox
            label="Component with the sketch (empty: assembly)"
            refs={sketchComponent}
            onChange={setSketchComponent}
            kinds={['component']}
            single
            active={active === 'sk'}
            onActivate={() => setActive('sk')}
          />
          <IntField label="Sketch feature ID" value={sketchId} min={0} onChange={setSketchId} />
        </Section>
      )}
      {type === 'curve_driven' && (
        <Section title="Curve">
          <AsmRefBox
            label="Curve edges"
            refs={curve}
            onChange={setCurve}
            kinds={['edge']}
            active={active === 'curve'}
            onActivate={() => setActive('curve')}
          />
          <LengthField label="Spacing" value={spacing} onChange={setSpacing} />
          <IntField label="Instances" value={count} onChange={setCount} />
          <Check label="Align to curve tangent" value={tangent} onChange={setTangent} />
          <Check label="Reverse" value={reverse} onChange={setReverse} />
        </Section>
      )}
    </PanelShell>
  )
}

export function MirrorComponentsPanel({ panel }: { panel: Panel }) {
  const assembly = useStore((s) => s.assembly)
  const existing =
    panel.editing !== undefined ? assembly?.mirrors.find((m) => m.id === panel.editing) : undefined
  const [plane, setPlane] = useState<Picked[]>(
    existing ? [{ ref: existing.plane, geometry: null }] : [],
  )
  const [components, setComponents] = useState<Picked[]>(
    existing
      ? asComponents(existing.components.map((c) => [c.component]))
      : asComponents(useStore.getState().asmComponents),
  )
  const [hands, setHands] = useState<Record<string, boolean>>(
    Object.fromEntries((existing?.components ?? []).map((c) => [c.component, !!c.opposite_hand])),
  )
  const [position, setPosition] = useState<string>(existing?.position ?? 'bounding_box')
  const [active, setActive] = useState('plane')
  const [error, setError] = useState<string | null>(null)
  useEffect(() => () => closeAsmPanel(), [])
  const items = components.map((c) => {
    const key = c.ref.component[0]!
    const name = componentName(assembly, [key]).replace(/<\d+>$/, '')
    return hands[key] ? { component: key, opposite_hand: `${name}-mirror.ofp` } : { component: key }
  })
  const ok = async () => {
    if (!plane[0] || !items.length) return
    const body = { plane: plane[0].ref, components: items, position }
    const done = existing
      ? await commitAsm('asm.mirror.update', { id: existing.id, ...body, name: existing.name })
      : await commitAsm('asm.mirror.add', body)
    if (!done) setError('The components could not be mirrored')
  }
  const unsaved = !assembly?.path && Object.values(hands).some(Boolean)
  return (
    <PanelShell
      title="Mirror Components"
      onOk={() => void ok()}
      onCancel={closeAsmPanel}
      okDisabled={!plane[0] || !items.length}
      error={
        error ??
        (unsaved ? 'Save the assembly first: opposite-hand parts are written beside it.' : null)
      }
    >
      <Section title="Selections">
        <AsmRefBox
          label="Mirror plane"
          refs={plane}
          onChange={setPlane}
          kinds={['face']}
          single
          active={active === 'plane'}
          onActivate={() => setActive('plane')}
          onFilled={() => setActive('components')}
        />
        <AsmRefBox
          label="Components to mirror"
          refs={components}
          onChange={setComponents}
          kinds={['component']}
          active={active === 'components'}
          onActivate={() => setActive('components')}
        />
      </Section>
      <Section title="Orientation">
        {components.map((c) => {
          const key = c.ref.component[0]!
          return (
            <Check
              key={key}
              label={`${componentName(assembly, [key])}: opposite-hand version`}
              value={!!hands[key]}
              onChange={(v) => setHands((h) => ({ ...h, [key]: v }))}
            />
          )
        })}
        <Choice
          label="Position by"
          value={position}
          options={[
            { value: 'bounding_box', label: 'Bounding box center' },
            { value: 'center_of_mass', label: 'Center of mass' },
            { value: 'origin', label: 'Component origin' },
          ]}
          onChange={setPosition}
        />
      </Section>
    </PanelShell>
  )
}

export function ExplodePanel() {
  const assembly = useStore((s) => s.assembly)
  const steps = assembly?.explode.steps ?? []
  const shown = assembly?.explode.shown ?? null
  const [components, setComponents] = useState<Picked[]>(
    asComponents(useStore.getState().asmComponents),
  )
  const [kind, setKind] = useState<'translate' | 'rotate' | 'radial'>('translate')
  const [axis, setAxis] = useState<'x' | 'y' | 'z' | '-x' | '-y' | '-z'>('z')
  const [distance, setDistance] = useState(50)
  const [angle, setAngle] = useState(Math.PI / 2)
  const [autoSpace, setAutoSpace] = useState(false)
  const [active, setActive] = useState('components')
  const [animating, setAnimating] = useState(false)
  useEffect(() => {
    if (shown === null && steps.length)
      void useStore.getState().run('asm.explode.show', { steps: null })
    return () => closeAsmPanel()
  }, []) // eslint-disable-line react-hooks/exhaustive-deps
  const vector = (): [number, number, number] => {
    const sign = axis.startsWith('-') ? -1 : 1
    const i = 'xyz'.indexOf(axis.replace('-', ''))
    const v: [number, number, number] = [0, 0, 0]
    v[i] = sign
    return v
  }
  const center = (): [number, number, number] => {
    const b = useStore.getState().asmScene?.bbox
    return b
      ? [(b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, (b.min[2] + b.max[2]) / 2]
      : [0, 0, 0]
  }
  const add = async () => {
    const keys = components.map((c) => c.ref.component.slice(0, 1))
    if (!keys.length) return
    const motion =
      kind === 'translate'
        ? { type: 'translate', direction: vector(), distance, auto_space: autoSpace }
        : kind === 'rotate'
          ? { type: 'rotate', origin: center(), axis: vector(), angle }
          : { type: 'radial', origin: center(), axis: vector(), distance }
    if (await commitAsm('asm.explode.add', { components: keys, motion }, true)) setComponents([])
  }
  const animate = async (explode: boolean) => {
    setAnimating(true)
    const total = steps.length
    for (let i = explode ? 1 : total - 1; explode ? i <= total : i >= 0; i += explode ? 1 : -1) {
      // Stop when the document is no longer this assembly.
      if (!useStore.getState().assembly) return setAnimating(false)
      await useStore.getState().run('asm.explode.show', { steps: i === 0 && !explode ? null : i })
      await new Promise((r) => setTimeout(r, 250))
    }
    if (!explode && useStore.getState().assembly)
      await useStore.getState().run('asm.explode.show', { steps: null })
    setAnimating(false)
  }
  return (
    <PanelShell title="Exploded View" onCancel={closeAsmPanel} onOk={closeAsmPanel}>
      <Section title={`Explode steps (${steps.length})`}>
        {steps.map((s) => (
          <div key={s.id} className="chip">
            <span>
              {s.name} · {s.components.map((k) => componentName(assembly, k)).join(', ')}
            </span>
            <button
              onClick={() => void useStore.getState().run('asm.explode.delete', { ids: [s.id] })}
            >
              ×
            </button>
          </div>
        ))}
        <div className="row">
          <button disabled={!steps.length || animating} onClick={() => void animate(true)}>
            Explode
          </button>
          <button disabled={!steps.length || animating} onClick={() => void animate(false)}>
            Collapse
          </button>
        </div>
      </Section>
      <Section title="Add step">
        <AsmRefBox
          label="Components"
          refs={components}
          onChange={setComponents}
          kinds={['component']}
          active={active === 'components'}
          onActivate={() => setActive('components')}
        />
        <Choice
          label="Step"
          value={kind}
          options={[
            { value: 'translate', label: 'Move along direction' },
            { value: 'rotate', label: 'Rotate about axis' },
            { value: 'radial', label: 'Radial (spread from axis)' },
          ]}
          onChange={setKind}
        />
        <Choice
          label={kind === 'translate' ? 'Direction' : 'Axis'}
          value={axis}
          options={['x', 'y', 'z', '-x', '-y', '-z'].map((v) => ({
            value: v as typeof axis,
            label: v.toUpperCase(),
          }))}
          onChange={setAxis}
        />
        {kind === 'rotate' ? (
          <AngleField label="Angle" value={angle} onChange={setAngle} />
        ) : (
          <LengthField label="Distance" value={distance} onChange={setDistance} />
        )}
        {kind === 'translate' && (
          <Check label="Auto-space components" value={autoSpace} onChange={setAutoSpace} />
        )}
        <button disabled={!components.length} onClick={() => void add()}>
          Add step
        </button>
      </Section>
    </PanelShell>
  )
}

interface InterferenceResult {
  a: string[]
  b: string[]
  volume: number
  ignored: boolean
  mesh: MeshPayload | null
}

export function InterferencePanel() {
  const assembly = useStore((s) => s.assembly)
  const [selected, setSelected] = useState<Picked[]>([])
  const [coincidence, setCoincidence] = useState(false)
  const [subAsComponents, setSubAsComponents] = useState(false)
  const [hidden, setHidden] = useState(false)
  const [showIgnored, setShowIgnored] = useState(false)
  const [results, setResults] = useState<InterferenceResult[] | null>(null)
  const [focus, setFocus] = useState<number | null>(null)
  const [active, setActive] = useState('components')
  const units = useStore((s) => s.snapshot!.units)
  useEffect(() => () => closeAsmPanel(), [])
  const calculate = async () => {
    try {
      const r = await call<{ interferences: InterferenceResult[] }>('asm.interference', {
        components: selected.map((s) => s.ref.component.slice(0, 1)),
        treat_coincidence: coincidence,
        subassemblies_as_components: subAsComponents,
        include_hidden: hidden,
      })
      setResults(r.interferences)
      useStore.getState().setAsmMarks({
        faces: [],
        meshes: r.interferences.flatMap((i) => (i.mesh ? [i.mesh] : [])),
      })
    } catch (e) {
      useStore.getState().toast((e as Error).message)
    }
  }
  const shownResults = (results ?? []).filter((r) => showIgnored || !r.ignored)
  return (
    <PanelShell title="Interference Detection" onCancel={closeAsmPanel} onOk={closeAsmPanel}>
      <Section title="Selected components">
        <AsmRefBox
          label="Components (empty: whole assembly)"
          refs={selected}
          onChange={setSelected}
          kinds={['component']}
          active={active === 'components'}
          onActivate={() => setActive('components')}
        />
        <button onClick={() => void calculate()}>Calculate</button>
      </Section>
      <Section title={`Results${results ? ` (${shownResults.length})` : ''}`}>
        {results && shownResults.length === 0 && <div className="hint">No interference</div>}
        {shownResults.map((r, i) => (
          <div
            key={i}
            className={`chip ${focus === i ? 'active' : ''}`}
            onClick={() => {
              setFocus(i)
              useStore.getState().selectComponents([r.a, r.b])
              useStore.getState().setAsmMarks({ faces: [], meshes: r.mesh ? [r.mesh] : [] })
            }}
          >
            <span>
              {componentName(assembly, r.a)} ↔ {componentName(assembly, r.b)}:{' '}
              {r.volume > 0 ? `${r.volume.toFixed(3)} ${units.length}³` : 'touching'}
              {r.ignored ? ' (ignored)' : ''}
            </span>
            <button
              title={r.ignored ? 'Stop ignoring' : 'Ignore'}
              onClick={async (e) => {
                e.stopPropagation()
                await useStore
                  .getState()
                  .run('asm.interference.ignore', { a: r.a, b: r.b, ignored: !r.ignored })
                setResults(
                  (rs) => rs?.map((x) => (x === r ? { ...x, ignored: !x.ignored } : x)) ?? null,
                )
              }}
            >
              {r.ignored ? '↺' : '⊘'}
            </button>
          </div>
        ))}
      </Section>
      <Section title="Options">
        <Check
          label="Treat coincidence as interference"
          value={coincidence}
          onChange={setCoincidence}
        />
        <Check label="Show ignored interferences" value={showIgnored} onChange={setShowIgnored} />
        <Check
          label="Treat subassemblies as components"
          value={subAsComponents}
          onChange={setSubAsComponents}
        />
        <Check label="Include hidden components" value={hidden} onChange={setHidden} />
      </Section>
    </PanelShell>
  )
}

export function ClearancePanel() {
  const assembly = useStore((s) => s.assembly)
  const [selected, setSelected] = useState<Picked[]>([])
  const [minimum, setMinimum] = useState(1)
  const [results, setResults] = useState<{ a: string[]; b: string[]; distance: number }[] | null>(
    null,
  )
  const [active, setActive] = useState('components')
  const units = useStore((s) => s.snapshot!.units)
  useEffect(() => () => closeAsmPanel(), [])
  return (
    <PanelShell title="Clearance Verification" onCancel={closeAsmPanel} onOk={closeAsmPanel}>
      <Section title="Selections">
        <AsmRefBox
          label="Components (empty: whole assembly)"
          refs={selected}
          onChange={setSelected}
          kinds={['component']}
          active={active === 'components'}
          onActivate={() => setActive('components')}
        />
        <LengthField label="Minimum acceptable clearance" value={minimum} onChange={setMinimum} />
        <button
          onClick={async () => {
            try {
              const r = await call<{
                clearances: { a: string[]; b: string[]; distance: number }[]
              }>('asm.clearance', {
                components: selected.map((s) => s.ref.component.slice(0, 1)),
                minimum,
              })
              setResults(r.clearances)
            } catch (e) {
              useStore.getState().toast((e as Error).message)
            }
          }}
        >
          Calculate
        </button>
      </Section>
      <Section title="Results">
        {results && results.length === 0 && (
          <div className="hint">Every clearance is at least {formatLength(minimum, units)}</div>
        )}
        {results?.map((r, i) => (
          <div
            key={i}
            className="chip"
            onClick={() => useStore.getState().selectComponents([r.a, r.b])}
          >
            <span>
              {componentName(assembly, r.a)} ↔ {componentName(assembly, r.b)}:{' '}
              {formatLength(r.distance, units)}
            </span>
          </div>
        ))}
      </Section>
    </PanelShell>
  )
}

export function BomPanel() {
  const [type, setType] = useState<'top_level' | 'parts_only' | 'indented'>('top_level')
  const [rows, setRows] = useState<BomRow[]>([])
  const units = useStore((s) => s.snapshot!.units)
  useEffect(() => {
    void call<{ rows: BomRow[] }>('asm.bom', { type }).then(
      (r) => setRows(r.rows),
      (e: Error) => useStore.getState().toast(e.message),
    )
  }, [type])
  useEffect(() => () => closeAsmPanel(), [])
  return (
    <PanelShell title="Bill of Materials" onCancel={closeAsmPanel} onOk={closeAsmPanel}>
      <Section title="Type">
        <Choice
          label="BOM type"
          value={type}
          options={[
            { value: 'top_level', label: 'Top-level only' },
            { value: 'parts_only', label: 'Parts only' },
            { value: 'indented', label: 'Indented' },
          ]}
          onChange={setType}
        />
        <button
          onClick={async () => {
            const name =
              (useStore
                .getState()
                .snapshot?.path?.split(/[\\/]/)
                .pop()
                ?.replace(/\.ofa$/i, '') ?? 'Assembly') + ' BOM.csv'
            const path = await window.openforge.saveDialog('csv', name)
            if (path) {
              await call('asm.bom', { type, csv: path })
              useStore.getState().toast('BOM exported', 'info')
            }
          }}
        >
          Export CSV…
        </button>
      </Section>
      <table className="results bom">
        <thead>
          <tr>
            <th>Item</th>
            <th>Qty</th>
            <th>Part Number</th>
            <th>Description</th>
            <th>Mass</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.item}>
              <td style={{ paddingLeft: 4 + r.level * 10 }}>{r.item}</td>
              <td>{r.quantity}</td>
              <td>{r.part_number}</td>
              <td>{r.description}</td>
              <td>
                {r.mass === null
                  ? ''
                  : units.mass === 'kg'
                    ? `${r.mass.toFixed(3)} kg`
                    : `${(r.mass * 1000).toFixed(1)} g`}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </PanelShell>
  )
}

export function ComponentPropertiesPanel({ panel }: { panel: Panel }) {
  const assembly = useStore((s) => s.assembly)
  const c = componentIndex(assembly).get(panel.component?.join('/') ?? '')
  const [name, setName] = useState(c?.name ?? '')
  useEffect(() => () => closeAsmPanel(), [])
  if (!c)
    return (
      <PanelShell title="Component Properties" onCancel={closeAsmPanel}>
        Component not found
      </PanelShell>
    )
  const key = c.key[0]!
  const update = (patch: Record<string, unknown>) =>
    void useStore.getState().run('asm.component.update', { key, ...patch })
  return (
    <PanelShell
      title="Component Properties"
      onCancel={closeAsmPanel}
      onOk={async () => {
        if (name.trim() && name !== c.name) await commitAsm('asm.component.update', { key, name })
        else closeAsmPanel()
      }}
    >
      <Section title="General">
        <label className="field">
          <span>Instance name</span>
          <input value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        <div className="hint">{c.file}</div>
        {c.error && <div className="panel-error">{c.error}</div>}
      </Section>
      {c.kind === 'part' && (c.configurations?.length ?? 0) > 1 && (
        <Section title="Configuration">
          <Choice
            label="Referenced configuration"
            value={c.configuration ?? 0}
            options={[
              { value: 0, label: 'Use the file’s active configuration' },
              ...(c.configurations ?? []).map((x) => ({ value: x.id, label: x.name })),
            ]}
            onChange={(v) => update({ configuration: v === 0 ? null : v })}
          />
        </Section>
      )}
      <Section title="State">
        <Check label="Fixed" value={c.fixed} onChange={(fixed) => update({ fixed })} />
        <Check label="Hidden" value={!!c.hidden} onChange={(hidden) => update({ hidden })} />
        <Check
          label="Suppressed (this configuration)"
          value={!!c.suppressed}
          onChange={(suppressed) => update({ suppressed })}
        />
        <Check
          label="Exclude from bill of materials"
          value={!!c.exclude_from_bom}
          onChange={(v) => update({ exclude_from_bom: v })}
        />
        {c.kind === 'assembly' && (
          <Check
            label="Solve as flexible"
            value={!!c.flexible}
            onChange={(flexible) => update({ flexible })}
          />
        )}
      </Section>
    </PanelShell>
  )
}

export function AsmConfigurationsPanel() {
  const assembly = useStore((s) => s.assembly)
  const run = useStore((s) => s.run)
  useEffect(() => () => closeAsmPanel(), [])
  if (!assembly) return null
  return (
    <PanelShell title="Assembly Configurations" onCancel={closeAsmPanel} onOk={closeAsmPanel}>
      <Section title="Configurations">
        {assembly.configurations.items.map((c) => (
          <div
            key={c.id}
            className={`chip ${c.id === assembly.configurations.active ? 'active' : ''}`}
          >
            <span onClick={() => void run('asm.configuration.activate', { id: c.id })}>
              {c.name}
              {c.id === assembly.configurations.active ? ' (active)' : ''}
            </span>
            <button
              title="Rename"
              onClick={() => {
                const name = prompt('Configuration name', c.name)
                if (name?.trim()) void run('asm.configuration.rename', { id: c.id, name })
              }}
            >
              ✎
            </button>
            <button
              title="Delete"
              disabled={assembly.configurations.items.length < 2}
              onClick={() => void run('asm.configuration.delete', { id: c.id })}
            >
              ×
            </button>
          </div>
        ))}
        <div className="row">
          <button
            onClick={() => {
              const name = prompt('New configuration name')
              if (name?.trim()) void run('asm.configuration.create', { name })
            }}
          >
            New
          </button>
          <button
            onClick={() => {
              const name = prompt('Copy configuration as')
              if (name?.trim())
                void run('asm.configuration.create', {
                  name,
                  copy_from: assembly.configurations.active,
                })
            }}
          >
            Duplicate
          </button>
        </div>
        <div className="hint">
          Suppression, part configurations, mate suppression and exploded views belong to the active
          configuration.
        </div>
      </Section>
    </PanelShell>
  )
}

/** Assembly reference planes and axes. */
export function AsmReferencePanel({ panel }: { panel: Panel }) {
  const [kind, setKind] = useState<'plane' | 'axis'>(panel.variant === 'axis' ? 'axis' : 'plane')
  const [base, setBase] = useState<'front_plane' | 'top_plane' | 'right_plane'>('front_plane')
  const [base2, setBase2] = useState<'front_plane' | 'top_plane' | 'right_plane'>('right_plane')
  const [distance, setDistance] = useState(20)
  useEffect(() => () => closeAsmPanel(), [])
  const planes = [
    { value: 'front_plane' as const, label: 'Front Plane' },
    { value: 'top_plane' as const, label: 'Top Plane' },
    { value: 'right_plane' as const, label: 'Right Plane' },
  ]
  const feature =
    kind === 'plane'
      ? { type: 'plane', mode: 'offset', reference: { type: base }, distance }
      : { type: 'axis', mode: 'two_planes', a: { type: base }, b: { type: base2 } }
  return (
    <PanelShell
      title={kind === 'plane' ? 'Plane' : 'Axis'}
      onCancel={closeAsmPanel}
      onOk={() => void commitAsm('asm.reference.add', { feature })}
      okDisabled={kind === 'axis' && base === base2}
    >
      <Section title="Reference">
        <Choice
          label="Type"
          value={kind}
          options={[
            { value: 'plane', label: 'Offset plane' },
            { value: 'axis', label: 'Axis where two planes meet' },
          ]}
          onChange={setKind}
        />
        <Choice
          label={kind === 'plane' ? 'From' : 'First plane'}
          value={base}
          options={planes}
          onChange={setBase}
        />
        {kind === 'plane' ? (
          <LengthField label="Offset" value={distance} min={-1e9} onChange={setDistance} />
        ) : (
          <Choice label="Second plane" value={base2} options={planes} onChange={setBase2} />
        )}
      </Section>
    </PanelShell>
  )
}

export function AsmMeasurePanel() {
  const [refs, setRefs] = useState<Picked[]>([])
  const [result, setResult] = useState<{ distance?: number; angle?: number } | null>(null)
  const units = useStore((s) => s.snapshot!.units)
  useEffect(() => () => closeAsmPanel(), [])
  const json = useMemo(() => JSON.stringify(refs.map((r) => r.ref)), [refs])
  useEffect(() => {
    const picked = JSON.parse(json) as CompRef[]
    if (!picked.length) return
    void call<{ distance?: number; angle?: number }>('asm.measure', { refs: picked }).then(
      setResult,
      () => setResult(null),
    )
  }, [json])
  const shown = refs.length ? result : null
  return (
    <PanelShell title="Measure" onCancel={closeAsmPanel}>
      <AsmRefBox
        label="Select one or two entities"
        refs={refs}
        onChange={(r) => setRefs(r.slice(-2))}
        kinds={['face', 'edge', 'vertex']}
        active
        onActivate={() => {}}
      />
      {shown?.distance !== undefined && (
        <div className="field">
          <span>Distance</span>
          <b>{formatLength(shown.distance, units)}</b>
        </div>
      )}
      {shown?.angle !== undefined && (
        <div className="field">
          <span>Angle</span>
          <b>{((shown.angle * 180) / Math.PI).toFixed(2)}°</b>
        </div>
      )}
    </PanelShell>
  )
}

export function AsmMassPanel() {
  const [mass, setMass] = useState<{
    mass: number
    volume: number
    area: number
    center: number[]
    components: number
  } | null>(null)
  const units = useStore((s) => s.snapshot!.units)
  useEffect(() => {
    void call<typeof mass>('mass_properties').then(setMass, (e: Error) =>
      useStore.getState().toast(e.message),
    )
    return () => closeAsmPanel()
  }, [])
  return (
    <PanelShell title="Mass Properties" onCancel={closeAsmPanel}>
      {mass && (
        <table className="results">
          <tbody>
            <tr>
              <th>Mass</th>
              <td>
                {units.mass === 'kg'
                  ? `${mass.mass.toFixed(4)} kg`
                  : `${(mass.mass * 1000).toFixed(2)} g`}
              </td>
            </tr>
            <tr>
              <th>Volume</th>
              <td>{mass.volume.toFixed(2)} mm³</td>
            </tr>
            <tr>
              <th>Surface area</th>
              <td>{mass.area.toFixed(2)} mm²</td>
            </tr>
            <tr>
              <th>Center of mass</th>
              <td>{mass.center.map((v) => formatLength(v, units)).join(', ')}</td>
            </tr>
            <tr>
              <th>Parts</th>
              <td>{mass.components}</td>
            </tr>
          </tbody>
        </table>
      )}
      {!mass && <div className="hint">Calculating…</div>}
    </PanelShell>
  )
}

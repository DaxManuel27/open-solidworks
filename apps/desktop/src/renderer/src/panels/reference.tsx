import { useState } from 'react'
import type { FeatureParams, Ref } from '../protocol'
import { type Panel, type PickKind, useStore } from '../store'
import {
  AngleField,
  Check,
  Choice,
  LengthField,
  PanelShell,
  RefBox,
  Section,
  usePreview,
} from './fields'

const SURFACE: PickKind[] = ['face', 'datum', 'std_plane']
const LINE: PickKind[] = ['edge', 'datum']
const POINT: PickKind[] = ['vertex', 'datum']
const ROUND: PickKind[] = ['face', 'edge']

interface Slot {
  key: string
  label: string
  kinds: PickKind[]
  origin?: boolean
}

interface Mode {
  value: string
  label: string
  slots: Slot[]
}

const PLANE_MODES: Mode[] = [
  {
    value: 'offset',
    label: 'Offset distance',
    slots: [{ key: 'reference', label: 'Plane or face', kinds: SURFACE }],
  },
  {
    value: 'angle',
    label: 'At angle',
    slots: [
      { key: 'reference', label: 'Plane or face', kinds: SURFACE },
      { key: 'axis', label: 'Through line or axis', kinds: LINE },
    ],
  },
  {
    value: 'three_points',
    label: 'Through three points',
    slots: [
      { key: 'p0', label: 'Point 1', kinds: POINT, origin: true },
      { key: 'p1', label: 'Point 2', kinds: POINT, origin: true },
      { key: 'p2', label: 'Point 3', kinds: POINT, origin: true },
    ],
  },
  {
    value: 'mid_plane',
    label: 'Mid plane',
    slots: [
      { key: 'a', label: 'First plane or face', kinds: SURFACE },
      { key: 'b', label: 'Second plane or face', kinds: SURFACE },
    ],
  },
  {
    value: 'parallel_at_point',
    label: 'Parallel at point',
    slots: [
      { key: 'reference', label: 'Plane or face', kinds: SURFACE },
      { key: 'point', label: 'Point', kinds: POINT, origin: true },
    ],
  },
  {
    value: 'normal_to_curve',
    label: 'Normal to edge',
    slots: [{ key: 'edge', label: 'Edge', kinds: ['edge'] }],
  },
]

const AXIS_MODES: Mode[] = [
  {
    value: 'edge',
    label: 'Along edge',
    slots: [{ key: 'edge', label: 'Straight edge', kinds: ['edge'] }],
  },
  {
    value: 'cylinder',
    label: 'Cylindrical face',
    slots: [{ key: 'face', label: 'Round face or edge', kinds: ROUND }],
  },
  {
    value: 'two_planes',
    label: 'Two planes',
    slots: [
      { key: 'a', label: 'First plane or face', kinds: SURFACE },
      { key: 'b', label: 'Second plane or face', kinds: SURFACE },
    ],
  },
  {
    value: 'two_points',
    label: 'Two points',
    slots: [
      { key: 'a', label: 'Point 1', kinds: POINT, origin: true },
      { key: 'b', label: 'Point 2', kinds: POINT, origin: true },
    ],
  },
  {
    value: 'point_and_face',
    label: 'Point and face',
    slots: [
      { key: 'point', label: 'Point', kinds: POINT, origin: true },
      { key: 'face', label: 'Plane or face', kinds: SURFACE },
    ],
  },
]

/** Reference slots for the chosen mode, and which one picks go to. */
function useSlots(init: Record<string, unknown>) {
  const [refs, setRefs] = useState<Record<string, Ref | null>>(() => {
    const out: Record<string, Ref | null> = {}
    for (const [k, v] of Object.entries(init))
      if (v && typeof v === 'object' && 'type' in v) out[k] = v as Ref
    const pts = init['points'] as Ref[] | undefined
    pts?.forEach((p, i) => (out[`p${i}`] = p))
    return out
  })
  const [active, setActive] = useState(0)
  const render = (slots: Slot[]) =>
    slots.map((s, i) => (
      <RefBox
        key={s.key}
        label={s.label}
        refs={refs[s.key] ? [refs[s.key]!] : []}
        onChange={(r) => {
          setRefs((cur) => ({ ...cur, [s.key]: r[0] ?? null }))
          // Move on to the next empty slot.
          if (r[0] && i + 1 < slots.length) setActive(i + 1)
        }}
        kinds={s.kinds}
        single
        origin={s.origin}
        active={active === i}
        onActivate={() => setActive(i)}
      />
    ))
  const filled = (slots: Slot[]) => slots.every((s) => refs[s.key])
  return { refs, render, filled, reset: () => setActive(0) }
}

function useInit(panel: Panel, defaults: Record<string, unknown>): Record<string, unknown> {
  return useState(() => {
    const f = useStore.getState().snapshot?.features.find((x) => x.id === panel.editing)
    return (f?.params as Record<string, unknown>) ?? defaults
  })[0]
}

function useDatum(panel: Panel, params: FeatureParams | null) {
  const { error } = usePreview(params, panel.editing)
  const commit = useStore((s) => s.commit)
  const close = useStore((s) => s.closePanel)
  return {
    error,
    onOk: () => params && void commit(params, panel.editing),
    onCancel: () => void close(),
    okDisabled: !params,
  }
}

function firstSelected(kinds: string[]): Ref | null {
  return (
    useStore
      .getState()
      .selection.map((s) => s.ref)
      .find((r) => kinds.includes(r.type)) ?? null
  )
}

export function PlanePanel({ panel }: { panel: Panel }) {
  const init = useInit(panel, {
    mode: 'offset',
    reference: firstSelected(['face', 'front_plane', 'top_plane', 'right_plane', 'feature']),
    distance: 10,
  })
  const [mode, setMode] = useState(init['mode'] as string)
  const [distance, setDistance] = useState((init['distance'] as number) ?? 10)
  const [angle, setAngle] = useState((init['angle'] as number) ?? Math.PI / 4)
  const [flip, setFlip] = useState(!!init['flip'])
  const [atEnd, setAtEnd] = useState(!!init['at_end'])
  const slots = useSlots(init)
  const m = PLANE_MODES.find((x) => x.value === mode)!
  const r = slots.refs
  let params: FeatureParams | null = null
  if (slots.filled(m.slots)) {
    switch (mode) {
      case 'offset':
        params = { type: 'plane', mode, reference: r['reference'], distance, flip }
        break
      case 'angle':
        params = { type: 'plane', mode, reference: r['reference'], axis: r['axis'], angle }
        break
      case 'three_points':
        params = { type: 'plane', mode, points: [r['p0'], r['p1'], r['p2']] }
        break
      case 'mid_plane':
        params = { type: 'plane', mode, a: r['a'], b: r['b'] }
        break
      case 'parallel_at_point':
        params = { type: 'plane', mode, reference: r['reference'], point: r['point'] }
        break
      case 'normal_to_curve':
        params = { type: 'plane', mode, edge: r['edge'], at_end: atEnd }
        break
    }
  }
  const f = useDatum(panel, params)
  return (
    <PanelShell title="Plane" {...f}>
      <Section title="References">
        <Choice
          label="Type"
          value={mode}
          options={PLANE_MODES.map(({ value, label }) => ({ value, label }))}
          onChange={(v) => {
            setMode(v)
            slots.reset()
          }}
        />
        {slots.render(m.slots)}
        {mode === 'offset' && (
          <>
            <LengthField label="Distance" value={distance} onChange={setDistance} />
            <Check label="Flip side" value={flip} onChange={setFlip} />
          </>
        )}
        {mode === 'angle' && <AngleField label="Angle" value={angle} onChange={setAngle} />}
        {mode === 'normal_to_curve' && (
          <Check label="At the end of the edge" value={atEnd} onChange={setAtEnd} />
        )}
      </Section>
    </PanelShell>
  )
}

export function AxisPanel({ panel }: { panel: Panel }) {
  const init = useInit(panel, { mode: 'edge', edge: firstSelected(['edge']) })
  const [mode, setMode] = useState(init['mode'] as string)
  const slots = useSlots(init)
  const m = AXIS_MODES.find((x) => x.value === mode)!
  const params: FeatureParams | null = slots.filled(m.slots)
    ? { type: 'axis', mode, ...Object.fromEntries(m.slots.map((s) => [s.key, slots.refs[s.key]])) }
    : null
  const f = useDatum(panel, params)
  return (
    <PanelShell title="Axis" {...f}>
      <Section title="References">
        <Choice
          label="Type"
          value={mode}
          options={AXIS_MODES.map(({ value, label }) => ({ value, label }))}
          onChange={(v) => {
            setMode(v)
            slots.reset()
          }}
        />
        {slots.render(m.slots)}
      </Section>
    </PanelShell>
  )
}

export function PointPanel({ panel }: { panel: Panel }) {
  const init = useInit(panel, { mode: 'at', target: firstSelected(['vertex', 'face', 'edge']) })
  const [mode, setMode] = useState(init['mode'] as string)
  const [xyz, setXyz] = useState<[number, number, number]>([
    (init['x'] as number) ?? 0,
    (init['y'] as number) ?? 0,
    (init['z'] as number) ?? 0,
  ])
  const slots = useSlots(init)
  const slot: Slot[] = [
    { key: 'target', label: 'Vertex, face or round edge', kinds: ['vertex', 'face', 'edge'] },
  ]
  const params: FeatureParams | null =
    mode === 'coordinates'
      ? { type: 'point', mode, x: xyz[0], y: xyz[1], z: xyz[2] }
      : slots.refs['target']
        ? { type: 'point', mode, target: slots.refs['target'] }
        : null
  const f = useDatum(panel, params)
  const setAxis = (i: number) => (v: number) =>
    setXyz((c) => c.map((x, j) => (j === i ? v : x)) as [number, number, number])
  return (
    <PanelShell title="Point" {...f}>
      <Section title="Selections">
        <Choice
          label="Type"
          value={mode}
          options={[
            { value: 'at', label: 'On vertex / center of face or arc' },
            { value: 'coordinates', label: 'Coordinates' },
          ]}
          onChange={setMode}
        />
        {mode === 'at' && slots.render(slot)}
        {mode === 'coordinates' && (
          <>
            <LengthField label="X" value={xyz[0]} onChange={setAxis(0)} min={-1e9} />
            <LengthField label="Y" value={xyz[1]} onChange={setAxis(1)} min={-1e9} />
            <LengthField label="Z" value={xyz[2]} onChange={setAxis(2)} min={-1e9} />
          </>
        )}
      </Section>
    </PanelShell>
  )
}

export function CoordSysPanel({ panel }: { panel: Panel }) {
  const init = useInit(panel, { origin: firstSelected(['vertex']) ?? { type: 'origin' } })
  const slots = useSlots(init)
  const list: Slot[] = [
    { key: 'origin', label: 'Origin', kinds: POINT, origin: true },
    { key: 'x_axis', label: 'X axis (optional)', kinds: LINE },
    { key: 'z_axis', label: 'Z axis (optional)', kinds: LINE },
  ]
  const r = slots.refs
  const params: FeatureParams | null = r['origin']
    ? {
        type: 'coord_sys',
        origin: r['origin'],
        x_axis: r['x_axis'] ?? null,
        z_axis: r['z_axis'] ?? null,
      }
    : null
  const f = useDatum(panel, params)
  return (
    <PanelShell title="Coordinate System" {...f}>
      <Section title="Selections">{slots.render(list)}</Section>
    </PanelShell>
  )
}

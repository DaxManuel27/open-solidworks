import { useEffect, useState } from 'react'
import { call } from '../core'
import type { EndCondition, FeatureParams, FeatureView, Ref, SketchView, Thin } from '../protocol'
import { type Panel, type PickKind, useStore } from '../store'
import { formatLength } from '../units'
import { BodyScope } from './bodies'
import {
  AngleField,
  Check,
  Choice,
  IntField,
  LengthField,
  PanelShell,
  RefBox,
  Section,
  defaultSketch,
  usePreview,
  useSketches,
} from './fields'

const SURFACE: PickKind[] = ['face', 'datum', 'std_plane']
const DIRECTION: PickKind[] = ['edge', 'face', 'datum', 'std_plane']
const AXIS: PickKind[] = ['edge', 'face', 'datum']
const PATTERNABLE = new Set([
  'extrude',
  'revolve',
  'sweep',
  'loft',
  'draft',
  'variable_fillet',
  'fillet',
  'chamfer',
  'shell',
  'hole',
  'linear_pattern',
  'circular_pattern',
  'mirror',
  'import',
])

/** The edited feature's parameters, or defaults for a new one. */
function useParams<T>(panel: Panel, defaults: () => T): [T, (p: T) => void] {
  return useState<T>(() => {
    const f = useStore.getState().snapshot?.features.find((x) => x.id === panel.editing)
    return f ? (f.params as unknown as T) : defaults()
  })
}

/** Shared OK/Cancel/preview wiring for feature panels. */
export function useFeature(panel: Panel, params: FeatureParams | null) {
  const supportsScope = params && PATTERNABLE.has(params.type) && params.type !== 'import'
  const [scope, setScope] = useState<string[] | null>(
    () =>
      (useStore.getState().snapshot?.features.find((f) => f.id === panel.editing)?.params[
        'scope'
      ] as string[] | null) ?? null,
  )
  const scoped = params ? { ...params, scope } : null
  const { error } = usePreview(scoped, panel.editing)
  const commit = useStore((s) => s.commit)
  const close = useStore((s) => s.closePanel)
  return {
    error,
    onOk: () => {
      if (scoped) void commit(scoped, panel.editing)
    },
    onCancel: () => void close(),
    okDisabled: !params,
    footer: supportsScope ? <BodyScope value={scope} onChange={setScope} /> : undefined,
  }
}

function useNewSketch(panel: Panel): number | null {
  const s = useStore.getState()
  return panel.sketch ?? defaultSketch(s.snapshot?.features ?? [], s.treeSelection)
}

function SketchChoice(props: { value: number | null; onChange: (id: number) => void }) {
  const sketches = useSketches()
  return (
    <Choice
      label="Sketch"
      value={props.value ?? 0}
      options={[
        ...(props.value === null ? [{ value: 0, label: 'Choose a sketch…' }] : []),
        ...sketches.map((s) => ({ value: s.id, label: s.name })),
      ]}
      onChange={(v) => v && props.onChange(v)}
    />
  )
}

function ThinEditor(props: { value: Thin | null; onChange: (t: Thin | null) => void }) {
  const t = props.value
  return (
    <Section title="Thin Feature" open={!!t}>
      <Check
        label="Thin feature"
        value={!!t}
        onChange={(on) => props.onChange(on ? { kind: 'one_direction', thickness: 1 } : null)}
      />
      {t && (
        <>
          <Choice
            label="Type"
            value={t.kind}
            options={[
              { value: 'one_direction', label: 'One-Direction' },
              { value: 'mid_plane', label: 'Mid-Plane' },
              { value: 'two_direction', label: 'Two-Direction' },
            ]}
            onChange={(kind) =>
              props.onChange({ ...t, kind, thickness2: t.thickness2 ?? t.thickness })
            }
          />
          <LengthField
            label="Thickness"
            value={t.thickness}
            onChange={(v) => props.onChange({ ...t, thickness: v })}
          />
          {t.kind === 'two_direction' && (
            <LengthField
              label="Thickness 2"
              value={t.thickness2 ?? t.thickness}
              onChange={(v) => props.onChange({ ...t, thickness2: v })}
            />
          )}
          {t.kind === 'one_direction' && (
            <Check
              label="Reverse direction"
              value={!!t.reverse}
              onChange={(reverse) => props.onChange({ ...t, reverse })}
            />
          )}
        </>
      )}
    </Section>
  )
}

/** An end condition while being edited: targets may not be picked yet. */
type EndDraft = {
  type: EndCondition['type']
  depth: number
  target: Ref | null
  offset: number
  reverse_offset: boolean
}

function toDraft(e: EndCondition | null | undefined): EndDraft {
  const d: EndDraft = { type: 'blind', depth: 10, target: null, offset: 1, reverse_offset: false }
  if (!e) return d
  return { ...d, ...e } as EndDraft
}

function fromDraft(d: EndDraft): EndCondition | null {
  switch (d.type) {
    case 'blind':
    case 'mid_plane':
      return d.depth > 0 ? { type: d.type, depth: d.depth } : null
    case 'through_all':
    case 'up_to_next':
      return { type: d.type }
    case 'up_to_vertex':
    case 'up_to_surface':
      return d.target ? { type: d.type, target: d.target } : null
    case 'offset_from_surface':
      return d.target
        ? { type: d.type, target: d.target, offset: d.offset, reverse_offset: d.reverse_offset }
        : null
  }
}

function EndEditor(props: {
  value: EndDraft
  onChange: (d: EndDraft) => void
  allowMid: boolean
  active: boolean
  onActivate: () => void
}) {
  const d = props.value
  const set = (patch: Partial<EndDraft>) => props.onChange({ ...d, ...patch })
  const options: { value: EndCondition['type']; label: string }[] = [
    { value: 'blind', label: 'Blind' },
    { value: 'through_all', label: 'Through All' },
    { value: 'up_to_next', label: 'Up To Next' },
    { value: 'up_to_vertex', label: 'Up To Vertex' },
    { value: 'up_to_surface', label: 'Up To Surface' },
    { value: 'offset_from_surface', label: 'Offset From Surface' },
  ]
  if (props.allowMid) options.push({ value: 'mid_plane', label: 'Mid Plane' })
  return (
    <>
      <Choice
        label="End condition"
        value={d.type}
        options={options}
        onChange={(type) => set({ type, target: null })}
      />
      {(d.type === 'blind' || d.type === 'mid_plane') && (
        <LengthField label="Depth" value={d.depth} onChange={(depth) => set({ depth })} />
      )}
      {(d.type === 'up_to_vertex' ||
        d.type === 'up_to_surface' ||
        d.type === 'offset_from_surface') && (
        <RefBox
          label={d.type === 'up_to_vertex' ? 'Vertex' : 'Face or plane'}
          refs={d.target ? [d.target] : []}
          onChange={(r) => set({ target: r[0] ?? null })}
          kinds={d.type === 'up_to_vertex' ? ['vertex'] : SURFACE}
          single
          active={props.active}
          onActivate={props.onActivate}
        />
      )}
      {d.type === 'offset_from_surface' && (
        <>
          <LengthField label="Offset" value={d.offset} onChange={(offset) => set({ offset })} />
          <Check
            label="Reverse offset"
            value={d.reverse_offset}
            onChange={(reverse_offset) => set({ reverse_offset })}
          />
        </>
      )}
    </>
  )
}

export function ExtrudePanel({ panel }: { panel: Panel }) {
  const cut = panel.type === 'cut'
  const newSketch = useNewSketch(panel)
  const [init] = useParams<FeatureParams>(panel, () => ({ type: 'extrude' }))
  const [sketch, setSketch] = useState<number | null>((init['sketch'] as number) ?? newSketch)
  const [end1, setEnd1] = useState(toDraft(init['end1'] as EndCondition))
  const [end2, setEnd2] = useState<EndDraft | null>(
    init['end2'] ? toDraft(init['end2'] as EndCondition) : null,
  )
  const [reverse, setReverse] = useState(!!init['reverse'])
  const [startOffset, setStartOffset] = useState((init['start_offset'] as number) ?? 0)
  const [thin, setThin] = useState<Thin | null>((init['thin'] as Thin) ?? null)
  const [flip, setFlip] = useState(!!init['flip_side_to_cut'])
  const [merge, setMerge] = useState((init['merge'] as boolean) ?? true)
  const [active, setActive] = useState<'end1' | 'end2'>('end1')

  const e1 = fromDraft(end1)
  const e2 = end2 ? fromDraft(end2) : null
  const params: FeatureParams | null =
    sketch !== null && e1 && (!end2 || e2)
      ? {
          type: 'extrude',
          sketch,
          cut,
          start_offset: startOffset,
          end1: e1,
          reverse,
          end2: e2,
          thin,
          flip_side_to_cut: cut && flip,
          merge,
        }
      : null
  const f = useFeature(panel, params)
  return (
    <PanelShell title={cut ? 'Cut-Extrude' : 'Boss-Extrude'} {...f}>
      <Section title="From">
        <SketchChoice value={sketch} onChange={setSketch} />
        <LengthField
          label="Start offset"
          value={startOffset}
          onChange={setStartOffset}
          min={-1e9}
        />
      </Section>
      <Section title="Direction 1">
        <EndEditor
          value={end1}
          onChange={setEnd1}
          allowMid={!end2}
          active={active === 'end1'}
          onActivate={() => setActive('end1')}
        />
        <Check label="Reverse direction" value={reverse} onChange={setReverse} />
        {cut && <Check label="Flip side to cut" value={flip} onChange={setFlip} />}
      </Section>
      <Section title="Direction 2" open={!!end2}>
        <Check
          label="Direction 2"
          value={!!end2}
          onChange={(on) => setEnd2(on ? toDraft(null) : null)}
        />
        {end2 && (
          <EndEditor
            value={end2}
            onChange={setEnd2}
            allowMid={false}
            active={active === 'end2'}
            onActivate={() => setActive('end2')}
          />
        )}
      </Section>
      <ThinEditor value={thin} onChange={setThin} />
      {!cut && (
        <Section title="Options">
          <Check label="Merge result" value={merge} onChange={setMerge} />
        </Section>
      )}
    </PanelShell>
  )
}

/** Lines of a sketch, for choosing a revolve axis. */
function useSketchLines(sketch: number | null): { id: number; label: string }[] {
  const [lines, setLines] = useState<{ id: number; label: string }[]>([])
  useEffect(() => {
    if (sketch === null) return
    let live = true
    void call<SketchView>('sketch.get', { id: sketch })
      .then((v) => {
        if (!live) return
        const ls = v.curves.filter((c) => c.type === 'line')
        // Centerlines first: they're the usual revolve axis.
        ls.sort((a, b) => Number(b.construction) - Number(a.construction))
        setLines(
          ls.map((c) => ({ id: c.id, label: `${c.construction ? 'Centerline' : 'Line'} ${c.id}` })),
        )
      })
      .catch(() => live && setLines([]))
    return () => {
      live = false
    }
  }, [sketch])
  return sketch === null ? [] : lines
}

interface RevolveEndDraft {
  type: string
  angle: number
  target: Ref | null
  offset: number
  reverse_offset: boolean
}

function revolveEndDraft(value: unknown): RevolveEndDraft {
  return {
    type: 'blind',
    angle: Math.PI / 2,
    target: null,
    offset: 1,
    reverse_offset: false,
    ...(value as Partial<RevolveEndDraft> | null),
  }
}

function revolveEndParams(d: RevolveEndDraft): Record<string, unknown> | null {
  if (d.type === 'blind') return { type: d.type, angle: d.angle }
  if (!d.target) return null
  return {
    type: d.type,
    target: d.target,
    ...(d.type === 'offset_from_surface'
      ? { offset: d.offset, reverse_offset: d.reverse_offset }
      : {}),
  }
}

function RevolveEndEditor({
  value: d,
  onChange,
  active,
  onActivate,
}: {
  value: RevolveEndDraft
  onChange: (value: RevolveEndDraft) => void
  active: boolean
  onActivate: () => void
}) {
  const set = (patch: Partial<RevolveEndDraft>) => onChange({ ...d, ...patch })
  return (
    <>
      <Choice
        label="End condition"
        value={d.type}
        options={[
          { value: 'blind', label: 'Blind' },
          { value: 'up_to_vertex', label: 'Up To Vertex' },
          { value: 'up_to_surface', label: 'Up To Surface' },
          { value: 'offset_from_surface', label: 'Offset From Surface' },
        ]}
        onChange={(type) => {
          set({ type, target: null })
          onActivate()
        }}
      />
      {d.type === 'blind' ? (
        <AngleField label="Angle" value={d.angle} onChange={(angle) => set({ angle })} />
      ) : (
        <RefBox
          label={d.type === 'up_to_vertex' ? 'Vertex or reference point' : 'Face or plane'}
          refs={d.target ? [d.target] : []}
          onChange={(refs) => set({ target: refs[0] ?? null })}
          kinds={d.type === 'up_to_vertex' ? ['vertex', 'datum'] : SURFACE}
          single
          active={active}
          onActivate={onActivate}
        />
      )}
      {d.type === 'offset_from_surface' && (
        <>
          <LengthField
            label="Offset"
            value={d.offset}
            onChange={(offset) => set({ offset })}
            min={0}
          />
          <Check
            label="Reverse offset"
            value={d.reverse_offset}
            onChange={(reverse_offset) => set({ reverse_offset })}
          />
        </>
      )}
    </>
  )
}

export function RevolvePanel({ panel }: { panel: Panel }) {
  const cut = panel.type === 'cut_revolve'
  const newSketch = useNewSketch(panel)
  const [init] = useParams<FeatureParams>(panel, () => ({ type: 'revolve' }))
  const [sketch, setSketch] = useState<number | null>((init['sketch'] as number) ?? newSketch)
  // undefined: not chosen yet.
  const [chosenAxis, setAxis] = useState<Ref | null | undefined>(init['axis'] as Ref | undefined)
  const initAngle = (init['angle'] as {
    type: string
    angle?: number
    angle1?: number
    angle2?: number
    target?: Ref
    offset?: number
    reverse_offset?: boolean
  }) ?? {
    type: 'blind',
    angle: 2 * Math.PI,
  }
  const [mode, setMode] = useState(initAngle.type)
  const [angle1, setAngle1] = useState(initAngle.angle ?? initAngle.angle1 ?? 2 * Math.PI)
  const [angle2, setAngle2] = useState(initAngle.angle2 ?? Math.PI / 2)
  const [target, setTarget] = useState<Ref | null>(initAngle.target ?? null)
  const [offset, setOffset] = useState(initAngle.offset ?? 1)
  const [reverseOffset, setReverseOffset] = useState(initAngle.reverse_offset ?? false)
  const [pickSlot, setPickSlot] = useState<'axis' | 'target' | 'target2'>(
    initAngle.target ? 'target' : 'axis',
  )
  const [second, setSecond] = useState(!!init['end2'])
  const [secondEnd, setSecondEnd] = useState<RevolveEndDraft>(() => revolveEndDraft(init['end2']))
  const [reverse, setReverse] = useState(!!init['reverse'])
  const [thin, setThin] = useState<Thin | null>((init['thin'] as Thin) ?? null)
  const [merge, setMerge] = useState((init['merge'] as boolean) ?? true)
  const lines = useSketchLines(sketch)

  // Until one is chosen, use the sketch's first centerline.
  const axis: Ref | null =
    chosenAxis !== undefined
      ? chosenAxis
      : sketch !== null && lines[0]
        ? { type: 'sketch_entity', sketch, entity: lines[0].id }
        : null

  const needsTarget = ['up_to_vertex', 'up_to_surface', 'offset_from_surface'].includes(mode)
  const angle = needsTarget
    ? {
        type: mode,
        target,
        ...(mode === 'offset_from_surface' ? { offset, reverse_offset: reverseOffset } : {}),
      }
    : mode === 'two_direction'
      ? { type: mode, angle1, angle2 }
      : { type: mode, angle: angle1 }
  const params: FeatureParams | null =
    sketch !== null && axis && (!needsTarget || target) && (!second || revolveEndParams(secondEnd))
      ? {
          type: 'revolve',
          sketch,
          axis,
          angle,
          end2: second ? revolveEndParams(secondEnd) : null,
          reverse,
          cut,
          thin,
          merge,
        }
      : null
  const f = useFeature(panel, params)
  const sketchAxis = axis?.type === 'sketch_entity' && axis.sketch === sketch ? axis.entity : 0
  return (
    <PanelShell title={cut ? 'Cut-Revolve' : 'Revolve'} {...f}>
      <Section title="Axis of Revolution">
        <SketchChoice
          value={sketch}
          onChange={(s) => {
            setSketch(s)
            setAxis(null)
          }}
        />
        <Choice
          label="Sketch line"
          value={sketchAxis}
          options={[
            { value: 0, label: sketchAxis ? '—' : 'Picked axis or edge' },
            ...lines.map((l) => ({ value: l.id, label: l.label })),
          ]}
          onChange={(id) =>
            sketch !== null && setAxis(id ? { type: 'sketch_entity', sketch, entity: id } : null)
          }
        />
        <RefBox
          label="Or an axis or straight edge"
          refs={axis && axis.type !== 'sketch_entity' ? [axis] : []}
          onChange={(r) => setAxis(r[0] ?? null)}
          kinds={AXIS}
          single
          active={pickSlot === 'axis'}
          onActivate={() => setPickSlot('axis')}
        />
      </Section>
      <Section title="Direction">
        <Choice
          label="Type"
          value={mode}
          options={[
            { value: 'blind', label: 'Blind' },
            { value: 'mid_plane', label: 'Mid-Plane' },
            { value: 'two_direction', label: 'Two-Direction' },
            { value: 'up_to_vertex', label: 'Up To Vertex' },
            { value: 'up_to_surface', label: 'Up To Surface' },
            { value: 'offset_from_surface', label: 'Offset From Surface' },
          ]}
          onChange={(mode) => {
            setMode(mode)
            if (mode === 'mid_plane' || mode === 'two_direction') setSecond(false)
            setTarget(null)
            setPickSlot(
              ['up_to_vertex', 'up_to_surface', 'offset_from_surface'].includes(mode)
                ? 'target'
                : 'axis',
            )
          }}
        />
        {!needsTarget && (
          <AngleField
            label={mode === 'two_direction' ? 'Angle 1' : 'Angle'}
            value={angle1}
            onChange={setAngle1}
          />
        )}
        {needsTarget && (
          <RefBox
            label={mode === 'up_to_vertex' ? 'Vertex or reference point' : 'Face or plane'}
            refs={target ? [target] : []}
            onChange={(refs) => setTarget(refs[0] ?? null)}
            kinds={mode === 'up_to_vertex' ? ['vertex', 'datum'] : SURFACE}
            single
            active={pickSlot === 'target'}
            onActivate={() => setPickSlot('target')}
          />
        )}
        {mode === 'offset_from_surface' && (
          <>
            <LengthField label="Offset" value={offset} onChange={setOffset} min={0} />
            <Check label="Reverse offset" value={reverseOffset} onChange={setReverseOffset} />
          </>
        )}
        {mode === 'two_direction' && (
          <AngleField label="Angle 2" value={angle2} onChange={setAngle2} />
        )}
        <Check label="Reverse direction" value={reverse} onChange={setReverse} />
      </Section>
      {mode !== 'mid_plane' && mode !== 'two_direction' && (
        <Section title="Direction 2">
          <Check label="Enable second direction" value={second} onChange={setSecond} />
          {second && (
            <RevolveEndEditor
              value={secondEnd}
              onChange={setSecondEnd}
              active={pickSlot === 'target2'}
              onActivate={() => setPickSlot('target2')}
            />
          )}
        </Section>
      )}
      <ThinEditor value={thin} onChange={setThin} />
      {!cut && (
        <Section title="Options">
          <Check label="Merge result" value={merge} onChange={setMerge} />
        </Section>
      )}
    </PanelShell>
  )
}

/** Faces and edges from the general selection, to prefill edge-based panels. */
function selectedRefs(kinds: string[]): Ref[] {
  return useStore
    .getState()
    .selection.map((s) => s.ref)
    .filter((r) => kinds.includes(r.type))
}

function splitEdgesFaces(refs: Ref[]): { edges: Ref[]; faces: Ref[] } {
  return {
    edges: refs.filter(
      (r) => r.type === 'edge' || (r.type === 'body_topology' && r.kind === 'edge'),
    ),
    faces: refs.filter(
      (r) => r.type === 'face' || (r.type === 'body_topology' && r.kind === 'face'),
    ),
  }
}

export function FilletPanel({ panel }: { panel: Panel }) {
  const [init] = useParams<FeatureParams>(panel, () => ({
    type: 'fillet',
    ...splitEdgesFaces(selectedRefs(['edge', 'face'])),
    radius: 2,
  }))
  const [refs, setRefs] = useState<Ref[]>([
    ...((init['edges'] as Ref[]) ?? []),
    ...((init['faces'] as Ref[]) ?? []),
  ])
  const [radius, setRadius] = useState((init['radius'] as number) ?? 2)
  const params: FeatureParams | null =
    refs.length && radius > 0 ? { type: 'fillet', ...splitEdgesFaces(refs), radius } : null
  const f = useFeature(panel, params)
  return (
    <PanelShell title="Fillet" {...f}>
      <Section title="Items to Fillet">
        <LengthField label="Radius" value={radius} onChange={setRadius} />
        <RefBox
          label="Edges and faces"
          refs={refs}
          onChange={setRefs}
          kinds={['edge', 'face']}
          active
          onActivate={() => {}}
        />
      </Section>
    </PanelShell>
  )
}

type ChamferMode =
  | { type: 'equal'; distance: number }
  | { type: 'distance_distance'; d1: number; d2: number }
  | { type: 'angle_distance'; distance: number; angle: number }

export function ChamferPanel({ panel }: { panel: Panel }) {
  const [init] = useParams<FeatureParams>(panel, () => ({
    type: 'chamfer',
    ...splitEdgesFaces(selectedRefs(['edge', 'face'])),
    mode: { type: 'equal', distance: 1 },
  }))
  const [refs, setRefs] = useState<Ref[]>([
    ...((init['edges'] as Ref[]) ?? []),
    ...((init['faces'] as Ref[]) ?? []),
  ])
  const m0 = init['mode'] as ChamferMode
  const [mode, setMode] = useState<ChamferMode['type']>(m0.type)
  const [d1, setD1] = useState('distance' in m0 ? m0.distance : m0.d1)
  const [d2, setD2] = useState(m0.type === 'distance_distance' ? m0.d2 : d1)
  const [angle, setAngle] = useState(m0.type === 'angle_distance' ? m0.angle : Math.PI / 4)
  const [flip, setFlip] = useState(!!init['flip'])
  const modeParams: ChamferMode =
    mode === 'equal'
      ? { type: mode, distance: d1 }
      : mode === 'distance_distance'
        ? { type: mode, d1, d2 }
        : { type: mode, distance: d1, angle }
  const params: FeatureParams | null = refs.length
    ? { type: 'chamfer', ...splitEdgesFaces(refs), mode: modeParams, flip }
    : null
  const f = useFeature(panel, params)
  return (
    <PanelShell title="Chamfer" {...f}>
      <Section title="Chamfer Parameters">
        <Choice
          label="Type"
          value={mode}
          options={[
            { value: 'equal', label: 'Equal distance' },
            { value: 'distance_distance', label: 'Distance–distance' },
            { value: 'angle_distance', label: 'Angle–distance' },
          ]}
          onChange={setMode}
        />
        <LengthField
          label={mode === 'distance_distance' ? 'Distance 1' : 'Distance'}
          value={d1}
          onChange={setD1}
        />
        {mode === 'distance_distance' && (
          <LengthField label="Distance 2" value={d2} onChange={setD2} />
        )}
        {mode === 'angle_distance' && (
          <AngleField label="Angle" value={angle} onChange={setAngle} />
        )}
        {mode !== 'equal' && <Check label="Flip direction" value={flip} onChange={setFlip} />}
        <RefBox
          label="Edges and faces"
          refs={refs}
          onChange={setRefs}
          kinds={['edge', 'face']}
          active
          onActivate={() => {}}
        />
      </Section>
    </PanelShell>
  )
}

export function ShellPanel({ panel }: { panel: Panel }) {
  const [init] = useParams<FeatureParams>(panel, () => ({
    type: 'shell',
    faces: selectedRefs(['face']),
    thickness: 1,
  }))
  const [faces, setFaces] = useState<Ref[]>((init['faces'] as Ref[]) ?? [])
  const [thickness, setThickness] = useState((init['thickness'] as number) ?? 1)
  const [outward, setOutward] = useState(!!init['outward'])
  const params: FeatureParams | null =
    thickness > 0 ? { type: 'shell', faces, thickness, outward } : null
  const f = useFeature(panel, params)
  return (
    <PanelShell title="Shell" {...f}>
      <Section title="Parameters">
        <LengthField label="Thickness" value={thickness} onChange={setThickness} />
        <RefBox
          label="Faces to remove (none hollows the part)"
          refs={faces}
          onChange={setFaces}
          kinds={['face']}
          active
          onActivate={() => {}}
        />
        <Check label="Shell outward" value={outward} onChange={setOutward} />
      </Section>
    </PanelShell>
  )
}

interface HoleDims {
  diameter: number
  cbore: [number, number] | null
  csink: [number, number] | null
}

export function HolePanel({ panel }: { panel: Panel }) {
  const newSketch = useNewSketch(panel)
  const units = useStore((s) => s.snapshot!.units)
  const [init] = useParams<FeatureParams>(panel, () => ({
    type: 'hole',
    kind: 'simple',
    standard: units.length === 'in' || units.length === 'ft' ? 'ansi' : 'iso',
    size: units.length === 'in' || units.length === 'ft' ? '1/4-20' : 'M6',
    fit: 'normal',
    end: { type: 'through_all' },
  }))
  const [sketch, setSketch] = useState<number | null>((init['sketch'] as number) ?? newSketch)
  const [kind, setKind] = useState(init['kind'] as string)
  const [standard, setStandard] = useState(init['standard'] as 'iso' | 'ansi')
  const [size, setSize] = useState(init['size'] as string)
  const [fit, setFit] = useState(init['fit'] as string)
  const end0 = init['end'] as { type: string; depth?: number }
  const [endType, setEndType] = useState(end0.type)
  const [depth, setDepth] = useState(end0.depth ?? 10)
  const [reverse, setReverse] = useState(!!init['reverse'])
  const [sizes, setSizes] = useState<{ iso: string[]; ansi: string[] }>({ iso: [], ansi: [] })
  const [dims, setDims] = useState<HoleDims | null>(null)

  useEffect(() => {
    void call<{ iso: string[]; ansi: string[] }>('hole.sizes').then(setSizes)
  }, [])

  const end = endType === 'blind' ? { type: 'blind', depth } : { type: 'through_all' }
  const params: FeatureParams | null =
    sketch !== null ? { type: 'hole', sketch, kind, standard, size, fit, end, reverse } : null
  const json = JSON.stringify(params)
  useEffect(() => {
    if (json === 'null') return
    let live = true
    call<HoleDims>('hole.dims', JSON.parse(json))
      .then((d) => live && setDims(d))
      .catch(() => live && setDims(null))
    return () => {
      live = false
    }
  }, [json])

  const f = useFeature(panel, params)
  return (
    <PanelShell title="Hole" {...f}>
      <Section title="Position">
        <SketchChoice value={sketch} onChange={setSketch} />
        <p className="hint">A hole is made at each point of the sketch, drilled into its plane.</p>
      </Section>
      <Section title="Hole Type">
        <Choice
          label="Type"
          value={kind}
          options={[
            { value: 'simple', label: 'Hole' },
            { value: 'counterbore', label: 'Counterbore' },
            { value: 'countersink', label: 'Countersink' },
            { value: 'tapped', label: 'Tapped hole' },
          ]}
          onChange={setKind}
        />
        <Choice
          label="Standard"
          value={standard}
          options={[
            { value: 'iso', label: 'ISO (metric)' },
            { value: 'ansi', label: 'ANSI (inch)' },
          ]}
          onChange={(s) => {
            setStandard(s)
            setSize(s === 'iso' ? 'M6' : '1/4-20')
          }}
        />
        <Choice
          label="Size"
          value={size}
          options={sizes[standard].map((s) => ({ value: s, label: s }))}
          onChange={setSize}
        />
        {kind !== 'tapped' && (
          <Choice
            label="Fit"
            value={fit}
            options={[
              { value: 'close', label: 'Close' },
              { value: 'normal', label: 'Normal' },
              { value: 'loose', label: 'Loose' },
            ]}
            onChange={setFit}
          />
        )}
        {dims && (
          <p className="hint">
            ⌀ {formatLength(dims.diameter, units)}
            {dims.cbore &&
              ` · C'bore ⌀ ${formatLength(dims.cbore[0], units)} × ${formatLength(dims.cbore[1], units)}`}
            {dims.csink && ` · C'sink ⌀ ${formatLength(dims.csink[0], units)}`}
          </p>
        )}
      </Section>
      <Section title="End Condition">
        <Choice
          label="End condition"
          value={endType}
          options={[
            { value: 'through_all', label: 'Through All' },
            { value: 'blind', label: 'Blind' },
          ]}
          onChange={setEndType}
        />
        {endType === 'blind' && <LengthField label="Depth" value={depth} onChange={setDepth} />}
        <Check label="Reverse direction" value={reverse} onChange={setReverse} />
      </Section>
    </PanelShell>
  )
}

/** Features that can be patterned or mirrored, before the one being edited. */
function FeatureList(props: {
  value: number[]
  onChange: (ids: number[]) => void
  editing?: number
  label: string
}) {
  const features = useStore((s) => s.snapshot?.features ?? [])
  const stop =
    props.editing !== undefined
      ? features.findIndex((f) => f.id === props.editing)
      : features.length
  const eligible = features
    .slice(0, stop < 0 ? features.length : stop)
    .filter((f) => PATTERNABLE.has(f.type) && !f.rolled_back)
  return (
    <div className="feature-list">
      <div className="refbox-label">{props.label}</div>
      {eligible.length === 0 && <div className="refbox-empty">No features to choose yet</div>}
      {eligible.map((f) => (
        <Check
          key={f.id}
          label={f.name}
          value={props.value.includes(f.id)}
          onChange={(on) =>
            props.onChange(on ? [...props.value, f.id] : props.value.filter((x) => x !== f.id))
          }
        />
      ))}
    </div>
  )
}

function selectedFeatures(features: FeatureView[]): number[] {
  const t = useStore.getState().treeSelection
  return features.some((f) => f.id === t && PATTERNABLE.has(f.type)) ? [t!] : []
}

interface DirDraft {
  reference: Ref | null
  spacing: number
  count: number
  reverse: boolean
}

function DirectionEditor(props: {
  value: DirDraft
  onChange: (d: DirDraft) => void
  active: boolean
  onActivate: () => void
}) {
  const d = props.value
  return (
    <>
      <RefBox
        label="Direction (edge, axis or plane)"
        refs={d.reference ? [d.reference] : []}
        onChange={(r) => props.onChange({ ...d, reference: r[0] ?? null })}
        kinds={DIRECTION}
        single
        active={props.active}
        onActivate={props.onActivate}
      />
      <Check
        label="Reverse direction"
        value={d.reverse}
        onChange={(reverse) => props.onChange({ ...d, reverse })}
      />
      <LengthField
        label="Spacing"
        value={d.spacing}
        onChange={(spacing) => props.onChange({ ...d, spacing })}
      />
      <IntField
        label="Instances"
        value={d.count}
        onChange={(count) => props.onChange({ ...d, count })}
      />
    </>
  )
}

export function LinearPatternPanel({ panel }: { panel: Panel }) {
  const features = useStore((s) => s.snapshot?.features ?? [])
  const [init] = useParams<FeatureParams>(panel, () => ({
    type: 'linear_pattern',
    features: selectedFeatures(features),
  }))
  const blank: DirDraft = { reference: null, spacing: 10, count: 2, reverse: false }
  const [ids, setIds] = useState<number[]>((init['features'] as number[]) ?? [])
  const [d1, setD1] = useState<DirDraft>((init['direction1'] as DirDraft) ?? blank)
  const [d2, setD2] = useState<DirDraft | null>((init['direction2'] as DirDraft) ?? null)
  const [active, setActive] = useState<1 | 2>(1)
  const ok = (d: DirDraft) => !!d.reference && d.count >= 1
  const params: FeatureParams | null =
    ids.length && ok(d1) && (!d2 || ok(d2))
      ? { type: 'linear_pattern', features: ids, direction1: d1, direction2: d2 }
      : null
  const f = useFeature(panel, params)
  return (
    <PanelShell title="Linear Pattern" {...f}>
      <Section title="Direction 1">
        <DirectionEditor
          value={d1}
          onChange={setD1}
          active={active === 1}
          onActivate={() => setActive(1)}
        />
      </Section>
      <Section title="Direction 2" open={!!d2}>
        <Check
          label="Direction 2"
          value={!!d2}
          onChange={(on) => setD2(on ? { ...blank } : null)}
        />
        {d2 && (
          <DirectionEditor
            value={d2}
            onChange={setD2}
            active={active === 2}
            onActivate={() => setActive(2)}
          />
        )}
      </Section>
      <Section title="Features to Pattern">
        <FeatureList label="Features" value={ids} onChange={setIds} editing={panel.editing} />
      </Section>
    </PanelShell>
  )
}

export function CircularPatternPanel({ panel }: { panel: Panel }) {
  const features = useStore((s) => s.snapshot?.features ?? [])
  const [init] = useParams<FeatureParams>(panel, () => ({
    type: 'circular_pattern',
    features: selectedFeatures(features),
    axis: null,
    count: 4,
    angle: 2 * Math.PI,
    equal_spacing: true,
    reverse: false,
  }))
  const [ids, setIds] = useState<number[]>((init['features'] as number[]) ?? [])
  // undefined: not chosen yet.
  const [axis, setAxis] = useState<Ref | null>((init['axis'] as Ref) ?? null)
  const [count, setCount] = useState(init['count'] as number)
  const [angle, setAngle] = useState(init['angle'] as number)
  const [equal, setEqual] = useState(init['equal_spacing'] as boolean)
  const [reverse, setReverse] = useState(!!init['reverse'])
  const params: FeatureParams | null =
    ids.length && axis
      ? {
          type: 'circular_pattern',
          features: ids,
          axis,
          count,
          angle,
          equal_spacing: equal,
          reverse,
        }
      : null
  const f = useFeature(panel, params)
  return (
    <PanelShell title="Circular Pattern" {...f}>
      <Section title="Parameters">
        <RefBox
          label="Axis (axis, round face or edge)"
          refs={axis ? [axis] : []}
          onChange={(r) => setAxis(r[0] ?? null)}
          kinds={AXIS}
          single
          active
          onActivate={() => {}}
        />
        <Check label="Reverse direction" value={reverse} onChange={setReverse} />
        <Check label="Equal spacing" value={equal} onChange={setEqual} />
        <AngleField
          label={equal ? 'Total angle' : 'Angle between'}
          value={angle}
          onChange={setAngle}
        />
        <IntField label="Instances" value={count} onChange={setCount} />
      </Section>
      <Section title="Features to Pattern">
        <FeatureList label="Features" value={ids} onChange={setIds} editing={panel.editing} />
      </Section>
    </PanelShell>
  )
}

export function MirrorPanel({ panel }: { panel: Panel }) {
  const features = useStore((s) => s.snapshot?.features ?? [])
  const [init] = useParams<FeatureParams>(panel, () => ({
    type: 'mirror',
    features: selectedFeatures(features),
    plane: selectedRefs(['face', 'front_plane', 'top_plane', 'right_plane', 'feature'])[0] ?? null,
  }))
  const [ids, setIds] = useState<number[]>((init['features'] as number[]) ?? [])
  const [plane, setPlane] = useState<Ref | null>((init['plane'] as Ref) ?? null)
  const params: FeatureParams | null = plane ? { type: 'mirror', features: ids, plane } : null
  const f = useFeature(panel, params)
  return (
    <PanelShell title="Mirror" {...f}>
      <Section title="Mirror Face/Plane">
        <RefBox
          label="Plane or planar face"
          refs={plane ? [plane] : []}
          onChange={(r) => setPlane(r[0] ?? null)}
          kinds={SURFACE}
          single
          active
          onActivate={() => {}}
        />
      </Section>
      <Section title="Features to Mirror">
        <FeatureList
          label="Features (none mirrors the whole body)"
          value={ids}
          onChange={setIds}
          editing={panel.editing}
        />
      </Section>
    </PanelShell>
  )
}

import { useCallback, useEffect, useMemo, useState } from 'react'
import { call } from '../core'
import type { AsmInstance, MateKind } from '../protocol'
import { type Panel, useStore } from '../store'
import { AngleField, Check, Choice, LengthField, NumberField, PanelShell, Section } from './fields'
import { AsmRefBox, closeAsmPanel, commitAsm, geometryKind, type Picked } from './asm-common'

type Group = 'standard' | 'advanced' | 'mechanical'

const TYPES: Record<Group, { type: string; label: string }[]> = {
  standard: [
    { type: 'coincident', label: 'Coincident' },
    { type: 'parallel', label: 'Parallel' },
    { type: 'perpendicular', label: 'Perpendicular' },
    { type: 'tangent', label: 'Tangent' },
    { type: 'concentric', label: 'Concentric' },
    { type: 'lock', label: 'Lock' },
    { type: 'distance', label: 'Distance' },
    { type: 'angle', label: 'Angle' },
  ],
  advanced: [
    { type: 'profile_center', label: 'Profile Center' },
    { type: 'symmetric', label: 'Symmetric' },
    { type: 'width', label: 'Width' },
    { type: 'path', label: 'Path Mate' },
    { type: 'linear_coupler', label: 'Linear/Linear Coupler' },
    { type: 'distance_limit', label: 'Distance Limit' },
    { type: 'angle_limit', label: 'Angle Limit' },
  ],
  mechanical: [
    { type: 'cam', label: 'Cam' },
    { type: 'slot', label: 'Slot' },
    { type: 'hinge', label: 'Hinge' },
    { type: 'gear', label: 'Gear' },
    { type: 'rack_pinion', label: 'Rack Pinion' },
    { type: 'screw', label: 'Screw' },
    { type: 'universal_joint', label: 'Universal Joint' },
  ],
}

const AXIS = ['line', 'cylinder', 'cone', 'circle']
const DIRECTION = ['plane', ...AXIS]
const CENTER = ['point', 'sphere', 'circle']

/** Standard mate types that suit two picked entities (the quick-mates rule). */
export function fits(type: string, a: string | undefined, b: string | undefined): boolean {
  if (!a || !b) return true
  const pair = (x: string[], y: string[]) =>
    (x.includes(a) && y.includes(b)) || (x.includes(b) && y.includes(a))
  switch (type) {
    case 'coincident':
      return (
        pair(['plane'], ['plane', 'point', 'line', 'circle']) ||
        pair(['point'], ['point', 'line', 'cylinder', 'sphere', 'circle', 'curve']) ||
        pair(['line'], ['line']) ||
        pair(['circle'], ['circle'])
      )
    case 'parallel':
    case 'perpendicular':
    case 'angle':
    case 'angle_limit':
      return DIRECTION.includes(a) && DIRECTION.includes(b)
    case 'tangent':
      return (
        pair(['plane'], ['cylinder', 'sphere']) ||
        pair(['cylinder', 'sphere'], ['cylinder', 'sphere'])
      )
    case 'concentric':
      return pair(AXIS, AXIS) || pair(AXIS, CENTER) || pair(CENTER, CENTER)
    case 'distance':
    case 'distance_limit':
      return a !== 'curve' && b !== 'curve'
    case 'profile_center':
      return a === 'plane' && b === 'plane'
    default:
      return true
  }
}

const empty: Picked[] = []

/** The Mate PropertyManager: pick entities, choose a type, preview, OK. */
export function MatePanel({ panel }: { panel: Panel }) {
  const assembly = useStore((s) => s.assembly)
  const editing =
    panel.editing !== undefined ? assembly?.mates.find((m) => m.id === panel.editing) : undefined
  const initialType = useMemo(() => {
    const t = editing?.definition.type ?? panel.variant ?? 'coincident'
    if (t === 'distance' && editing?.definition['limit']) return 'distance_limit'
    if (t === 'angle' && editing?.definition['limit']) return 'angle_limit'
    return t
  }, [editing, panel.variant])
  const [type, setType] = useState(initialType)
  const [group, setGroup] = useState<Group>(
    (Object.keys(TYPES) as Group[]).find((g) => TYPES[g].some((t) => t.type === initialType)) ??
      'standard',
  )
  const [boxes, setBoxes] = useState<Record<string, Picked[]>>(() =>
    fromDefinition(editing?.definition),
  )
  const [active, setActive] = useState('a')
  const [alignment, setAlignment] = useState<string>(
    (editing?.definition['alignment'] as string) ?? 'closest',
  )
  const [distance, setDistance] = useState<number>(Number(editing?.definition['distance'] ?? 10))
  const [angle, setAngle] = useState<number>(Number(editing?.definition['angle'] ?? Math.PI / 2))
  const [flip, setFlip] = useState<boolean>(!!editing?.definition['flip'])
  const [lockRotation, setLockRotation] = useState<boolean>(!!editing?.definition['lock_rotation'])
  const limit = editing?.definition['limit'] as { min: number; max: number } | undefined
  const [min, setMin] = useState<number>(limit?.min ?? 0)
  const [max, setMax] = useState<number>(
    limit?.max ?? (initialType === 'angle_limit' ? Math.PI / 2 : 20),
  )
  const [ratioA, setRatioA] = useState<number>(Number(editing?.definition['ratio_a'] ?? 1))
  const [ratioB, setRatioB] = useState<number>(Number(editing?.definition['ratio_b'] ?? 1))
  const [reverse, setReverse] = useState<boolean>(!!editing?.definition['reverse'])
  const [perRev, setPerRev] = useState<number>(
    Number(editing?.definition['distance_per_revolution'] ?? 10),
  )
  const [travel, setTravel] = useState<string>(
    ((editing?.definition['constraint'] as { type?: string } | undefined)?.type as string) ??
      'free',
  )
  const [travelValue, setTravelValue] = useState<number>(0.5)
  const [offset, setOffset] = useState<number>(Number(editing?.definition['offset'] ?? 0))
  const [error, setError] = useState<string | null>(null)

  const set = useCallback(
    (key: string) => (refs: Picked[]) => setBoxes((b) => ({ ...b, [key]: refs })),
    [],
  )
  const a = boxes['a'] ?? empty
  const b = boxes['b'] ?? empty
  const kindA = a[0] ? geometryKind(a[0]) : undefined
  const kindB = b[0] ? geometryKind(b[0]) : undefined

  const mate = buildMate()

  function buildMate(): MateKind | null {
    const r = (key: string, i = 0) => boxes[key]?.[i]?.ref
    const ab = () => (r('a') && r('b') ? { a: r('a')!, b: r('b')! } : null)
    const travelConstraint = () =>
      travel === 'free' || travel === 'center'
        ? { type: travel }
        : travel === 'distance'
          ? { type: 'distance', distance: travelValue }
          : { type: 'percent', percent: travelValue }
    switch (type) {
      case 'coincident':
      case 'parallel':
      case 'tangent':
        return ab() && { type, ...ab()!, alignment }
      case 'perpendicular':
        return ab() && { type, ...ab()! }
      case 'concentric':
        return ab() && { type, ...ab()!, alignment, lock_rotation: lockRotation }
      case 'lock':
        return r('a') && r('b') ? { type, a: r('a')!.component, b: r('b')!.component } : null
      case 'distance':
        return ab() && { type, ...ab()!, distance, flip }
      case 'distance_limit':
        return ab() && { type: 'distance', ...ab()!, distance, flip, limit: { min, max } }
      case 'angle':
        return ab() && { type, ...ab()!, angle, flip }
      case 'angle_limit':
        return ab() && { type: 'angle', ...ab()!, angle, flip, limit: { min, max } }
      case 'profile_center':
        return ab() && { type, ...ab()!, offset, flip, lock_rotation: lockRotation }
      case 'symmetric':
        return ab() && r('plane') ? { type, ...ab()!, plane: r('plane')! } : null
      case 'width': {
        const w = boxes['width'] ?? []
        const t = boxes['tab'] ?? []
        if (w.length !== 2 || t.length !== 2) return null
        const constraint =
          travel === 'distance'
            ? { type: 'dimension', distance: travelValue }
            : travel === 'percent'
              ? { type: 'percent', percent: travelValue }
              : { type: travel === 'free' ? 'free' : 'centered' }
        return { type, width: [w[0]!.ref, w[1]!.ref], tab: [t[0]!.ref, t[1]!.ref], constraint }
      }
      case 'path': {
        const path = (boxes['path'] ?? []).map((p) => p.ref)
        if (!r('point') || !path.length) return null
        return {
          type,
          point: r('point')!,
          path,
          constraint: travelConstraint(),
          ...(r('follow') ? { follow: r('follow')! } : {}),
        }
      }
      case 'linear_coupler':
        return ab() && { type, ...ab()!, ratio_a: ratioA, ratio_b: ratioB, reverse }
      case 'cam':
        return r('cam') && r('follower') ? { type, cam: r('cam')!, follower: r('follower')! } : null
      case 'slot':
        return r('slot') && r('pin')
          ? { type, slot: r('slot')!, pin: r('pin')!, constraint: travelConstraint() }
          : null
      case 'hinge': {
        const axis = boxes['axis'] ?? []
        const faces = boxes['faces'] ?? []
        const limitFaces = boxes['limit'] ?? []
        if (axis.length !== 2 || faces.length !== 2) return null
        return {
          type,
          axis: [axis[0]!.ref, axis[1]!.ref],
          faces: [faces[0]!.ref, faces[1]!.ref],
          ...(limitFaces.length === 2
            ? { limit: { faces: [limitFaces[0]!.ref, limitFaces[1]!.ref], min, max } }
            : {}),
        }
      }
      case 'gear':
        return ab() && { type, ...ab()!, ratio_a: ratioA, ratio_b: ratioB, reverse }
      case 'rack_pinion':
        return r('rack') && r('pinion')
          ? {
              type,
              rack: r('rack')!,
              pinion: r('pinion')!,
              distance_per_revolution: perRev,
              reverse,
            }
          : null
      case 'screw':
        return ab() && { type, ...ab()!, distance_per_revolution: perRev, reverse }
      case 'universal_joint':
        return ab() && { type, ...ab()!, ...(r('joint') ? { joint_point: r('joint')! } : {}) }
    }
    return null
  }

  // Preview the mate's effect while it is complete.
  const json = JSON.stringify(mate)
  const editingId = editing?.id
  useEffect(() => {
    const m = JSON.parse(json) as MateKind | null
    if (!m) {
      useStore.setState({ asmPreview: null })
      return
    }
    if (editingId !== undefined) return
    const t = setTimeout(() => {
      call<{ instances: AsmInstance[] }>('asm.mate.preview', { mate: m })
        .then((r) => {
          useStore.setState({ asmPreview: r.instances })
          setError(null)
        })
        .catch((e: Error) => {
          useStore.setState({ asmPreview: null })
          setError(e.message)
        })
    }, 150)
    return () => clearTimeout(t)
  }, [json, editingId])

  useEffect(() => () => closeAsmPanel(), [])

  const ok = async () => {
    if (!mate) return
    const done = editing
      ? await commitAsm('asm.mate.update', { id: editing.id, mate })
      : await commitAsm('asm.mate.add', { mate })
    if (!done) setError('The mate could not be added; see the message above.')
  }

  const kinds = { faces: ['face', 'edge', 'vertex'] as ('face' | 'edge' | 'vertex')[] }
  const box = (
    key: string,
    label: string,
    single = true,
    k: ('face' | 'edge' | 'vertex' | 'component')[] = kinds.faces,
    next?: string,
  ) => (
    <AsmRefBox
      key={key}
      label={label}
      refs={boxes[key] ?? empty}
      onChange={set(key)}
      kinds={k}
      single={single}
      active={active === key}
      onActivate={() => setActive(key)}
      onFilled={next ? () => setActive(next) : undefined}
    />
  )

  const entities = (() => {
    switch (type) {
      case 'lock':
        return [
          box('a', 'First component', true, ['component'], 'b'),
          box('b', 'Second component', true, ['component']),
        ]
      case 'symmetric':
        return [
          box('a', 'First entity', true, kinds.faces, 'b'),
          box('b', 'Second entity', true, kinds.faces, 'plane'),
          box('plane', 'Symmetry plane', true, ['face']),
        ]
      case 'width':
        return [
          box('width', 'Width faces (2)', false, ['face']),
          box('tab', 'Tab faces (2)', false, ['face']),
        ]
      case 'path':
        return [
          box('point', 'Component vertex', true, ['vertex'], 'path'),
          box('path', 'Path edges', false, ['edge']),
          box('follow', 'Follow with axis (optional)', true, ['edge', 'face']),
        ]
      case 'cam':
        return [
          box('cam', 'Cam face', true, ['face'], 'follower'),
          box('follower', 'Follower (vertex or cylinder)', true, ['vertex', 'face']),
        ]
      case 'slot':
        return [
          box('slot', 'Slot face', true, ['face'], 'pin'),
          box('pin', 'Pin (axis or cylinder)', true, ['face', 'edge']),
        ]
      case 'hinge':
        return [
          box('axis', 'Concentric selections (2)', false, ['face', 'edge']),
          box('faces', 'Coincident selections (2)', false, ['face']),
          box('limit', 'Angle limit faces (optional, 2)', false, ['face']),
        ]
      case 'rack_pinion':
        return [
          box('rack', 'Rack (linear edge)', true, ['edge', 'face'], 'pinion'),
          box('pinion', 'Pinion (cylinder or circle)', true, ['face', 'edge']),
        ]
      case 'universal_joint':
        return [
          box('a', 'Shaft 1 (axis)', true, ['face', 'edge'], 'b'),
          box('b', 'Shaft 2 (axis)', true, ['face', 'edge']),
          box('joint', 'Joint point (optional)', true, ['vertex']),
        ]
      default:
        return [
          box('a', 'First entity', true, kinds.faces, 'b'),
          box('b', 'Second entity', true, kinds.faces),
        ]
    }
  })()

  const alignmentChoice = (
    <Choice
      label="Alignment"
      value={alignment}
      options={[
        { value: 'closest', label: 'Closest' },
        { value: 'aligned', label: 'Aligned' },
        { value: 'anti_aligned', label: 'Anti-aligned' },
      ]}
      onChange={setAlignment}
    />
  )
  const travelChoice = (withCenter: boolean) => (
    <>
      <Choice
        label="Constraint"
        value={travel}
        options={[
          { value: 'free', label: 'Free' },
          ...(withCenter ? [{ value: 'center', label: 'Centered' }] : []),
          { value: 'distance', label: 'Distance along' },
          { value: 'percent', label: 'Percent along' },
        ]}
        onChange={setTravel}
      />
      {travel === 'distance' && (
        <LengthField label="Distance" value={travelValue} onChange={setTravelValue} />
      )}
      {travel === 'percent' && (
        <NumberField
          label="Percent (0–1)"
          value={travelValue}
          min={0}
          onChange={(v) => setTravelValue(Math.min(1, v))}
        />
      )}
    </>
  )

  return (
    <PanelShell
      title={editing ? `Edit ${editing.name}` : 'Mate'}
      onOk={() => void ok()}
      onCancel={closeAsmPanel}
      okDisabled={!mate}
      error={mate ? error : null}
    >
      <Section title="Entities to mate">{entities}</Section>
      <Section title="Mate type">
        <div className="button-grid">
          {(Object.keys(TYPES) as Group[]).map((g) => (
            <button key={g} className={group === g ? 'active' : ''} onClick={() => setGroup(g)}>
              {g[0]!.toUpperCase() + g.slice(1)}
            </button>
          ))}
        </div>
        <div className="button-grid">
          {TYPES[group].map((t) => (
            <button
              key={t.type}
              className={type === t.type ? 'active' : ''}
              disabled={!fits(t.type, kindA, kindB)}
              title={
                fits(t.type, kindA, kindB)
                  ? t.label
                  : `${t.label} doesn't suit a ${kindA} and a ${kindB}`
              }
              onClick={() => setType(t.type)}
            >
              {t.label}
            </button>
          ))}
        </div>
      </Section>
      <Section title="Options">
        {['coincident', 'parallel', 'tangent', 'concentric'].includes(type) && alignmentChoice}
        {(type === 'concentric' || type === 'profile_center') && (
          <Check label="Lock rotation" value={lockRotation} onChange={setLockRotation} />
        )}
        {(type === 'distance' || type === 'distance_limit') && (
          <>
            <LengthField label="Distance" value={distance} onChange={setDistance} />
            <Check label="Flip dimension" value={flip} onChange={setFlip} />
          </>
        )}
        {(type === 'angle' || type === 'angle_limit') && (
          <>
            <AngleField label="Angle" value={angle} onChange={setAngle} />
            <Check label="Flip dimension" value={flip} onChange={setFlip} />
          </>
        )}
        {(type === 'distance_limit' || type === 'hinge') && type === 'distance_limit' && (
          <>
            <LengthField label="Minimum" value={min} onChange={setMin} />
            <LengthField label="Maximum" value={max} onChange={setMax} />
          </>
        )}
        {(type === 'angle_limit' || type === 'hinge') && (
          <>
            <AngleField label="Minimum angle" value={min} onChange={setMin} />
            <AngleField label="Maximum angle" value={max} onChange={setMax} />
          </>
        )}
        {type === 'profile_center' && (
          <>
            <LengthField label="Offset" value={offset} onChange={setOffset} />
            <Check label="Flip" value={flip} onChange={setFlip} />
          </>
        )}
        {type === 'width' && (
          <>
            <Choice
              label="Constraint"
              value={
                travel === 'free' || travel === 'distance' || travel === 'percent'
                  ? travel
                  : 'center'
              }
              options={[
                { value: 'center', label: 'Centered' },
                { value: 'free', label: 'Free' },
                { value: 'distance', label: 'Dimension' },
                { value: 'percent', label: 'Percent' },
              ]}
              onChange={setTravel}
            />
            {travel === 'distance' && (
              <LengthField label="Distance" value={travelValue} onChange={setTravelValue} />
            )}
            {travel === 'percent' && (
              <NumberField
                label="Percent (0–1)"
                value={travelValue}
                min={0}
                onChange={(v) => setTravelValue(Math.min(1, v))}
              />
            )}
          </>
        )}
        {type === 'path' && travelChoice(false)}
        {type === 'slot' && travelChoice(true)}
        {(type === 'gear' || type === 'linear_coupler') && (
          <>
            <NumberField
              label={type === 'gear' ? 'Ratio A (teeth)' : 'Ratio A'}
              value={ratioA}
              min={0}
              onChange={setRatioA}
            />
            <NumberField
              label={type === 'gear' ? 'Ratio B (teeth)' : 'Ratio B'}
              value={ratioB}
              min={0}
              onChange={setRatioB}
            />
            <Check label="Reverse" value={reverse} onChange={setReverse} />
          </>
        )}
        {(type === 'rack_pinion' || type === 'screw') && (
          <>
            <LengthField label="Distance per revolution" value={perRev} onChange={setPerRev} />
            <Check label="Reverse" value={reverse} onChange={setReverse} />
          </>
        )}
      </Section>
    </PanelShell>
  )
}

/** Selection boxes for an existing mate's references (geometry unknown until re-picked). */
function fromDefinition(d: MateKind | undefined): Record<string, Picked[]> {
  if (!d) return {}
  const one = (v: unknown): Picked[] =>
    v && typeof v === 'object' && 'entity' in (v as object)
      ? [{ ref: v as Picked['ref'], geometry: null }]
      : []
  const many = (v: unknown): Picked[] => (Array.isArray(v) ? v.flatMap(one) : [])
  const out: Record<string, Picked[]> = {}
  for (const k of [
    'a',
    'b',
    'plane',
    'point',
    'follow',
    'cam',
    'follower',
    'slot',
    'pin',
    'rack',
    'pinion',
    'joint_point',
  ])
    if (d[k]) out[k === 'joint_point' ? 'joint' : k] = one(d[k])
  for (const k of ['width', 'tab', 'path', 'axis', 'faces']) if (d[k]) out[k] = many(d[k])
  const limit = d['limit'] as { faces?: unknown } | undefined
  if (limit?.faces) out['limit'] = many(limit.faces)
  if (d.type === 'lock') {
    out['a'] = [
      { ref: { component: d['a'] as string[], entity: { type: 'origin' } }, geometry: null },
    ]
    out['b'] = [
      { ref: { component: d['b'] as string[], entity: { type: 'origin' } }, geometry: null },
    ]
  }
  return out
}

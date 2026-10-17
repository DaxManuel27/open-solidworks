import { useEffect, useState } from 'react'
import type {
  ConnectedSegments,
  CornerTreatment,
  Family,
  MemberGroup,
  PiercePoint,
  ProfileMirror,
  ProfileSource,
} from '../../../../shared/tools.gen'
import { call } from '../../core'
import type { SketchView } from '../../protocol'
import { type Panel, useStore } from '../../store'
import { AngleField, Check, Choice, IntField, LengthField, PanelShell, Section } from '../fields'
import {
  OneRef,
  TextField,
  useActiveBox,
  useSheetFeature,
  type SheetFeature,
} from '../sheet-metal/common'

type Member = SheetFeature<'structural_member'>

interface FamilyInfo {
  family: Family
  label: string
  standard: 'iso' | 'ansi'
  sizes: string[]
}

interface SectionPreview {
  width: number
  height: number
  area: number
  ixx: number
  iyy: number
  outline: [number, number][]
}

/** The profile library: families and sizes, from the core. */
export function useFamilies(): FamilyInfo[] {
  const [families, setFamilies] = useState<FamilyInfo[]>([])
  useEffect(() => {
    void call<{ families: FamilyInfo[] }>('weldment.profiles', {}).then((r) =>
      setFamilies(r.families),
    )
  }, [])
  return families
}

/** A section's outline drawn to fit a small box, with its properties. */
function Preview({ source }: { source: ProfileSource }) {
  const [fetched, setP] = useState<{ key: string; preview: SectionPreview | null } | null>(null)
  const key = JSON.stringify(source)
  useEffect(() => {
    if (source.type !== 'standard') return
    let live = true
    void call<SectionPreview>('weldment.profiles', { family: source.family, size: source.size })
      .then((r) => live && setP({ key, preview: r }))
      .catch(() => live && setP({ key, preview: null }))
    return () => {
      live = false
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key])
  // Only the preview of the profile shown now.
  const p = source.type === 'standard' && fetched?.key === key ? fetched.preview : null
  if (!p) return null
  const s = 100 / Math.max(p.width, p.height)
  const d = p.outline
    .map(([x, y], i) => `${i ? 'L' : 'M'}${(60 + x * s).toFixed(2)} ${(60 - y * s).toFixed(2)}`)
    .join(' ')
  return (
    <div className="profile-preview">
      <svg viewBox="0 0 120 120" width="120" height="120" aria-label="Section preview">
        <path d={`${d} Z`} fill="none" stroke="currentColor" strokeWidth="1.2" />
        <circle cx="60" cy="60" r="1.6" fill="currentColor" />
      </svg>
      <small>
        {p.width.toFixed(1)} × {p.height.toFixed(1)} mm · A {(p.area / 100).toFixed(2)} cm² · Ix{' '}
        {(p.ixx / 1e4).toFixed(1)} cm⁴ · Iy {(p.iyy / 1e4).toFixed(1)} cm⁴
      </small>
    </div>
  )
}

/** Choose a library profile by standard, type and size, or a custom sketch. */
export function ProfilePicker(props: {
  value: ProfileSource
  onChange: (p: ProfileSource) => void
}) {
  const families = useFamilies()
  const p = props.value
  const standard =
    p.type === 'standard' ? families.find((f) => f.family === p.family)?.standard : null
  return (
    <Section title="Profile">
      <Choice
        label="Standard"
        value={p.type === 'custom' ? 'custom' : (standard ?? 'iso')}
        options={[
          { value: 'iso', label: 'ISO' },
          { value: 'ansi', label: 'ANSI inch' },
          { value: 'custom', label: 'Custom (sketch in a part)' },
        ]}
        onChange={(v) => {
          if (v === 'custom') return props.onChange({ type: 'custom', path: '', sketch: 1 })
          const f = families.find((x) => x.standard === v)
          if (f) props.onChange({ type: 'standard', family: f.family, size: f.sizes[0]! })
        }}
      />
      {p.type === 'standard' ? (
        <>
          <Choice
            label="Type"
            value={p.family}
            options={families
              .filter((f) => f.standard === (standard ?? 'iso'))
              .map((f) => ({ value: f.family, label: f.label }))}
            onChange={(family) => {
              const f = families.find((x) => x.family === family)!
              props.onChange({ type: 'standard', family, size: f.sizes[0]! })
            }}
          />
          <Choice
            label="Size"
            value={p.size}
            options={(families.find((f) => f.family === p.family)?.sizes ?? [p.size]).map((s) => ({
              value: s,
              label: s,
            }))}
            onChange={(size) => props.onChange({ ...p, size })}
          />
          <Preview source={p} />
        </>
      ) : (
        <>
          <TextField
            label="Profile part (.ofp)"
            value={p.path}
            placeholder="path/to/profile.ofp"
            onChange={(path) => props.onChange({ ...p, path })}
          />
          <IntField
            label="Sketch ID in that part"
            value={p.sketch}
            onChange={(sketch) => props.onChange({ ...p, sketch })}
          />
        </>
      )}
    </Section>
  )
}

/** Straight lines of a 2D or 3D sketch, by entity ID. */
function useSketchLines(sketch: number | null): { id: number; label: string }[] {
  const scene = useStore((s) => s.scene)
  const revision = useStore((s) => s.snapshot?.revision)
  const spatial = sketch ? scene?.sketches3d.find((s) => s.id === sketch) : undefined
  const [fetched, setFetched] = useState<{
    sketch: number
    lines: { id: number; label: string }[]
  } | null>(null)
  useEffect(() => {
    if (!sketch || spatial) return
    let live = true
    void call<SketchView>('sketch.get', { id: sketch }).then((v) => {
      if (!live) return
      setFetched({
        sketch,
        lines: v.curves
          .filter((c) => c.type === 'line' && !c.construction)
          .map((c) => {
            const [a, b] = [c.samples[0]!, c.samples[c.samples.length - 1]!]
            const len = Math.hypot(b[0] - a[0], b[1] - a[1])
            return { id: c.id, label: `Line ${c.id} (${len.toFixed(1)} mm)` }
          }),
      })
    })
    return () => {
      live = false
    }
  }, [sketch, spatial, revision])
  if (!sketch) return []
  if (spatial) {
    const entities = (spatial.sketch.entities ?? []) as {
      kind: string
      id: number
      construction?: boolean
    }[]
    return entities
      .filter((e) => e.kind === 'line' && !e.construction)
      .map((e) => ({ id: e.id, label: `Line ${e.id}` }))
  }
  return fetched?.sketch === sketch ? fetched.lines : []
}

/** 2D and 3D sketches in the tree. */
function useAllSketches() {
  const features = useStore((s) => s.snapshot?.features ?? [])
  return features.filter((f) => f.type === 'sketch' || f.type === 'sketch3d').reverse()
}

function GroupEditor(props: {
  index: number
  value: MemberGroup
  onChange: (g: MemberGroup | null) => void
}) {
  const g = props.value
  const sketches = useAllSketches()
  const first = g.segments[0]
  const [sketch, setSketch] = useState<number | null>(
    first?.type === 'sketch_entity' ? first.sketch : (sketches[0]?.id ?? null),
  )
  const lines = useSketchLines(sketch)
  const chosen = new Set(
    g.segments.flatMap((r) =>
      r.type === 'sketch_entity' && r.sketch === sketch ? [r.entity] : [],
    ),
  )
  const setChosen = (ids: number[]) =>
    props.onChange({
      ...g,
      segments: ids.map((entity) => ({ type: 'sketch_entity', sketch: sketch!, entity })),
    })
  return (
    <Section title={`Group ${props.index + 1}`}>
      <Choice
        label="Sketch"
        value={sketch ?? 0}
        options={[
          ...(sketch ? [] : [{ value: 0, label: 'Choose a sketch…' }]),
          ...sketches.map((s) => ({ value: s.id, label: s.name })),
        ]}
        onChange={(v) => {
          setSketch(v || null)
          props.onChange({ ...g, segments: [] })
        }}
      />
      <Check
        label="All lines"
        value={lines.length > 0 && lines.every((l) => chosen.has(l.id))}
        onChange={(all) => setChosen(all ? lines.map((l) => l.id) : [])}
      />
      {lines.map((l) => (
        <Check
          key={l.id}
          label={l.label}
          value={chosen.has(l.id)}
          onChange={(on) =>
            setChosen(on ? [...chosen, l.id] : [...chosen].filter((x) => x !== l.id))
          }
        />
      ))}
      <Choice<CornerTreatment>
        label="Corner treatment"
        value={g.corner ?? 'end_miter'}
        options={[
          { value: 'end_miter', label: 'End miter' },
          { value: 'end_butt1', label: 'End butt 1' },
          { value: 'end_butt2', label: 'End butt 2' },
        ]}
        onChange={(corner) => props.onChange({ ...g, corner })}
      />
      <LengthField
        label="Gap at corners"
        value={g.gap ?? 0}
        onChange={(gap) => props.onChange({ ...g, gap })}
      />
      <button className="link" onClick={() => props.onChange(null)}>
        Remove group
      </button>
    </Section>
  )
}

const PIERCE: { value: PiercePoint; label: string }[] = [
  { value: 'center', label: 'Center' },
  { value: 'centroid', label: 'Centroid' },
  { value: 'top_left', label: 'Top left' },
  { value: 'top', label: 'Top' },
  { value: 'top_right', label: 'Top right' },
  { value: 'left', label: 'Left' },
  { value: 'right', label: 'Right' },
  { value: 'bottom_left', label: 'Bottom left' },
  { value: 'bottom', label: 'Bottom' },
  { value: 'bottom_right', label: 'Bottom right' },
]

export function StructuralMemberPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'align'>('align')
  const { p, set, f } = useSheetFeature<Member>(
    panel,
    () => ({
      type: 'structural_member',
      profile: { type: 'standard', family: 'rhs', size: 'SHS 40x40x3' },
      groups: [{ segments: [], corner: 'end_miter', gap: 0 }],
      pierce: 'center',
      rotation: 0,
      mirror: 'none',
      offset: [0, 0],
      merge_miter_trimmed: false,
    }),
    (p) => p.groups.length > 0 && p.groups.every((g) => g.segments.length > 0),
  )
  const groups = p.groups
  return (
    <PanelShell title="Structural Member" {...f}>
      <ProfilePicker value={p.profile} onChange={(profile) => set({ profile })} />
      {groups.map((g, i) => (
        <GroupEditor
          key={i}
          index={i}
          value={g}
          onChange={(next) =>
            set({
              groups: next
                ? groups.map((x, k) => (k === i ? next : x))
                : groups.filter((_, k) => k !== i),
            })
          }
        />
      ))}
      <button
        className="link"
        onClick={() => set({ groups: [...groups, { segments: [], corner: 'end_miter', gap: 0 }] })}
      >
        Add group
      </button>
      <Section title="Locate Profile">
        <Choice
          label="Pierce point"
          value={p.pierce ?? 'center'}
          options={PIERCE}
          onChange={(pierce) => set({ pierce })}
        />
        <AngleField
          label="Rotation"
          value={p.rotation ?? 0}
          path="rotation"
          onChange={(rotation) => set({ rotation })}
        />
        <Choice<ProfileMirror>
          label="Mirror profile"
          value={p.mirror ?? 'none'}
          options={[
            { value: 'none', label: 'No' },
            { value: 'horizontal', label: 'Horizontal axis' },
            { value: 'vertical', label: 'Vertical axis' },
          ]}
          onChange={(mirror) => set({ mirror })}
        />
        <LengthField
          label="Offset across"
          value={p.offset?.[0] ?? 0}
          min={-1e6}
          onChange={(x) => set({ offset: [x, p.offset?.[1] ?? 0] })}
        />
        <LengthField
          label="Offset up"
          value={p.offset?.[1] ?? 0}
          min={-1e6}
          onChange={(y) => set({ offset: [p.offset?.[0] ?? 0, y] })}
        />
        <OneRef
          label="Align to (edge, axis or plane)"
          value={p.align}
          onChange={(align) => set({ align })}
          kinds={['edge', 'datum', 'std_plane']}
          active={box === 'align'}
          onActivate={() => setBox('align')}
        />
      </Section>
      <Section title="Settings">
        <Check
          label="Merge miter trimmed bodies"
          value={!!p.merge_miter_trimmed}
          onChange={(merge_miter_trimmed) => set({ merge_miter_trimmed })}
        />
        <Check
          label="Allow protrusion at butted corners"
          value={p.allow_protrusion ?? true}
          onChange={(allow_protrusion) => set({ allow_protrusion })}
        />
        <Choice<ConnectedSegments>
          label="Connected segments"
          value={p.connected ?? 'overlap'}
          options={[
            { value: 'overlap', label: 'Leave overlapping' },
            { value: 'simple', label: 'Simple cut' },
            { value: 'coped', label: 'Coped cut' },
          ]}
          onChange={(connected) => set({ connected })}
        />
      </Section>
      <p className="hint">
        Each line becomes a member body; lines of a group sharing an end get its corner treatment.
      </p>
    </PanelShell>
  )
}

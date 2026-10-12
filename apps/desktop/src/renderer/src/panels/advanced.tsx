import { useEffect, useState } from 'react'
import { call } from '../core'
import type { FeatureParams, Ref, SketchView, Thin } from '../protocol'
import type { ContourSelection, ProfileSelection } from '../../../shared/tools.gen'
import { type Panel, useStore } from '../store'
import {
  AngleField,
  Check,
  Choice,
  LengthField,
  NumberField,
  PanelShell,
  RefBox,
  Section,
  usePreview,
  useSketches,
} from './fields'
import { ThinEditor, useFeature } from './features'

const CURVE_TYPES = new Set(['sketch', 'sketch3d', 'helix', 'composite_curve', 'projected_curve'])

export function AdvancedDressupPanel({ panel }: { panel: Panel }) {
  const isDraft = panel.type === 'draft'
  const [active, setActive] = useState('geometry')
  const [params, setParams] = useState(() => ({
    type: panel.type,
    faces: [] as Ref[],
    edge: null as Ref | null,
    neutral_plane: { type: 'front_plane' } as Ref,
    direction: null as Ref | null,
    angle: Math.PI / 60,
    parting_line: [] as Ref[],
    other_side: false,
    step: false,
    reverse: false,
    straight: false,
    stations: [
      { position: 0, radius: 1 },
      { position: 1, radius: 2 },
    ],
    ...useStore.getState().snapshot?.features.find((f) => f.id === panel.editing)?.params,
  }))
  const [partingMode, setPartingMode] = useState(() => params.parting_line.length > 0)
  const parting = isDraft && partingMode
  const valid = isDraft
    ? parting
      ? params.parting_line.length > 0
      : params.faces.length > 0
    : !!params.edge
  const sent = isDraft
    ? parting
      ? { ...params, faces: [] }
      : { ...params, parting_line: [], other_side: false, step: false }
    : params
  const wiring = useFeature(panel, valid ? (sent as FeatureParams) : null)
  return (
    <PanelShell
      title={isDraft ? (parting ? 'Parting Line Draft' : 'Neutral Plane Draft') : 'Variable Fillet'}
      {...wiring}
    >
      {isDraft && (
        <Choice
          label="Type of draft"
          value={parting ? 'parting' : 'neutral'}
          options={[
            { value: 'neutral', label: 'Neutral plane' },
            { value: 'parting', label: 'Parting line' },
          ]}
          onChange={(kind) => {
            setPartingMode(kind === 'parting')
            if (kind === 'neutral') setParams({ ...params, parting_line: [] })
          }}
        />
      )}
      {isDraft && parting ? (
        <>
          <RefBox
            label="Parting line edges"
            refs={params.parting_line}
            kinds={['edge']}
            active={active === 'geometry'}
            onActivate={() => setActive('geometry')}
            onChange={(refs) => setParams({ ...params, parting_line: refs })}
          />
          <Check
            label="Other face (draft the side opposite the pull direction)"
            value={params.other_side}
            onChange={(other_side) => setParams({ ...params, other_side })}
          />
          <Check
            label="Step draft (level hinge, perpendicular steps on planar faces)"
            value={params.step}
            onChange={(step) => setParams({ ...params, step })}
          />
        </>
      ) : (
        <RefBox
          label={isDraft ? 'Faces to draft' : 'Edge to fillet'}
          refs={isDraft ? params.faces : params.edge ? [params.edge] : []}
          kinds={isDraft ? ['face'] : ['edge']}
          single={!isDraft}
          active={active === 'geometry'}
          onActivate={() => setActive('geometry')}
          onChange={(refs) => setParams({ ...params, faces: refs, edge: refs[0] ?? null })}
        />
      )}
      {isDraft ? (
        <>
          <RefBox
            label={parting ? 'Direction of pull (plane or face)' : 'Neutral plane'}
            refs={[params.neutral_plane]}
            kinds={['face', 'datum', 'std_plane']}
            single
            active={active === 'plane'}
            onActivate={() => setActive('plane')}
            onChange={(refs) => refs[0] && setParams({ ...params, neutral_plane: refs[0] })}
          />
          <RefBox
            label="Pull direction (default: neutral plane normal)"
            refs={params.direction ? [params.direction] : []}
            kinds={['edge', 'face', 'datum', 'std_plane']}
            single
            active={active === 'direction'}
            onActivate={() => setActive('direction')}
            onChange={(refs) => setParams({ ...params, direction: refs[0] ?? null })}
          />
          <AngleField
            label="Draft angle"
            value={params.angle}
            onChange={(angle) => setParams({ ...params, angle })}
          />
        </>
      ) : (
        <>
          <p className="hint">
            The fillet runs along every tangent-continuous edge joined to the selected one. Station
            positions are fractions of that chain, measured in the selected edge&apos;s direction.
          </p>
          <Check
            label="Reverse radius direction"
            value={params.reverse}
            onChange={(reverse) => setParams({ ...params, reverse })}
          />
          <Check
            label="Straight transition (linear between stations)"
            value={params.straight}
            onChange={(straight) => setParams({ ...params, straight })}
          />
          {params.stations.map((station, index) => (
            <Section key={index} title={`Radius station ${index + 1}`}>
              <NumberField
                label="Position (0–1 along contour)"
                value={station.position}
                min={0}
                onChange={(position) =>
                  setParams({
                    ...params,
                    stations: params.stations.map((s, i) => (i === index ? { ...s, position } : s)),
                  })
                }
              />
              <LengthField
                label="Radius"
                value={station.radius}
                onChange={(radius) =>
                  setParams({
                    ...params,
                    stations: params.stations.map((s, i) => (i === index ? { ...s, radius } : s)),
                  })
                }
              />
              {index > 0 && index < params.stations.length - 1 && (
                <button
                  onClick={() =>
                    setParams({
                      ...params,
                      stations: params.stations.filter((_, i) => i !== index),
                    })
                  }
                >
                  Remove
                </button>
              )}
            </Section>
          ))}
          <button
            onClick={() => {
              const stations = [...params.stations]
              const a = stations[stations.length - 2]!,
                b = stations[stations.length - 1]!
              stations.splice(stations.length - 1, 0, {
                position: (a.position + b.position) / 2,
                radius: (a.radius + b.radius) / 2,
              })
              setParams({ ...params, stations })
            }}
          >
            Add radius station
          </button>
        </>
      )}
    </PanelShell>
  )
}
function PathChoice({
  value,
  onChange,
  profileOnly = false,
}: {
  value: Ref | null
  onChange: (value: Ref | null) => void
  profileOnly?: boolean
}) {
  const features = useStore((s) => s.snapshot!.features).filter((f) =>
    profileOnly ? f.type === 'sketch' : CURVE_TYPES.has(f.type),
  )
  const scene = useStore((s) => s.scene)
  const curveViews = scene?.curves ?? []
  const revision = useStore((s) => s.snapshot!.revision)
  const selected =
    value?.type === 'feature'
      ? value.id
      : value?.type === 'sketch_contour' || value?.type === 'ordered_sketch_contour'
        ? value.sketch
        : value?.type === 'curve_segment'
          ? value.curve
          : 0
  const sketch = features.find((f) => f.id === selected && f.type === 'sketch')?.id
  const [loaded, setLoaded] = useState<{
    sketch: number
    revision: number
    contours: SketchView['contours']
    error?: string
  } | null>(null)
  useEffect(() => {
    if (!sketch) return
    let live = true
    void call<SketchView>('sketch.get', { id: sketch }).then(
      (v) =>
        live &&
        setLoaded({
          sketch,
          revision,
          contours: v.contours ?? [],
          error: v.profile_error ?? undefined,
        }),
      (e: Error) => live && setLoaded({ sketch, revision, contours: [], error: e.message }),
    )
    return () => {
      live = false
    }
  }, [sketch, revision])
  const current = loaded?.sketch === sketch && loaded?.revision === revision ? loaded : null
  const contourKey = (entities: number[]) => [...entities].sort((a, b) => a - b).join(',')
  const key =
    value?.type === 'sketch_contour' || value?.type === 'ordered_sketch_contour'
      ? contourKey(value.entities)
      : ''
  const contours = (current?.contours ?? []).filter((c) => !profileOnly || c.closed)
  const selectedContour = contours.find((c) => contourKey(c.reference.entities) === key)
  const segments = curveViews.find((c) => c.id === selected)?.segments ?? []
  return (
    <>
      <Choice
        label={profileOnly ? 'Profile sketch' : 'Sketch or curve'}
        value={selected}
        options={[
          { value: 0, label: profileOnly ? 'Choose a profile…' : 'Choose a path…' },
          ...features.map((f) => ({ value: f.id, label: f.name })),
        ]}
        onChange={(id) => onChange(id ? { type: 'feature', id } : null)}
      />
      {sketch && (
        <>
          <Choice
            label={profileOnly ? 'Closed profile contour' : 'Path contour'}
            value={key}
            options={[
              { value: '', label: 'Whole sketch (one connected path)' },
              ...contours.map((c, index) => ({
                value: contourKey(c.reference.entities),
                label: `${c.closed ? 'Closed loop' : 'Open chain'} ${index + 1} · ${c.reference.entities.length} curve(s)`,
              })),
              ...(key && !contours.some((c) => contourKey(c.reference.entities) === key)
                ? [
                    {
                      value: key,
                      label: current
                        ? 'Selected contour is missing — choose again'
                        : 'Loading selected contour…',
                    },
                  ]
                : []),
            ]}
            onChange={(choice) => {
              const contour = contours.find((c) => contourKey(c.reference.entities) === choice)
              if (!choice) onChange({ type: 'feature', id: sketch })
              else if (contour)
                onChange({
                  ...contour.reference,
                  type: 'ordered_sketch_contour',
                  reverse: false,
                  start: contour.ordered_entities[0],
                })
            }}
          />
          {(value?.type === 'ordered_sketch_contour' || value?.type === 'sketch_contour') && (
            <>
              {selectedContour?.closed && (
                <Choice
                  label="Start / seam entity"
                  value={value.type === 'ordered_sketch_contour' ? (value.start ?? 0) : 0}
                  options={[
                    { value: 0, label: 'Automatic start' },
                    ...(selectedContour?.ordered_entities ?? value.entities).map((id) => ({
                      value: id,
                      label: `Entity ${id}`,
                    })),
                  ]}
                  onChange={(start) =>
                    onChange({
                      ...value,
                      type: 'ordered_sketch_contour',
                      reverse:
                        value.type === 'ordered_sketch_contour' ? (value.reverse ?? false) : false,
                      start: start || null,
                    })
                  }
                />
              )}
              {selectedContour?.closed && value.type === 'ordered_sketch_contour' && (
                <>
                  <Check
                    label="Seam inside the start entity"
                    value={value.seam !== undefined && value.seam !== null}
                    onChange={(on) => onChange({ ...value, seam: on ? 0.5 : null })}
                  />
                  {value.seam !== undefined && value.seam !== null && (
                    <NumberField
                      label="Seam position along the start entity (0 to 1)"
                      value={value.seam}
                      min={0}
                      onChange={(seam) => onChange({ ...value, seam })}
                    />
                  )}
                </>
              )}
              <Check
                label="Reverse contour direction"
                value={value.type === 'ordered_sketch_contour' ? (value.reverse ?? false) : false}
                onChange={(reverse) =>
                  onChange({
                    ...value,
                    type: 'ordered_sketch_contour',
                    reverse,
                    start:
                      selectedContour?.closed && value.type === 'ordered_sketch_contour'
                        ? value.start
                        : null,
                  })
                }
              />
            </>
          )}
          {current?.error && <p role="alert">{current.error}</p>}
        </>
      )}
      {!sketch && !!selected && !profileOnly && (
        <Choice
          label="Generated curve segment"
          value={value?.type === 'curve_segment' ? value.segment : ''}
          options={[
            { value: '', label: 'Whole curve' },
            ...segments.map((segment, index) => ({
              value: segment.reference.segment,
              label: `Segment ${index + 1}`,
            })),
            ...(value?.type === 'curve_segment' &&
            !segments.some((s) => s.reference.segment === value.segment)
              ? [
                  {
                    value: value.segment,
                    label: 'Selected segment is missing or ambiguous — choose again',
                  },
                ]
              : []),
          ]}
          onChange={(segment) =>
            onChange(
              segment
                ? { type: 'curve_segment', curve: selected, segment }
                : { type: 'feature', id: selected },
            )
          }
        />
      )}
      {!profileOnly && (
        <RefBox
          label="Path reference"
          refs={value ? [value] : []}
          onChange={(refs) => onChange(refs[0] ?? null)}
          kinds={['edge', 'datum']}
          single
          active={true}
          onActivate={() => {}}
        />
      )}
    </>
  )
}

const profileSketch = (profile: ProfileSelection) =>
  typeof profile === 'number' ? profile : profile.sketch

function ProfileChoice({
  value,
  onChange,
}: {
  value: ProfileSelection
  onChange: (value: ProfileSelection) => void
}) {
  const reference: Ref =
    typeof value === 'number'
      ? { type: 'feature', id: value }
      : { ...value, type: 'ordered_sketch_contour' }
  return (
    <PathChoice
      profileOnly
      value={reference}
      onChange={(selected) => {
        if (!selected) onChange(0)
        else if (selected.type === 'feature') onChange(selected.id)
        else if (selected.type === 'ordered_sketch_contour') {
          const { sketch, entities, start, reverse, seam } = selected
          onChange({ sketch, entities, start, reverse, seam })
        }
      }}
    />
  )
}

function InnerProfileChoices({
  outer,
  value,
  onChange,
}: {
  outer: ProfileSelection
  value: ContourSelection[]
  onChange: (value: ContourSelection[]) => void
}) {
  const revision = useStore((s) => s.snapshot!.revision)
  const sketch = profileSketch(outer)
  const [loaded, setLoaded] = useState<{
    sketch: number
    revision: number
    contours: SketchView['contours']
  } | null>(null)
  useEffect(() => {
    if (!sketch) return
    let live = true
    void call<SketchView>('sketch.get', { id: sketch }).then(
      (view) => {
        if (live) setLoaded({ sketch, revision, contours: view.contours ?? [] })
      },
      () => {
        if (live) setLoaded({ sketch, revision, contours: [] })
      },
    )
    return () => {
      live = false
    }
  }, [sketch, revision])
  if (typeof outer === 'number') return null
  const key = (entities: number[]) => [...entities].sort((a, b) => a - b).join(',')
  const contours = (
    loaded?.sketch === sketch && loaded.revision === revision ? loaded.contours : []
  ).filter((c) => c.closed && key(c.reference.entities) !== key(outer.entities))
  return (
    <Section title="Profile holes">
      {contours.length === 0 && value.length === 0 && (
        <p>No additional closed contours in this sketch.</p>
      )}
      {contours.map((contour, index) => (
        <Check
          key={key(contour.reference.entities)}
          label={`Inner contour ${index + 1}`}
          value={value.some(
            (selected) =>
              selected.sketch === sketch &&
              key(selected.entities) === key(contour.reference.entities),
          )}
          onChange={(selected) =>
            onChange(
              selected
                ? [
                    ...value,
                    {
                      sketch,
                      entities: contour.reference.entities,
                      reverse: false,
                      start: contour.ordered_entities[0],
                    },
                  ]
                : value.filter(
                    (c) =>
                      c.sketch !== sketch || key(c.entities) !== key(contour.reference.entities),
                  ),
            )
          }
        />
      ))}
      {value
        .filter(
          (selected) =>
            selected.sketch !== sketch ||
            !contours.some((c) => key(c.reference.entities) === key(selected.entities)),
        )
        .map((missing) => (
          <Check
            key={`${missing.sketch}:${key(missing.entities)}`}
            label="Missing or incompatible hole — clear to repair"
            value={true}
            onChange={() => onChange(value.filter((c) => c !== missing))}
          />
        ))}
    </Section>
  )
}

type LoftEnd =
  | { type: 'none' }
  | { type: 'normal'; tangent_length?: number }
  | { type: 'direction'; direction: Ref; flip?: boolean; tangent_length?: number }

function LoftEndEditor(props: {
  label: string
  path: string
  value: LoftEnd
  active: boolean
  onActivate: () => void
  onChange: (value: LoftEnd) => void
}) {
  const { value, onChange } = props
  const length = value.type === 'none' ? 1 : (value.tangent_length ?? 1)
  return (
    <>
      <Choice
        label={`${props.label} constraint`}
        value={value.type}
        options={[
          { value: 'none', label: 'None' },
          { value: 'normal', label: 'Normal to profile' },
          { value: 'direction', label: 'Direction vector' },
        ]}
        onChange={(type) =>
          onChange(
            type === 'none'
              ? { type }
              : type === 'normal'
                ? { type, tangent_length: length }
                : {
                    type: 'direction',
                    direction: { type: 'front_plane' },
                    flip: false,
                    tangent_length: length,
                  },
          )
        }
      />
      {value.type === 'direction' && (
        <>
          <RefBox
            label={`${props.label} direction (edge, axis, plane or face)`}
            refs={[value.direction]}
            kinds={['edge', 'face', 'datum', 'std_plane']}
            single
            active={props.active}
            onActivate={props.onActivate}
            onChange={(refs) => refs[0] && onChange({ ...value, direction: refs[0] })}
          />
          <Check
            label="Reverse direction"
            value={!!value.flip}
            onChange={(flip) => onChange({ ...value, flip })}
          />
        </>
      )}
      {value.type !== 'none' && (
        <NumberField
          label="Tangent length"
          path={`${props.path}.tangent_length`}
          value={length}
          min={0}
          onChange={(tangent_length) => onChange({ ...value, tangent_length })}
        />
      )}
    </>
  )
}

export function SweepLoftPanel({ panel }: { panel: Panel }) {
  const sketches = useSketches()
  const isSweep = panel.type === 'sweep' || panel.type === 'sweep_cut'
  const [params, setParams] = useState(() => ({
    type: isSweep ? 'sweep' : 'loft',
    profile: (panel.sketch ?? sketches[0]?.id ?? 0) as ProfileSelection | null,
    circle_diameter: null as number | null,
    twist: 0,
    guide: null as Ref | null,
    guide_scale: false,
    guide2: null as Ref | null,
    thin: null as Thin | null,
    guides: [] as (Ref | null)[],
    closed: false,
    centerline: null as Ref | null,
    start: { type: 'none' } as LoftEnd,
    end: { type: 'none' } as LoftEnd,
    profiles: [] as ProfileSelection[],
    inner_profiles: [] as ContourSelection[],
    reversed: [] as number[],
    path: null as Ref | null,
    orientation: 'minimum_twist',
    reverse_path: false,
    ruled: false,
    cut: panel.type.endsWith('_cut'),
    merge: true,
    ...useStore.getState().snapshot?.features.find((f) => f.id === panel.editing)?.params,
  }))
  const [orient, setOrient] = useState<string>(() =>
    params.guide ? 'guide' : params.twist ? 'twist' : params.orientation,
  )
  const [active, setActive] = useState('')
  const ends = params.start.type !== 'none' || params.end.type !== 'none'
  const circular = params.circle_diameter !== null && params.circle_diameter !== undefined
  const valid = isSweep
    ? (circular
        ? params.circle_diameter! > 0
        : !!params.profile && profileSketch(params.profile) > 0) &&
      !!params.path &&
      (orient !== 'guide' || !!params.guide)
    : params.profiles.length >= 2 &&
      params.profiles.every((p) => profileSketch(p) > 0) &&
      params.guides.every((g) => !!g) &&
      params.guides.length !== 1 &&
      (!params.closed || (params.profiles.length >= 3 && params.guides.length === 0)) &&
      (!ends || (!params.closed && !params.ruled && !params.centerline && !params.guides.length)) &&
      (!params.centerline || (!params.closed && !params.guides.length)) &&
      [params.start, params.end].every((e) => e.type !== 'direction' || !!e.direction)
  const sent = isSweep && circular ? { ...params, profile: null, inner_profiles: [] } : params
  const wiring = useFeature(panel, valid ? (sent as FeatureParams) : null)
  return (
    <PanelShell title={`${isSweep ? 'Sweep' : 'Loft'}${params.cut ? ' Cut' : ''}`} {...wiring}>
      {isSweep ? (
        <>
          <Choice
            label="Section"
            value={circular ? 'circle' : 'sketch'}
            options={[
              { value: 'sketch', label: 'Sketch profile' },
              { value: 'circle', label: 'Circular profile' },
            ]}
            onChange={(kind) =>
              setParams({
                ...params,
                circle_diameter: kind === 'circle' ? (params.circle_diameter ?? 5) : null,
                profile:
                  kind === 'sketch'
                    ? (params.profile ?? panel.sketch ?? sketches[0]?.id ?? 0)
                    : params.profile,
              })
            }
          />
          {circular ? (
            <LengthField
              label="Circle diameter"
              path="circle_diameter"
              value={params.circle_diameter!}
              onChange={(circle_diameter) => setParams({ ...params, circle_diameter })}
            />
          ) : (
            <>
              <ProfileChoice
                value={params.profile ?? 0}
                onChange={(profile) => setParams({ ...params, profile })}
              />
              <InnerProfileChoices
                outer={params.profile ?? 0}
                value={params.inner_profiles}
                onChange={(inner_profiles) => setParams({ ...params, inner_profiles })}
              />
              <ThinEditor value={params.thin} onChange={(thin) => setParams({ ...params, thin })} />
            </>
          )}
          <PathChoice value={params.path} onChange={(path) => setParams({ ...params, path })} />
          <Check
            label="Reverse path"
            value={params.reverse_path}
            onChange={(reverse_path) => setParams({ ...params, reverse_path })}
          />
          <Choice
            label="Orientation"
            value={orient}
            options={[
              { value: 'minimum_twist', label: 'Minimum twist' },
              { value: 'frenet', label: 'Follow curvature' },
              { value: 'fixed', label: 'Keep profile parallel' },
              { value: 'twist', label: 'Twist along path' },
              { value: 'guide', label: 'Follow path and guide curve' },
            ]}
            onChange={(choice) => {
              setOrient(choice)
              setParams({
                ...params,
                orientation: choice === 'twist' || choice === 'guide' ? 'minimum_twist' : choice,
                twist: choice === 'twist' ? params.twist || Math.PI / 2 : 0,
                guide: choice === 'guide' ? params.guide : null,
                guide_scale: choice === 'guide' && params.guide_scale,
                guide2: choice === 'guide' ? params.guide2 : null,
              })
            }}
          />
          {orient === 'twist' && (
            <AngleField
              label="Twist angle"
              path="twist"
              value={params.twist}
              onChange={(twist) => setParams({ ...params, twist })}
            />
          )}
          {orient === 'guide' && (
            <>
              <PathChoice
                value={params.guide}
                onChange={(guide) => setParams({ ...params, guide })}
              />
              <Check
                label="Scale the profile to follow the guide"
                value={params.guide_scale}
                onChange={(guide_scale) => setParams({ ...params, guide_scale })}
              />
              <Section title="Second guide curve" open={!!params.guide2}>
                <PathChoice
                  value={params.guide2}
                  onChange={(guide2) => setParams({ ...params, guide2 })}
                />
                {params.guide2 && (
                  <button onClick={() => setParams({ ...params, guide2: null })}>
                    Remove second guide
                  </button>
                )}
              </Section>
              <p className="hint">
                The guide sets the profile&apos;s rotation where it crosses each plane normal to the
                path. With scaling, the profile also grows or shrinks to keep that point on it; a
                second guide stretches it so both guides stay on it. Scaling and second guides need
                an open, tangent-continuous path.
              </p>
            </>
          )}
        </>
      ) : (
        <>
          <Section title="Profiles in order">
            {params.profiles.map((profile, index) => (
              <div key={index}>
                <span>Section {index + 1}</span>
                <ProfileChoice
                  value={profile}
                  onChange={(next) =>
                    setParams({
                      ...params,
                      profiles: params.profiles.map((p, i) => (i === index ? next : p)),
                    })
                  }
                />
                <button
                  disabled={index === 0}
                  onClick={() => {
                    const profiles = [...params.profiles]
                    ;[profiles[index - 1], profiles[index]] = [
                      profiles[index]!,
                      profiles[index - 1]!,
                    ]
                    setParams({ ...params, profiles })
                  }}
                >
                  ↑
                </button>
                <button
                  onClick={() =>
                    setParams({
                      ...params,
                      profiles: params.profiles.filter((_, i) => i !== index),
                      reversed: params.reversed.filter((p) => p !== profileSketch(profile)),
                    })
                  }
                >
                  Remove
                </button>
                <Check
                  label="Reverse section"
                  value={params.reversed.includes(profileSketch(profile))}
                  onChange={(on) =>
                    setParams({
                      ...params,
                      reversed: on
                        ? [...params.reversed, profileSketch(profile)]
                        : params.reversed.filter((p) => p !== profileSketch(profile)),
                    })
                  }
                />
              </div>
            ))}
            <Choice
              label="Add section"
              value={0}
              options={[
                { value: 0, label: 'Choose a sketch…' },
                ...sketches
                  .filter((s) => !params.profiles.some((p) => profileSketch(p) === s.id))
                  .map((s) => ({ value: s.id, label: s.name })),
              ]}
              onChange={(id) => id && setParams({ ...params, profiles: [...params.profiles, id] })}
            />
          </Section>
          <Section title="Guide curves" open={params.guides.length > 0}>
            {params.guides.map((guide, index) => (
              <div key={index}>
                <span>Guide {index + 1}</span>
                <PathChoice
                  value={guide}
                  onChange={(next) =>
                    setParams({
                      ...params,
                      guides: params.guides.map((g, k) => (k === index ? next : g)),
                    })
                  }
                />
                <button
                  onClick={() =>
                    setParams({ ...params, guides: params.guides.filter((_, k) => k !== index) })
                  }
                >
                  Remove
                </button>
              </div>
            ))}
            <button onClick={() => setParams({ ...params, guides: [...params.guides, null] })}>
              Add guide
            </button>
            <p className="hint">
              With guides, the loft passes exactly through two or more curves that cross every
              section in order; ruled sides do not apply.
            </p>
          </Section>
          <Section title="Centerline" open={!!params.centerline}>
            <PathChoice
              value={params.centerline}
              onChange={(centerline) => setParams({ ...params, centerline })}
            />
            {params.centerline && (
              <button onClick={() => setParams({ ...params, centerline: null })}>
                Clear centerline
              </button>
            )}
            <p className="hint">
              The sections blend along a curve that crosses each section&apos;s plane in order.
            </p>
          </Section>
          <Section title="Start/end constraints" open={ends}>
            <LoftEndEditor
              label="Start"
              path="start"
              value={params.start}
              active={active === 'start'}
              onActivate={() => setActive('start')}
              onChange={(start) => setParams({ ...params, start })}
            />
            <LoftEndEditor
              label="End"
              path="end"
              value={params.end}
              active={active === 'end'}
              onActivate={() => setActive('end')}
              onChange={(end) => setParams({ ...params, end })}
            />
            <p className="hint">
              The surface leaves the end section along the chosen tangent; a longer tangent bulges
              further. Not available with guides, a centerline, ruled sides or closed lofts.
            </p>
          </Section>
          <Check
            label="Close loft (join the last section to the first)"
            value={params.closed}
            onChange={(closed) => setParams({ ...params, closed })}
          />
          <Check
            label="Ruled sides"
            value={params.ruled}
            onChange={(ruled) => setParams({ ...params, ruled })}
          />
        </>
      )}
      <p>Use one closed contour per section. Position the sweep section at the path start.</p>
      {!params.cut && (
        <Check
          label="Merge result"
          value={params.merge}
          onChange={(merge) => setParams({ ...params, merge })}
        />
      )}
    </PanelShell>
  )
}

export function BoundaryPanel({ panel }: { panel: Panel }) {
  const sketches = useSketches()
  const [params, setParams] = useState(() => ({
    type: 'boundary' as const,
    sections: [] as ProfileSelection[],
    curves: [] as (Ref | null)[],
    cut: panel.type === 'boundary_cut',
    merge: true,
    ...(useStore.getState().snapshot?.features.find((f) => f.id === panel.editing)?.params as
      object | undefined),
  }))
  const curves = params.curves.filter((c): c is Ref => !!c)
  const valid =
    params.sections.length >= 2 &&
    params.sections.every((p) => profileSketch(p) > 0) &&
    curves.length >= 2 &&
    curves.length === params.curves.length
  const wiring = useFeature(panel, valid ? ({ ...params, curves } as FeatureParams) : null)
  const move = <T,>(list: T[], index: number): T[] => {
    const next = [...list]
    ;[next[index - 1], next[index]] = [next[index]!, next[index - 1]!]
    return next
  }
  return (
    <PanelShell title={`Boundary${params.cut ? ' Cut' : ''}`} {...wiring}>
      <Section title="Direction 1 sections in order">
        {params.sections.map((section, index) => (
          <div key={index}>
            <span>Section {index + 1}</span>
            <ProfileChoice
              value={section}
              onChange={(next) =>
                setParams({
                  ...params,
                  sections: params.sections.map((p, i) => (i === index ? next : p)),
                })
              }
            />
            <button
              disabled={index === 0}
              onClick={() => setParams({ ...params, sections: move(params.sections, index) })}
            >
              ↑
            </button>
            <button
              onClick={() =>
                setParams({ ...params, sections: params.sections.filter((_, i) => i !== index) })
              }
            >
              Remove
            </button>
          </div>
        ))}
        <Choice
          label="Add section"
          value={0}
          options={[
            { value: 0, label: 'Choose a sketch…' },
            ...sketches
              .filter((s) => !params.sections.some((p) => profileSketch(p) === s.id))
              .map((s) => ({ value: s.id, label: s.name })),
          ]}
          onChange={(id) => id && setParams({ ...params, sections: [...params.sections, id] })}
        />
      </Section>
      <Section title="Direction 2 curves">
        {params.curves.map((curve, index) => (
          <div key={index}>
            <span>Curve {index + 1}</span>
            <PathChoice
              value={curve}
              onChange={(next) =>
                setParams({
                  ...params,
                  curves: params.curves.map((c, i) => (i === index ? next : c)),
                })
              }
            />
            <button
              onClick={() =>
                setParams({ ...params, curves: params.curves.filter((_, i) => i !== index) })
              }
            >
              Remove
            </button>
          </div>
        ))}
        <button onClick={() => setParams({ ...params, curves: [...params.curves, null] })}>
          Add curve
        </button>
      </Section>
      <p className="hint">
        Each direction 2 curve must cross every section, in section order, and all curves must go
        around the sections in the same order. The surface passes exactly through both curve
        families; patches meet with position continuity. The first and last sections must be planar
        to cap the solid.
      </p>
      {!params.cut && (
        <Check
          label="Merge result"
          value={params.merge}
          onChange={(merge) => setParams({ ...params, merge })}
        />
      )}
    </PanelShell>
  )
}

export function CurvePanel({ panel }: { panel: Panel }) {
  const [params, setParams] = useState(() => ({
    type: panel.type,
    plane: { type: 'front_plane' } as Ref,
    radius: 10,
    pitch: 5,
    turns: 3,
    start_angle: 0,
    left_handed: false,
    defined_by: 'pitch_and_turns' as 'pitch_and_turns' | 'height_and_turns' | 'height_and_pitch',
    height: 15,
    variable_pitch: [] as { turn: number; pitch: number }[],
    segments: [] as { reference: Ref; reverse: boolean }[],
    source: null as Ref | null,
    mode: 'onto_face' as 'onto_face' | 'sketch_on_sketch' | 'intersection' | 'silhouette',
    target: null as Ref | null,
    direction: [0, 0, -1],
    ...useStore.getState().snapshot?.features.find((f) => f.id === panel.editing)?.params,
  }))
  const [path, setPath] = useState<Ref | null>(null)
  const [active, setActive] = useState('source')
  const valid =
    panel.type === 'helix' ||
    (panel.type === 'composite_curve'
      ? params.segments.length > 0
      : !!params.target && (params.mode === 'silhouette' || !!params.source))
  const feature = valid ? (params as FeatureParams) : null
  const { error } = usePreview(feature, panel.editing)
  const close = useStore((s) => s.closePanel)
  const commit = useStore((s) => s.commit)
  return (
    <PanelShell
      title={
        panel.type === 'helix'
          ? 'Helix'
          : panel.type === 'composite_curve'
            ? 'Composite Curve'
            : panel.type === 'split_line'
              ? 'Split Line'
              : 'Projected Curve'
      }
      error={error}
      okDisabled={!valid}
      onCancel={() => void close()}
      onOk={() => feature && void commit(feature, panel.editing)}
    >
      {panel.type === 'helix' && (
        <>
          <RefBox
            label="Start plane"
            refs={[params.plane]}
            kinds={['face', 'datum', 'std_plane']}
            active={true}
            single
            onActivate={() => {}}
            onChange={(refs) => refs[0] && setParams({ ...params, plane: refs[0] })}
          />
          <LengthField
            label="Radius"
            value={params.radius}
            onChange={(radius) => setParams({ ...params, radius })}
          />
          <Check
            label="Variable pitch"
            value={params.variable_pitch.length > 0}
            onChange={(on) =>
              setParams({
                ...params,
                variable_pitch: on
                  ? [
                      { turn: 0, pitch: params.pitch },
                      { turn: params.turns, pitch: params.pitch },
                    ]
                  : [],
              })
            }
          />
          {params.variable_pitch.length > 0 ? (
            <Section title="Pitch rows (linear between rows)">
              {params.variable_pitch.map((row, index) => (
                <div key={index}>
                  <NumberField
                    label={`Row ${index + 1} at turn`}
                    value={row.turn}
                    min={0}
                    onChange={(turn) =>
                      setParams({
                        ...params,
                        variable_pitch: params.variable_pitch.map((r, i) =>
                          i === index ? { ...r, turn } : r,
                        ),
                      })
                    }
                  />
                  <LengthField
                    label="Pitch"
                    value={row.pitch}
                    onChange={(pitch) =>
                      setParams({
                        ...params,
                        variable_pitch: params.variable_pitch.map((r, i) =>
                          i === index ? { ...r, pitch } : r,
                        ),
                      })
                    }
                  />
                  {params.variable_pitch.length > 2 && (
                    <button
                      onClick={() =>
                        setParams({
                          ...params,
                          variable_pitch: params.variable_pitch.filter((_, i) => i !== index),
                        })
                      }
                    >
                      Remove
                    </button>
                  )}
                </div>
              ))}
              <button
                onClick={() => {
                  const last = params.variable_pitch[params.variable_pitch.length - 1]!
                  setParams({
                    ...params,
                    variable_pitch: [
                      ...params.variable_pitch,
                      { turn: last.turn + 1, pitch: last.pitch },
                    ],
                  })
                }}
              >
                Add row
              </button>
            </Section>
          ) : (
            <>
              <Choice
                label="Defined by"
                value={params.defined_by}
                options={[
                  { value: 'pitch_and_turns', label: 'Pitch and revolutions' },
                  { value: 'height_and_turns', label: 'Height and revolutions' },
                  { value: 'height_and_pitch', label: 'Height and pitch' },
                ]}
                onChange={(defined_by) => setParams({ ...params, defined_by })}
              />
              {params.defined_by !== 'pitch_and_turns' && (
                <LengthField
                  label="Height"
                  value={params.height}
                  onChange={(height) => setParams({ ...params, height })}
                />
              )}
              {params.defined_by !== 'height_and_turns' && (
                <LengthField
                  label="Pitch"
                  value={params.pitch}
                  onChange={(pitch) => setParams({ ...params, pitch })}
                />
              )}
              {params.defined_by !== 'height_and_pitch' && (
                <NumberField
                  label="Revolutions"
                  value={params.turns}
                  min={0.000001}
                  onChange={(turns) => setParams({ ...params, turns })}
                />
              )}
            </>
          )}
          <AngleField
            label="Start angle"
            value={params.start_angle}
            onChange={(start_angle) => setParams({ ...params, start_angle })}
          />
          <Check
            label="Left handed"
            value={params.left_handed}
            onChange={(left_handed) => setParams({ ...params, left_handed })}
          />
        </>
      )}
      {panel.type === 'composite_curve' && (
        <>
          <PathChoice value={path} onChange={setPath} />
          <button
            disabled={!path}
            onClick={() =>
              path &&
              setParams({
                ...params,
                segments: [...params.segments, { reference: path, reverse: false }],
              })
            }
          >
            Append to path
          </button>
          {params.segments.map((segment, index) => (
            <div key={index}>
              <Check
                label={`Segment ${index + 1}: reverse`}
                value={segment.reverse}
                onChange={(reverse) =>
                  setParams({
                    ...params,
                    segments: params.segments.map((s, i) => (i === index ? { ...s, reverse } : s)),
                  })
                }
              />
              <button
                onClick={() =>
                  setParams({ ...params, segments: params.segments.filter((_, i) => i !== index) })
                }
              >
                Remove
              </button>
            </div>
          ))}
        </>
      )}
      {(panel.type === 'projected_curve' || panel.type === 'split_line') && (
        <>
          <Choice
            label="Type"
            value={params.mode}
            options={
              panel.type === 'split_line'
                ? [
                    { value: 'onto_face', label: 'Projection' },
                    { value: 'intersection', label: 'Intersection' },
                    { value: 'silhouette', label: 'Silhouette' },
                  ]
                : [
                    { value: 'onto_face', label: 'Sketch on faces' },
                    { value: 'sketch_on_sketch', label: 'Sketch on sketch' },
                  ]
            }
            onChange={(mode) =>
              setParams({
                ...params,
                mode,
                source: mode === 'silhouette' ? { type: 'origin' } : null,
                target: null,
              })
            }
          />
          {params.mode === 'intersection' ? (
            <RefBox
              label="Splitting plane, face or body"
              refs={params.source ? [params.source] : []}
              kinds={['face', 'datum', 'std_plane']}
              active={active === 'source'}
              single
              onActivate={() => setActive('source')}
              onChange={(refs) => setParams({ ...params, source: refs[0] ?? null })}
            />
          ) : (
            params.mode !== 'silhouette' && (
              <Choice
                label={
                  params.mode === 'sketch_on_sketch' ? 'Second sketch' : 'Source sketch or curve'
                }
                value={params.source?.type === 'feature' ? params.source.id : 0}
                options={[
                  { value: 0, label: 'Choose source…' },
                  ...useStore
                    .getState()
                    .snapshot!.features.filter((f) =>
                      params.mode === 'sketch_on_sketch'
                        ? f.type === 'sketch'
                        : CURVE_TYPES.has(f.type),
                    )
                    .map((f) => ({ value: f.id, label: f.name })),
                ]}
                onChange={(id) =>
                  setParams({ ...params, source: id ? { type: 'feature', id } : null })
                }
              />
            )
          )}
          {params.mode === 'sketch_on_sketch' ? (
            <Choice
              label="First sketch"
              value={params.target?.type === 'feature' ? params.target.id : 0}
              options={[
                { value: 0, label: 'Choose a sketch…' },
                ...useStore
                  .getState()
                  .snapshot!.features.filter((f) => f.type === 'sketch')
                  .map((f) => ({ value: f.id, label: f.name })),
              ]}
              onChange={(id) =>
                setParams({ ...params, target: id ? { type: 'feature', id } : null })
              }
            />
          ) : (
            <RefBox
              label={panel.type === 'split_line' ? 'Face to split' : 'Target face'}
              refs={params.target ? [params.target] : []}
              kinds={['face']}
              active={active === 'target'}
              single
              onActivate={() => setActive('target')}
              onChange={(refs) => setParams({ ...params, target: refs[0] ?? null })}
            />
          )}
          {params.mode === 'silhouette' && (
            <p className="hint">
              Splits a cylindrical or spherical face where it turns away from the pull direction
              below.
            </p>
          )}
          {(params.mode === 'onto_face' || params.mode === 'silhouette') &&
            ['X', 'Y', 'Z'].map((axis, i) => (
              <NumberField
                key={axis}
                label={`Direction ${axis}`}
                value={params.direction[i]!}
                onChange={(value) =>
                  setParams({
                    ...params,
                    direction: params.direction.map((v, j) => (j === i ? value : v)),
                  })
                }
              />
            ))}
        </>
      )}
    </PanelShell>
  )
}

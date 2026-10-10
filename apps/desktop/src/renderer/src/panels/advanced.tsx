import { useState } from 'react'
import type { FeatureParams, Ref } from '../protocol'
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
import { useFeature } from './features'

const CURVE_TYPES = new Set(['sketch', 'helix', 'composite_curve', 'projected_curve'])

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
    reverse: false,
    stations: [
      { position: 0, radius: 1 },
      { position: 1, radius: 2 },
    ],
    ...useStore.getState().snapshot?.features.find((f) => f.id === panel.editing)?.params,
  }))
  const valid = isDraft ? params.faces.length > 0 : !!params.edge
  const wiring = useFeature(panel, valid ? (params as FeatureParams) : null)
  return (
    <PanelShell title={isDraft ? 'Neutral Plane Draft' : 'Variable Fillet'} {...wiring}>
      <RefBox
        label={isDraft ? 'Faces to draft' : 'Edge to fillet'}
        refs={isDraft ? params.faces : params.edge ? [params.edge] : []}
        kinds={isDraft ? ['face'] : ['edge']}
        single={!isDraft}
        active={active === 'geometry'}
        onActivate={() => setActive('geometry')}
        onChange={(refs) => setParams({ ...params, faces: refs, edge: refs[0] ?? null })}
      />
      {isDraft ? (
        <>
          <RefBox
            label="Neutral plane"
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
          <Check
            label="Reverse radius direction"
            value={params.reverse}
            onChange={(reverse) => setParams({ ...params, reverse })}
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
}: {
  value: Ref | null
  onChange: (value: Ref | null) => void
}) {
  const features = useStore((s) => s.snapshot!.features).filter((f) => CURVE_TYPES.has(f.type))
  return (
    <>
      <Choice
        label="Sketch or curve"
        value={value?.type === 'feature' ? value.id : 0}
        options={[
          { value: 0, label: 'Choose a path…' },
          ...features.map((f) => ({ value: f.id, label: f.name })),
        ]}
        onChange={(id) => onChange(id ? { type: 'feature', id } : null)}
      />
      <RefBox
        label="Path reference"
        refs={value ? [value] : []}
        onChange={(refs) => onChange(refs[0] ?? null)}
        kinds={['edge', 'datum']}
        single
        active={true}
        onActivate={() => {}}
      />
    </>
  )
}

export function SweepLoftPanel({ panel }: { panel: Panel }) {
  const sketches = useSketches()
  const isSweep = panel.type === 'sweep' || panel.type === 'sweep_cut'
  const [params, setParams] = useState(() => ({
    type: isSweep ? 'sweep' : 'loft',
    profile: panel.sketch ?? sketches[0]?.id ?? 0,
    profiles: [] as number[],
    reversed: [] as number[],
    path: null as Ref | null,
    orientation: 'minimum_twist',
    reverse_path: false,
    ruled: false,
    cut: panel.type.endsWith('_cut'),
    merge: true,
    ...useStore.getState().snapshot?.features.find((f) => f.id === panel.editing)?.params,
  }))
  const valid = isSweep ? !!params.profile && !!params.path : params.profiles.length >= 2
  const wiring = useFeature(panel, valid ? (params as FeatureParams) : null)
  return (
    <PanelShell title={`${isSweep ? 'Sweep' : 'Loft'}${params.cut ? ' Cut' : ''}`} {...wiring}>
      {isSweep ? (
        <>
          <Choice
            label="Closed profile"
            value={params.profile}
            options={sketches.map((s) => ({ value: s.id, label: s.name }))}
            onChange={(profile) => setParams({ ...params, profile })}
          />
          <PathChoice value={params.path} onChange={(path) => setParams({ ...params, path })} />
          <Check
            label="Reverse path"
            value={params.reverse_path}
            onChange={(reverse_path) => setParams({ ...params, reverse_path })}
          />
          <Choice
            label="Orientation"
            value={params.orientation}
            options={[
              { value: 'minimum_twist', label: 'Minimum twist' },
              { value: 'frenet', label: 'Follow curvature' },
              { value: 'fixed', label: 'Keep profile parallel' },
            ]}
            onChange={(orientation) => setParams({ ...params, orientation })}
          />
        </>
      ) : (
        <>
          <Section title="Profiles in order">
            {params.profiles.map((id, index) => (
              <div key={id}>
                <span>
                  {index + 1}. {sketches.find((s) => s.id === id)?.name ?? `Missing sketch ${id}`}
                </span>
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
                      profiles: params.profiles.filter((p) => p !== id),
                      reversed: params.reversed.filter((p) => p !== id),
                    })
                  }
                >
                  Remove
                </button>
                <Check
                  label="Reverse section"
                  value={params.reversed.includes(id)}
                  onChange={(on) =>
                    setParams({
                      ...params,
                      reversed: on
                        ? [...params.reversed, id]
                        : params.reversed.filter((p) => p !== id),
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
                  .filter((s) => !params.profiles.includes(s.id))
                  .map((s) => ({ value: s.id, label: s.name })),
              ]}
              onChange={(id) => id && setParams({ ...params, profiles: [...params.profiles, id] })}
            />
          </Section>
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

export function CurvePanel({ panel }: { panel: Panel }) {
  const [params, setParams] = useState(() => ({
    type: panel.type,
    plane: { type: 'front_plane' } as Ref,
    radius: 10,
    pitch: 5,
    turns: 3,
    start_angle: 0,
    left_handed: false,
    segments: [] as { reference: Ref; reverse: boolean }[],
    source: null as Ref | null,
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
      : !!params.source && !!params.target)
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
          <LengthField
            label="Pitch"
            value={params.pitch}
            onChange={(pitch) => setParams({ ...params, pitch })}
          />
          <NumberField
            label="Turns"
            value={params.turns}
            min={0.000001}
            onChange={(turns) => setParams({ ...params, turns })}
          />
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
            label="Source sketch or curve"
            value={params.source?.type === 'feature' ? params.source.id : 0}
            options={[
              { value: 0, label: 'Choose source…' },
              ...useStore
                .getState()
                .snapshot!.features.filter((f) => CURVE_TYPES.has(f.type))
                .map((f) => ({ value: f.id, label: f.name })),
            ]}
            onChange={(id) => setParams({ ...params, source: id ? { type: 'feature', id } : null })}
          />
          <RefBox
            label="Target face"
            refs={params.target ? [params.target] : []}
            kinds={['face']}
            active={active === 'target'}
            single
            onActivate={() => setActive('target')}
            onChange={(refs) => setParams({ ...params, target: refs[0] ?? null })}
          />
          {['X', 'Y', 'Z'].map((axis, i) => (
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

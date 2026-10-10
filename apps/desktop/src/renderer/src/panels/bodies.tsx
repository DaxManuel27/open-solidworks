import { useState } from 'react'
import type { FeatureParams, Ref, Vec3 } from '../protocol'
import { type Panel, useStore } from '../store'
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

export function BodyScope({
  value,
  onChange,
}: {
  value: string[] | null
  onChange: (value: string[] | null) => void
}) {
  const bodies = useStore((s) => s.scene?.bodies)
  if (!bodies?.length) return null
  return (
    <Section title="Feature scope">
      <Check
        label="All bodies"
        value={value === null}
        onChange={(all) => onChange(all ? null : bodies.map((b) => b.id))}
      />
      {value !== null &&
        bodies.map((body) => (
          <Check
            key={body.id}
            label={body.name}
            value={value.includes(body.id)}
            onChange={(checked) =>
              onChange(checked ? [...value, body.id] : value.filter((id) => id !== body.id))
            }
          />
        ))}
    </Section>
  )
}

export function BodiesPanel({ panel }: { panel: Panel }) {
  const scene = useStore((s) => s.scene)
  const commit = useStore((s) => s.commit)
  const close = useStore((s) => s.closePanel)
  const [params, setParams] = useState(() => {
    const initial = useStore
      .getState()
      .snapshot?.features.find((f) => f.id === panel.editing)?.params
    const selected = useStore
      .getState()
      .selection.flatMap(({ ref }) =>
        ref.type === 'body_topology' ? [ref.body] : ref.type === 'body' ? [ref.id] : [],
      )
    return {
      type: panel.type,
      bodies: [...new Set(selected)],
      operation: 'fuse',
      translation: [0, 0, 0] as Vec3,
      rotation: [0, 0, 0] as Vec3,
      origin: [0, 0, 0] as Vec3,
      copy: false,
      plane: null as Ref | null,
      keep: 'both',
      ...initial,
    }
  })
  const min = panel.type === 'combine' ? 2 : 1
  const valid = params.bodies.length >= min && (panel.type !== 'split_body' || params.plane)
  const feature = valid ? (params as FeatureParams) : null
  const { error } = usePreview(feature, panel.editing)
  const vector = (key: 'translation' | 'rotation' | 'origin', title: string) => (
    <Section title={title}>
      {(['X', 'Y', 'Z'] as const).map((label, axis) => {
        const Field = key === 'rotation' ? AngleField : LengthField
        return (
          <Field
            key={axis}
            label={label}
            value={params[key][axis]!}
            min={-Number.MAX_VALUE}
            onChange={(value) => {
              const next = [...params[key]] as Vec3
              next[axis] = value
              setParams({ ...params, [key]: next })
            }}
          />
        )
      })}
    </Section>
  )
  return (
    <PanelShell
      title={
        panel.type === 'combine'
          ? 'Combine Bodies'
          : panel.type === 'split_body'
            ? 'Split Bodies'
            : 'Move/Copy Bodies'
      }
      error={error}
      okDisabled={!valid}
      onCancel={() => void close()}
      onOk={() => feature && void commit(feature, panel.editing)}
    >
      <Section
        title={panel.type === 'combine' ? 'Bodies (first selected is the target)' : 'Bodies'}
      >
        {(scene?.bodies ?? []).map((body) => (
          <Check
            key={body.id}
            label={`${params.bodies.includes(body.id) ? `${params.bodies.indexOf(body.id) + 1}. ` : ''}${body.name}`}
            value={params.bodies.includes(body.id)}
            onChange={(checked) =>
              setParams({
                ...params,
                bodies: checked
                  ? [...params.bodies, body.id]
                  : params.bodies.filter((id) => id !== body.id),
              })
            }
          />
        ))}
        {params.bodies
          .filter((id) => !scene?.bodies.some((b) => b.id === id))
          .map((id) => (
            <Check
              key={id}
              label={`Missing: ${id}`}
              value={true}
              onChange={() =>
                setParams({ ...params, bodies: params.bodies.filter((b) => b !== id) })
              }
            />
          ))}
      </Section>
      {panel.type === 'combine' && (
        <Choice
          label="Operation"
          value={params.operation}
          options={[
            { value: 'fuse', label: 'Union' },
            { value: 'cut', label: 'Subtract' },
            { value: 'common', label: 'Intersect' },
          ]}
          onChange={(operation) => setParams({ ...params, operation })}
        />
      )}
      {panel.type === 'move_body' && (
        <>
          <Check
            label="Create copy"
            value={params.copy}
            onChange={(copy) => setParams({ ...params, copy })}
          />
          {vector('translation', 'Translation')}
          {vector('rotation', 'Rotation (X then Y then Z)')}
          {vector('origin', 'Rotation center')}
        </>
      )}
      {panel.type === 'split_body' && (
        <>
          <RefBox
            label="Splitting plane"
            refs={params.plane ? [params.plane] : []}
            kinds={['face', 'datum', 'std_plane']}
            active={true}
            single
            onActivate={() => {}}
            onChange={(refs) => setParams({ ...params, plane: refs[0] ?? null })}
          />
          <Choice
            label="Keep pieces"
            value={params.keep}
            options={[
              { value: 'both', label: 'Both sides' },
              { value: 'positive', label: 'Positive side' },
              { value: 'negative', label: 'Negative side' },
            ]}
            onChange={(keep) => setParams({ ...params, keep })}
          />
        </>
      )}
    </PanelShell>
  )
}

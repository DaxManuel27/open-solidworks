import { useEffect, useState } from 'react'
import { call } from '../core'
import type { FeatureParams, Ref } from '../protocol'
import { type Panel, useStore } from '../store'
import { AngleField, Check, Choice, LengthField, PanelShell, RefBox } from './fields'
import { useFeature } from './features'

type ThreadParams = {
  face: Ref | null
  diameter: number
  pitch: number
  length: number
  start_offset: number
  start_angle: number
  reverse: boolean
  left_handed: boolean
  internal: boolean
}

type Cylinder = { ref: Ref; surface: string; radius: number | null }

/** Cosmetic and modeled threads share parameters; only modeled ones cut material. */
export function CosmeticThreadPanel({ panel }: { panel: Panel }) {
  const modeled = panel.type === 'thread'
  const [params, setParams] = useState<ThreadParams>(() => ({
    face: useStore.getState().selection.find((s) => s.pick.kind === 'face')?.ref ?? null,
    diameter: 10,
    pitch: 1,
    length: 5,
    start_offset: 0,
    start_angle: 0,
    reverse: false,
    left_handed: false,
    internal: false,
    ...useStore.getState().snapshot?.features.find((f) => f.id === panel.editing)?.params,
  }))
  const [cylinders, setCylinders] = useState<Cylinder[]>([])
  const revision = useStore((s) => s.snapshot?.revision)
  useEffect(() => {
    let current = true
    void call<{ faces: Cylinder[] }>('doc.topology')
      .then((topology) => {
        if (current) setCylinders(topology.faces.filter((face) => face.surface === 'cylinder'))
      })
      .catch(() => {
        if (current) setCylinders([])
      })
    return () => {
      current = false
    }
  }, [revision])
  const select = (face: Ref | null) => {
    const cylinder = cylinders.find((c) => JSON.stringify(c.ref) === JSON.stringify(face))
    setParams((current) => ({
      ...current,
      face,
      diameter: cylinder?.radius ? 2 * cylinder.radius : current.diameter,
    }))
  }
  const selected = cylinders.findIndex((c) => JSON.stringify(c.ref) === JSON.stringify(params.face))
  const wiring = useFeature(
    panel,
    params.face
      ? ({ type: modeled ? 'thread' : 'cosmetic_thread', ...params } as FeatureParams)
      : null,
  )
  return (
    <PanelShell
      title={modeled ? 'Thread' : 'Cosmetic Thread'}
      {...wiring}
      okDisabled={wiring.okDisabled || !!wiring.error}
    >
      <p>
        {modeled
          ? 'A modeled thread cut into a complete cylindrical face with the ISO 60° basic profile. External threads use the face as the major diameter; internal threads use the bore as the minor diameter.'
          : 'A thread annotation on a complete cylindrical face. This adds no material and leaves mass unchanged.'}
      </p>
      <RefBox
        label="Cylindrical face"
        refs={params.face ? [params.face] : []}
        single
        kinds={['face']}
        active
        onActivate={() => {}}
        onChange={(refs) => select(refs[0] ?? null)}
      />
      <Choice
        label="Available cylinder"
        value={selected}
        options={[
          { value: -1, label: 'Pick a cylindrical face…' },
          ...cylinders.map((c, index) => ({
            value: index,
            label: `Cylinder ${index + 1} · Ø${2 * (c.radius ?? 0)} mm`,
          })),
        ]}
        onChange={(index) => select(cylinders[index]?.ref ?? null)}
      />
      <Check
        label="Internal hole thread"
        value={params.internal}
        onChange={(internal) => setParams((current) => ({ ...current, internal }))}
      />
      <LengthField
        label="Cylinder diameter"
        path="diameter"
        value={params.diameter}
        min={0.000001}
        onChange={(diameter) => setParams((current) => ({ ...current, diameter }))}
      />
      <LengthField
        label="Pitch"
        path="pitch"
        value={params.pitch}
        min={0.000001}
        onChange={(pitch) => setParams((current) => ({ ...current, pitch }))}
      />
      <LengthField
        label="Thread length"
        path="length"
        value={params.length}
        min={0.000001}
        onChange={(length) => setParams((current) => ({ ...current, length }))}
      />
      <LengthField
        label="Start offset"
        path="start_offset"
        value={params.start_offset}
        onChange={(start_offset) => setParams((current) => ({ ...current, start_offset }))}
      />
      <AngleField
        label="Start angle"
        path="start_angle"
        value={params.start_angle}
        onChange={(start_angle) => setParams((current) => ({ ...current, start_angle }))}
      />
      <Check
        label="Start at opposite end"
        value={params.reverse}
        onChange={(reverse) => setParams((current) => ({ ...current, reverse }))}
      />
      <Check
        label="Left-handed"
        value={params.left_handed}
        onChange={(left_handed) => setParams((current) => ({ ...current, left_handed }))}
      />
      <p>
        Both circular rims must remain complete. Length and offset must fit within the face. Up to{' '}
        {modeled ? '200 turns; modeled threads take longer to rebuild.' : '500 turns.'}
      </p>
    </PanelShell>
  )
}

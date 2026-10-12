import { useState } from 'react'
import type { FeatureParams, Ref } from '../protocol'
import { type Panel, useStore } from '../store'
import { LengthField, PanelShell, RefBox, Section } from './fields'
import { useFeature } from './features'

export function FaceFilletPanel({ panel }: { panel: Panel }) {
  const [init] = useState(() => {
    const editing = useStore
      .getState()
      .snapshot?.features.find((f) => f.id === panel.editing)?.params
    return {
      faces1: (editing?.['faces1'] as Ref[] | undefined) ?? [],
      faces2: (editing?.['faces2'] as Ref[] | undefined) ?? [],
      radius: (editing?.['radius'] as number | undefined) ?? 2,
    }
  })
  const [faces1, setFaces1] = useState<Ref[]>(init.faces1)
  const [faces2, setFaces2] = useState<Ref[]>(init.faces2)
  const [radius, setRadius] = useState(init.radius)
  const [active, setActive] = useState<1 | 2>(faces1.length ? 2 : 1)
  const params: FeatureParams | null =
    faces1.length && faces2.length && radius > 0
      ? { type: 'face_fillet', faces1, faces2, radius }
      : null
  const f = useFeature(panel, params)
  return (
    <PanelShell title="Face Fillet" {...f}>
      <Section title="Fillet parameters">
        <LengthField label="Radius" value={radius} path="radius" onChange={setRadius} />
        <RefBox
          label="Face set 1"
          refs={faces1}
          onChange={setFaces1}
          kinds={['face']}
          active={active === 1}
          onActivate={() => setActive(1)}
        />
        <RefBox
          label="Face set 2"
          refs={faces2}
          onChange={setFaces2}
          kinds={['face']}
          active={active === 2}
          onActivate={() => setActive(2)}
        />
      </Section>
      <p className="hint">
        Each set must lie in one plane. The blend is tangent to both planes and replaces any faces
        between the sets; it adds material at an inside corner and removes it at an outside corner.
        Curved faces are not supported yet.
      </p>
    </PanelShell>
  )
}

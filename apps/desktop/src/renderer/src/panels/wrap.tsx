import { useState } from 'react'
import type { FeatureParams, Ref } from '../protocol'
import { type Panel, useStore } from '../store'
import {
  Choice,
  LengthField,
  PanelShell,
  RefBox,
  Section,
  defaultSketch,
  useSketches,
} from './fields'
import { useFeature } from './features'

type WrapMode = 'emboss' | 'deboss' | 'scribe'

export function WrapPanel({ panel }: { panel: Panel }) {
  const [init] = useState(() => {
    const s = useStore.getState()
    const editing = s.snapshot?.features.find((f) => f.id === panel.editing)?.params
    const picked =
      s.selection.find(
        (x) => x.ref.type === 'face' || (x.ref.type === 'body_topology' && x.ref.kind === 'face'),
      )?.ref ?? null
    return {
      sketch:
        (editing?.['sketch'] as number | undefined) ??
        panel.sketch ??
        defaultSketch(s.snapshot?.features ?? [], s.treeSelection),
      face: (editing?.['face'] as Ref | undefined) ?? picked,
      mode: (editing?.['mode'] as WrapMode | undefined) ?? 'emboss',
      depth: (editing?.['depth'] as number | undefined) ?? 1,
    }
  })
  const sketches = useSketches()
  const [sketch, setSketch] = useState<number | null>(init.sketch)
  const [face, setFace] = useState<Ref | null>(init.face)
  const [mode, setMode] = useState<WrapMode>(init.mode)
  const [depth, setDepth] = useState(init.depth)
  const params: FeatureParams | null =
    sketch !== null && face && (mode === 'scribe' || depth > 0)
      ? { type: 'wrap', sketch, face, mode, depth: mode === 'scribe' ? 0 : depth }
      : null
  const f = useFeature(panel, params)
  return (
    <PanelShell title="Wrap" {...f}>
      <Section title="Wrap type">
        <Choice
          label="Type"
          value={mode}
          options={[
            { value: 'emboss', label: 'Emboss — raise material' },
            { value: 'deboss', label: 'Deboss — recess material' },
            { value: 'scribe', label: 'Scribe — split the face only' },
          ]}
          onChange={setMode}
        />
      </Section>
      <Section title="Parameters">
        <Choice
          label="Source sketch"
          value={sketch ?? 0}
          options={[
            ...(sketch === null ? [{ value: 0, label: 'Choose a closed sketch…' }] : []),
            ...sketches.map((s) => ({ value: s.id, label: s.name })),
          ]}
          onChange={(v) => v && setSketch(v)}
        />
        <RefBox
          label="Face to wrap onto (curved)"
          refs={face ? [face] : []}
          kinds={['face']}
          single
          active
          onActivate={() => {}}
          onChange={(refs) => setFace(refs[0] ?? null)}
        />
        {mode !== 'scribe' && (
          <LengthField label="Depth" value={depth} path="depth" onChange={setDepth} />
        )}
      </Section>
      <p className="hint">
        Cylinder: sketch on a plane parallel to its axis. Cone: sketch on a plane parallel to one
        that touches the cone along a line; the sketch unrolls about the apex. Other curved faces:
        sketch on a plane touching the face under the sketch origin; shapes follow the surface from
        there and may stretch lengths by at most 1%. Emboss and deboss depths run along the face
        normal.
      </p>
    </PanelShell>
  )
}

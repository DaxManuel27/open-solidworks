import { useState } from 'react'
import type { FeatureParams } from '../protocol'
import { type Panel, useStore } from '../store'
import {
  AngleField,
  Check,
  Choice,
  LengthField,
  PanelShell,
  Section,
  defaultSketch,
  useSketches,
} from './fields'
import { useFeature } from './features'

type RibSide = 'both' | 'first' | 'second'
type RibDirection = 'parallel_to_sketch' | 'normal_to_sketch'

export function RibPanel({ panel }: { panel: Panel }) {
  const [init] = useState(() => {
    const s = useStore.getState()
    const editing = s.snapshot?.features.find((f) => f.id === panel.editing)?.params
    return {
      sketch:
        (editing?.['sketch'] as number | undefined) ??
        panel.sketch ??
        defaultSketch(s.snapshot?.features ?? [], s.treeSelection),
      thickness: (editing?.['thickness'] as number | undefined) ?? 2,
      side: (editing?.['side'] as RibSide | undefined) ?? 'both',
      direction: (editing?.['direction'] as RibDirection | undefined) ?? 'parallel_to_sketch',
      flip: !!editing?.['flip'],
      draft: (editing?.['draft'] as number | null | undefined) ?? null,
    }
  })
  const sketches = useSketches()
  const [sketch, setSketch] = useState<number | null>(init.sketch)
  const [thickness, setThickness] = useState(init.thickness)
  const [side, setSide] = useState<RibSide>(init.side)
  const [direction, setDirection] = useState<RibDirection>(init.direction)
  const [flip, setFlip] = useState(init.flip)
  const [draft, setDraft] = useState<number | null>(init.draft)
  const params: FeatureParams | null =
    sketch !== null && thickness > 0
      ? { type: 'rib', sketch, thickness, side, direction, flip, draft }
      : null
  const f = useFeature(panel, params)
  return (
    <PanelShell title="Rib" {...f}>
      <Section title="Parameters">
        <Choice
          label="Sketch"
          value={sketch ?? 0}
          options={[
            ...(sketch === null ? [{ value: 0, label: 'Choose an open sketch…' }] : []),
            ...sketches.map((s) => ({ value: s.id, label: s.name })),
          ]}
          onChange={(v) => v && setSketch(v)}
        />
        <LengthField label="Thickness" value={thickness} path="thickness" onChange={setThickness} />
        <Choice
          label="Thickness side"
          value={side}
          options={[
            { value: 'both', label: 'Both sides (centered)' },
            { value: 'first', label: 'First side' },
            { value: 'second', label: 'Second side' },
          ]}
          onChange={setSide}
        />
        <Choice
          label="Extrusion direction"
          value={direction}
          options={[
            { value: 'parallel_to_sketch', label: 'Parallel to sketch' },
            { value: 'normal_to_sketch', label: 'Normal to sketch' },
          ]}
          onChange={setDirection}
        />
        <Check label="Flip material side" value={flip} onChange={setFlip} />
        <Check
          label="Draft"
          value={draft !== null}
          onChange={(on) => setDraft(on ? Math.PI / 90 : null)}
        />
        {draft !== null && (
          <AngleField label="Draft angle" value={draft} path="draft" onChange={setDraft} />
        )}
      </Section>
      <p className="hint">
        Draw open lines or arcs. The rib extends along the sketch&apos;s end tangents and grows
        toward the part until it meets material; flip it if it grows away. Draft on rib faces is not
        supported yet.
      </p>
    </PanelShell>
  )
}

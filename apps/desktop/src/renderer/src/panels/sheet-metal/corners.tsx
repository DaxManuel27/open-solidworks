import type { CornerBreak, CornerReliefKind } from '../../../../shared/tools.gen'
import type { Panel } from '../../store'
import { Check, Choice, LengthField, NumberField, PanelShell, Section } from '../fields'
import {
  EDGE,
  FACE,
  OneRef,
  POINT,
  Refs,
  TextField,
  UNPICKED,
  useActiveBox,
  useSheetFeature,
  type SheetFeature,
} from './common'

const RELIEFS: { value: CornerReliefKind; label: string }[] = [
  { value: 'rectangular', label: 'Rectangular' },
  { value: 'circular', label: 'Circular' },
  { value: 'tear', label: 'Tear' },
  { value: 'obround', label: 'Obround' },
  { value: 'constant_width', label: 'Constant width' },
  { value: 'full_round', label: 'Full round' },
  { value: 'suitcase', label: 'Suitcase' },
]

export function CornerReliefPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'bends'>('bends')
  const { p, set, f } = useSheetFeature<SheetFeature<'corner_relief'>>(
    panel,
    () => ({
      type: 'corner_relief',
      bends: [],
      kind: 'circular',
      ratio: 0.5,
      centered: true,
      slit: false,
    }),
    () => true,
  )
  return (
    <PanelShell title="Corner Relief" {...f}>
      <Section title="Scope">
        <Refs
          label="Bend faces (every corner when empty)"
          value={p.bends ?? []}
          onChange={(bends) => set({ bends })}
          kinds={FACE}
          active={box === 'bends'}
          onActivate={() => setBox('bends')}
        />
      </Section>
      <Section title="Relief Options">
        <Choice
          label="Relief type"
          value={p.kind ?? 'rectangular'}
          options={RELIEFS}
          onChange={(kind) => set({ kind })}
        />
        <NumberField
          label="Size past the bends (× thickness)"
          value={p.ratio ?? 0.5}
          min={0}
          path="ratio"
          onChange={(ratio) => set({ ratio })}
        />
        <Choice
          label="Placement"
          value={(p.centered ?? true) ? 'centered' : 'tangent'}
          options={[
            { value: 'centered', label: 'Centered on bend lines' },
            { value: 'tangent', label: 'Tangent to bend' },
          ]}
          onChange={(v) => set({ centered: v === 'centered' })}
        />
        <Check label="Add slit" value={!!p.slit} onChange={(slit) => set({ slit })} />
      </Section>
      <p className="hint">
        Corners of two or three bends get one relief each, cut through the sheet.
      </p>
    </PanelShell>
  )
}

export function ClosedCornerPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'faces'>('faces')
  const { p, set, f } = useSheetFeature<SheetFeature<'closed_corner'>>(
    panel,
    () => ({
      type: 'closed_corner',
      faces: [],
      kind: 'butt',
      gap: 0.1,
      overlap_ratio: 1,
      coplanar: false,
    }),
    (p) => p.faces.length > 0,
  )
  return (
    <PanelShell title="Closed Corner" {...f}>
      <Section title="Faces to Extend">
        <Refs
          label="End faces of flanges"
          value={p.faces}
          onChange={(faces) => set({ faces })}
          kinds={FACE}
          active={box === 'faces'}
          onActivate={() => setBox('faces')}
        />
        <Check
          label="Coplanar faces too"
          value={!!p.coplanar}
          onChange={(coplanar) => set({ coplanar })}
        />
      </Section>
      <Section title="Corner Type">
        <Choice
          label="Type"
          value={p.kind ?? 'butt'}
          options={[
            { value: 'butt', label: 'Butt' },
            { value: 'overlap', label: 'Overlap' },
            { value: 'underlap', label: 'Underlap' },
          ]}
          onChange={(kind) => set({ kind })}
        />
        <LengthField label="Gap" value={p.gap ?? 0.1} path="gap" onChange={(gap) => set({ gap })} />
        {p.kind !== 'butt' && (
          <NumberField
            label="Overlap ratio"
            value={p.overlap_ratio ?? 1}
            min={0}
            path="overlap_ratio"
            onChange={(overlap_ratio) => set({ overlap_ratio: Math.min(1, overlap_ratio) })}
          />
        )}
      </Section>
    </PanelShell>
  )
}

export function WeldedCornerPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'edge' | 'stop'>('edge')
  const { p, set, f } = useSheetFeature<SheetFeature<'welded_corner'>>(
    panel,
    () => ({
      type: 'welded_corner',
      edge: UNPICKED,
      size: 2,
      symbol: null,
      stop: null,
      fillet: null,
    }),
    (p) => !!p.edge,
  )
  return (
    <PanelShell title="Welded Corner" {...f}>
      <Section title="Corner">
        <OneRef
          label="Seam edge"
          value={p.edge}
          onChange={(edge) => edge && set({ edge })}
          kinds={EDGE}
          active={box === 'edge'}
          onActivate={() => setBox('edge')}
        />
        <OneRef
          label="Stop point (optional)"
          value={p.stop}
          onChange={(stop) => set({ stop })}
          kinds={POINT}
          active={box === 'stop'}
          onActivate={() => setBox('stop')}
        />
      </Section>
      <Section title="Bead">
        <LengthField
          label="Size"
          value={p.size ?? 2}
          path="size"
          onChange={(size) => set({ size })}
        />
        <Check
          label="Fillet"
          value={p.fillet !== null && p.fillet !== undefined}
          onChange={(on) => set({ fillet: on ? 1 : null })}
        />
        {p.fillet !== null && p.fillet !== undefined && (
          <LengthField
            label="Fillet radius"
            value={p.fillet}
            path="fillet"
            onChange={(fillet) => set({ fillet })}
          />
        )}
        <TextField
          label="Weld symbol"
          value={p.symbol ?? ''}
          placeholder="e.g. 3 ⊿"
          onChange={(symbol) => set({ symbol: symbol || null })}
        />
      </Section>
      <p className="hint">The bead is shown and listed; the solid is not changed.</p>
    </PanelShell>
  )
}

export function CornerBreakEditor(props: {
  value: CornerBreak
  onChange: (c: CornerBreak) => void
}) {
  const c = props.value
  return (
    <>
      <Choice
        label="Break type"
        value={c.type}
        options={[
          { value: 'chamfer', label: 'Chamfer' },
          { value: 'fillet', label: 'Fillet' },
        ]}
        onChange={(type) =>
          props.onChange(
            type === 'chamfer'
              ? { type, distance: 'radius' in c ? c.radius : c.distance }
              : { type, radius: 'distance' in c ? c.distance : c.radius },
          )
        }
      />
      {c.type === 'chamfer' ? (
        <LengthField
          label="Distance"
          value={c.distance}
          path="corner.distance"
          onChange={(distance) => props.onChange({ ...c, distance })}
        />
      ) : (
        <LengthField
          label="Radius"
          value={c.radius}
          path="corner.radius"
          onChange={(radius) => props.onChange({ ...c, radius })}
        />
      )}
    </>
  )
}

export function BreakCornerPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'edges'>('edges')
  const { p, set, f } = useSheetFeature<SheetFeature<'break_corner'>>(
    panel,
    () => ({
      type: 'break_corner',
      edges: [],
      corner: { type: 'fillet', radius: 3 },
      internal_only: false,
    }),
    () => true,
  )
  return (
    <PanelShell title="Break Corner/Corner Trim" {...f}>
      <Section title="Corner Parameters">
        <Refs
          label="Corner edges (every outside corner when empty)"
          value={p.edges ?? []}
          onChange={(edges) => set({ edges })}
          kinds={EDGE}
          active={box === 'edges'}
          onActivate={() => setBox('edges')}
        />
        <CornerBreakEditor value={p.corner} onChange={(corner) => set({ corner })} />
        <Check
          label="Internal corners only"
          value={!!p.internal_only}
          onChange={(internal_only) => set({ internal_only })}
        />
      </Section>
    </PanelShell>
  )
}

export function BendNotchPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'bends'>('bends')
  const { p, set, f } = useSheetFeature<SheetFeature<'bend_notch'>>(
    panel,
    () => ({
      type: 'bend_notch',
      bends: [],
      shape: 'v',
      width: 1,
      depth: 1.5,
      both_ends: true,
    }),
    () => true,
  )
  return (
    <PanelShell title="Bend Notch" {...f}>
      <Section title="Bends">
        <Refs
          label="Bend faces (every bend when empty)"
          value={p.bends ?? []}
          onChange={(bends) => set({ bends })}
          kinds={FACE}
          active={box === 'bends'}
          onActivate={() => setBox('bends')}
        />
      </Section>
      <Section title="Notch">
        <Choice
          label="Shape"
          value={p.shape ?? 'v'}
          options={[
            { value: 'v', label: 'V' },
            { value: 'rectangular', label: 'Rectangular' },
          ]}
          onChange={(shape) => set({ shape })}
        />
        <LengthField
          label="Width"
          value={p.width ?? 1}
          path="width"
          onChange={(width) => set({ width })}
        />
        <LengthField
          label="Depth"
          value={p.depth ?? 1.5}
          path="depth"
          onChange={(depth) => set({ depth })}
        />
        <Check
          label="Both ends"
          value={p.both_ends ?? true}
          onChange={(both_ends) => set({ both_ends })}
        />
      </Section>
    </PanelShell>
  )
}

import type { Ref as GenRef, RipEdge } from '../../../../shared/tools.gen'
import type { Panel } from '../../store'
import { AngleField, Check, Choice, LengthField, PanelShell, Section } from '../fields'
import {
  BendOverrideEditor,
  EDGE,
  FACE,
  OneRef,
  Refs,
  SheetParamsEditor,
  SketchPick,
  UNPICKED,
  defaultSheet,
  partSheet,
  startSketch,
  useActiveBox,
  useSheetFeature,
  type SheetFeature,
} from './common'

const BEND_POSITIONS = [
  { value: 'centerline', label: 'Bend centerline' },
  { value: 'material_inside', label: 'Material inside' },
  { value: 'material_outside', label: 'Material outside' },
  { value: 'bend_outside', label: 'Bend outside' },
] as const

export function SketchedBendPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'fixed'>('fixed')
  const { p, set, f } = useSheetFeature<SheetFeature<'sketched_bend'>>(
    panel,
    () => ({
      type: 'sketched_bend',
      sketch: startSketch(panel),
      fixed: null,
      position: 'centerline',
      angle: Math.PI / 2,
      reverse: false,
      bend: {},
    }),
    (p) => p.sketch > 0,
  )
  return (
    <PanelShell title="Sketched Bend" {...f}>
      <Section title="Bend Parameters">
        <SketchPick
          label="Bend line sketch"
          value={p.sketch}
          onChange={(sketch) => set({ sketch })}
        />
        <OneRef
          label="Fixed face (side that stays)"
          value={p.fixed}
          onChange={(fixed) => set({ fixed })}
          kinds={FACE}
          active={box === 'fixed'}
          onActivate={() => setBox('fixed')}
        />
        <Choice
          label="Bend position"
          value={p.position ?? 'centerline'}
          options={[...BEND_POSITIONS]}
          onChange={(position) => set({ position })}
        />
        <AngleField
          label="Angle"
          value={p.angle ?? Math.PI / 2}
          path="angle"
          onChange={(angle) => set({ angle })}
        />
        <Check
          label="Reverse direction"
          value={!!p.reverse}
          onChange={(reverse) => set({ reverse })}
        />
      </Section>
      <BendOverrideEditor value={p.bend} onChange={(bend) => set({ bend })} relief={false} />
    </PanelShell>
  )
}

export function JogPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'fixed'>('fixed')
  const { p, set, f } = useSheetFeature<SheetFeature<'jog'>>(
    panel,
    () => ({
      type: 'jog',
      sketch: startSketch(panel),
      offset: 5,
      fixed: null,
      position: 'centerline',
      dimension: 'outside',
      angle: Math.PI / 2,
      fix_projected_length: true,
      reverse: false,
      bend: {},
    }),
    (p) => p.sketch > 0 && p.offset > 0,
  )
  return (
    <PanelShell title="Jog" {...f}>
      <Section title="Selections">
        <SketchPick
          label="Jog line sketch"
          value={p.sketch}
          onChange={(sketch) => set({ sketch })}
        />
        <OneRef
          label="Fixed face"
          value={p.fixed}
          onChange={(fixed) => set({ fixed })}
          kinds={FACE}
          active={box === 'fixed'}
          onActivate={() => setBox('fixed')}
        />
      </Section>
      <Section title="Jog Offset">
        <LengthField
          label="Offset"
          value={p.offset}
          path="offset"
          onChange={(offset) => set({ offset })}
        />
        <Choice
          label="Dimension position"
          value={p.dimension ?? 'outside'}
          options={[
            { value: 'outside', label: 'Outside offset' },
            { value: 'inside', label: 'Inside offset' },
            { value: 'overall', label: 'Overall dimension' },
          ]}
          onChange={(dimension) => set({ dimension })}
        />
        <Check
          label="Fix projected length"
          value={p.fix_projected_length ?? true}
          onChange={(fix_projected_length) => set({ fix_projected_length })}
        />
        <Check
          label="Reverse direction"
          value={!!p.reverse}
          onChange={(reverse) => set({ reverse })}
        />
      </Section>
      <Section title="Jog Position">
        <Choice
          label="Position"
          value={p.position ?? 'centerline'}
          options={[...BEND_POSITIONS]}
          onChange={(position) => set({ position })}
        />
        <AngleField
          label="Jog angle"
          value={p.angle ?? Math.PI / 2}
          path="angle"
          onChange={(angle) => set({ angle })}
        />
      </Section>
      <BendOverrideEditor value={p.bend} onChange={(bend) => set({ bend })} relief={false} />
    </PanelShell>
  )
}

export function CrossBreakPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'face'>('face')
  const { p, set, f } = useSheetFeature<SheetFeature<'cross_break'>>(
    panel,
    () => ({
      type: 'cross_break',
      face: UNPICKED,
      angle: (3 * Math.PI) / 180,
      radius: 1,
      reverse: false,
    }),
    (p) => !!p.face,
  )
  return (
    <PanelShell title="Cross-Break" {...f}>
      <Section title="Cross-Break Parameters">
        <OneRef
          label="Face"
          value={p.face}
          onChange={(face) => face && set({ face })}
          kinds={FACE}
          active={box === 'face'}
          onActivate={() => setBox('face')}
        />
        <AngleField
          label="Break angle"
          value={p.angle ?? 0.05}
          path="angle"
          onChange={(angle) => set({ angle })}
        />
        <LengthField
          label="Radius"
          value={p.radius ?? 1}
          path="radius"
          onChange={(radius) => set({ radius })}
        />
        <Check
          label="Reverse direction"
          value={!!p.reverse}
          onChange={(reverse) => set({ reverse })}
        />
      </Section>
      <p className="hint">
        Cross breaks are drawn on the face and in the flat; the solid stays flat.
      </p>
    </PanelShell>
  )
}

/** Rip edges with a direction each. */
function RipEdges(props: {
  value: RipEdge[]
  onChange: (r: RipEdge[]) => void
  active: boolean
  onActivate: () => void
}) {
  return (
    <>
      <Refs
        label="Rip edges"
        value={props.value.map((r) => r.edge)}
        onChange={(edges: GenRef[]) =>
          props.onChange(
            edges.map(
              (edge) =>
                props.value.find((r) => JSON.stringify(r.edge) === JSON.stringify(edge)) ?? {
                  edge,
                  direction: 'both',
                },
            ),
          )
        }
        kinds={EDGE}
        active={props.active}
        onActivate={props.onActivate}
      />
      {props.value.map((r, i) => (
        <Choice
          key={i}
          label={`Rip ${i + 1} direction`}
          value={r.direction ?? 'both'}
          options={[
            { value: 'both', label: 'Both directions' },
            { value: 'first', label: 'First direction' },
            { value: 'second', label: 'Second direction' },
          ]}
          onChange={(direction) =>
            props.onChange(props.value.map((x, k) => (k === i ? { ...x, direction } : x)))
          }
        />
      ))}
    </>
  )
}

export function InsertBendsPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'fixed' | 'rips'>('fixed')
  const { p, set, f } = useSheetFeature<SheetFeature<'insert_bends'>>(
    panel,
    () => ({
      type: 'insert_bends',
      fixed_face: UNPICKED,
      sheet: partSheet() ?? defaultSheet(),
      rips: [],
      rip_gap: 0.1,
    }),
    (p) => !!p.fixed_face,
  )
  return (
    <PanelShell title="Insert Bends" {...f}>
      <Section title="Bend Parameters">
        <OneRef
          label="Fixed face"
          value={p.fixed_face}
          onChange={(fixed_face) => fixed_face && set({ fixed_face })}
          kinds={FACE}
          active={box === 'fixed'}
          onActivate={() => setBox('fixed')}
        />
      </Section>
      <Section title="Rip Parameters">
        <RipEdges
          value={p.rips ?? []}
          onChange={(rips) => set({ rips })}
          active={box === 'rips'}
          onActivate={() => setBox('rips')}
        />
        <LengthField
          label="Rip gap"
          value={p.rip_gap ?? 0.1}
          path="rip_gap"
          onChange={(rip_gap) => set({ rip_gap })}
        />
      </Section>
      <SheetParamsEditor value={p.sheet} onChange={(sheet) => set({ sheet })} path="sheet" />
      <p className="hint">
        For a thin-walled part such as a shell: the sheet&apos;s two sides are found, sharp edges
        get bends and the rip edges are cut open.
      </p>
    </PanelShell>
  )
}

export function ConvertPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'fixed' | 'bends'>('fixed')
  const { p, set, f } = useSheetFeature<SheetFeature<'convert_to_sheet_metal'>>(
    panel,
    () => ({
      type: 'convert_to_sheet_metal',
      fixed_face: UNPICKED,
      sheet: partSheet() ?? defaultSheet(),
      bend_edges: [],
      rip_gap: 0.1,
      keep_body: false,
    }),
    (p) => !!p.fixed_face && p.bend_edges.length > 0,
  )
  return (
    <PanelShell title="Convert To Sheet Metal" {...f}>
      <Section title="Sheet Metal Parameters">
        <OneRef
          label="Fixed face"
          value={p.fixed_face}
          onChange={(fixed_face) => fixed_face && set({ fixed_face })}
          kinds={FACE}
          active={box === 'fixed'}
          onActivate={() => setBox('fixed')}
        />
        <Check
          label="Keep the solid body"
          value={!!p.keep_body}
          onChange={(keep_body) => set({ keep_body })}
        />
      </Section>
      <Section title="Bend Edges">
        <Refs
          label="Edges to bend (the others are ripped)"
          value={p.bend_edges}
          onChange={(bend_edges) => set({ bend_edges })}
          kinds={EDGE}
          active={box === 'bends'}
          onActivate={() => setBox('bends')}
        />
        <LengthField
          label="Rip gap"
          value={p.rip_gap ?? 0.1}
          path="rip_gap"
          onChange={(rip_gap) => set({ rip_gap })}
        />
      </Section>
      <SheetParamsEditor value={p.sheet} onChange={(sheet) => set({ sheet })} path="sheet" />
    </PanelShell>
  )
}

export function RipPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'rips'>('rips')
  const { p, set, f } = useSheetFeature<SheetFeature<'rip'>>(
    panel,
    () => ({ type: 'rip', edges: [], gap: 0.1 }),
    (p) => p.edges.length > 0,
  )
  return (
    <PanelShell title="Rip" {...f}>
      <Section title="Rip Parameters">
        <RipEdges
          value={p.edges}
          onChange={(edges) => set({ edges })}
          active={box === 'rips'}
          onActivate={() => setBox('rips')}
        />
        <LengthField
          label="Rip gap"
          value={p.gap ?? 0.1}
          path="gap"
          onChange={(gap) => set({ gap })}
        />
      </Section>
    </PanelShell>
  )
}

export function UnfoldPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'fixed'>('fixed')
  const { p, set, f } = useSheetFeature<SheetFeature<'unfold'>>(
    panel,
    () => ({ type: 'unfold', fixed_face: null }),
    () => true,
  )
  return (
    <PanelShell title="Unfold" {...f}>
      <Section title="Selections">
        <OneRef
          label="Fixed face (the body's own when empty)"
          value={p.fixed_face}
          onChange={(fixed_face) => set({ fixed_face })}
          kinds={FACE}
          active={box === 'fixed'}
          onActivate={() => setBox('fixed')}
        />
      </Section>
      <p className="hint">
        Flattens the sheet so cuts can cross its bends; add a Fold after them to form it again.
      </p>
    </PanelShell>
  )
}

export function FoldPanel({ panel }: { panel: Panel }) {
  const { f } = useSheetFeature<SheetFeature<'fold'>>(
    panel,
    () => ({ type: 'fold' }),
    () => true,
  )
  return (
    <PanelShell title="Fold" {...f}>
      <p className="hint">Forms the sheet unfolded by the last Unfold again, cuts and all.</p>
    </PanelShell>
  )
}

export function NormalCutPanel({ panel }: { panel: Panel }) {
  const { p, set, f } = useSheetFeature<SheetFeature<'normal_cut'>>(
    panel,
    () => ({ type: 'normal_cut', sketch: startSketch(panel) }),
    (p) => p.sketch > 0,
  )
  return (
    <PanelShell title="Cut-Normal" {...f}>
      <Section title="Profile">
        <SketchPick label="Sketch" value={p.sketch} onChange={(sketch) => set({ sketch })} />
      </Section>
      <p className="hint">The sketch is projected onto the sheet and cut square through it.</p>
    </PanelShell>
  )
}

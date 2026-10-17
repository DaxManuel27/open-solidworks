import type {
  BeadKind,
  BeadSides,
  CapDirection,
  CapOffset,
  GussetLocation,
  GussetShape,
  GussetSide,
  TrimKind,
} from '../../../../shared/tools.gen'
import { type Panel, useStore } from '../../store'
import { AngleField, Check, Choice, LengthField, NumberField, PanelShell, Section } from '../fields'
import {
  EDGE,
  FACE,
  OneRef,
  Refs,
  TextField,
  UNPICKED,
  useActiveBox,
  useSheetFeature,
  type SheetFeature,
} from '../sheet-metal/common'
import { CornerBreakEditor } from '../sheet-metal/corners'

/** Bodies of the part as a checklist. */
function BodyList(props: {
  label: string
  value: string[]
  onChange: (ids: string[]) => void
  exclude?: string[]
}) {
  const bodies = useStore((s) => s.scene?.bodies ?? [])
  return (
    <div className="field-group">
      <div className="refbox-label">{props.label}</div>
      {bodies
        .filter((b) => !props.exclude?.includes(b.id))
        .map((b) => (
          <Check
            key={b.id}
            label={b.name}
            value={props.value.includes(b.id)}
            onChange={(on) =>
              props.onChange(on ? [...props.value, b.id] : props.value.filter((x) => x !== b.id))
            }
          />
        ))}
    </div>
  )
}

export function TrimExtendPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'faces'>('faces')
  const { p, set, f } = useSheetFeature<SheetFeature<'trim_extend'>>(
    panel,
    () => ({
      type: 'trim_extend',
      bodies: [],
      boundary: { type: 'bodies', bodies: [], coped: false },
      kind: 'end_trim',
      allow_extension: true,
      gap: 0,
    }),
    (p) =>
      p.bodies.length > 0 &&
      (p.boundary.type === 'faces' ? p.boundary.faces.length > 0 : p.boundary.bodies.length > 0),
  )
  const b = p.boundary
  return (
    <PanelShell title="Trim/Extend" {...f}>
      <Section title="Corner Type">
        <Choice<TrimKind>
          label="Type"
          value={p.kind ?? 'end_trim'}
          options={[
            { value: 'end_trim', label: 'End trim' },
            { value: 'end_miter', label: 'End miter' },
            { value: 'end_butt1', label: 'End butt 1' },
            { value: 'end_butt2', label: 'End butt 2' },
          ]}
          onChange={(kind) => set({ kind })}
        />
      </Section>
      <Section title="Bodies to Be Trimmed">
        <BodyList label="Members" value={p.bodies} onChange={(bodies) => set({ bodies })} />
        <Check
          label="Allow extension"
          value={p.allow_extension ?? true}
          onChange={(allow_extension) => set({ allow_extension })}
        />
      </Section>
      <Section title="Trimming Boundary">
        <Choice
          label="Boundary"
          value={b.type}
          options={[
            { value: 'bodies', label: 'Bodies' },
            { value: 'faces', label: 'Faces or planes' },
          ]}
          onChange={(type) =>
            set({
              boundary: type === 'faces' ? { type, faces: [] } : { type, bodies: [], coped: false },
            })
          }
        />
        {b.type === 'faces' ? (
          <Refs
            label="Faces or planes"
            value={b.faces}
            onChange={(faces) => set({ boundary: { type: 'faces', faces } })}
            kinds={['face', 'datum', 'std_plane']}
            active={box === 'faces'}
            onActivate={() => setBox('faces')}
          />
        ) : (
          <>
            <BodyList
              label="Boundary bodies"
              value={b.bodies}
              exclude={p.bodies}
              onChange={(bodies) => set({ boundary: { ...b, bodies } })}
            />
            {p.kind === 'end_trim' && (
              <Choice
                label="Cut"
                value={b.coped ? 'coped' : 'simple'}
                options={[
                  { value: 'simple', label: 'Simple cut (flush with the face)' },
                  { value: 'coped', label: 'Coped cut (to the body)' },
                ]}
                onChange={(v) => set({ boundary: { ...b, coped: v === 'coped' } })}
              />
            )}
          </>
        )}
        <LengthField
          label="Weld gap"
          value={p.gap ?? 0}
          path="gap"
          onChange={(gap) => set({ gap })}
        />
      </Section>
    </PanelShell>
  )
}

export function EndCapPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'faces'>('faces')
  const { p, set, f } = useSheetFeature<SheetFeature<'end_cap'>>(
    panel,
    () => ({
      type: 'end_cap',
      faces: [],
      direction: { type: 'outward' },
      thickness: 5,
      offset: { type: 'ratio', ratio: 0.5 },
      corners: null,
    }),
    (p) => p.faces.length > 0 && p.thickness > 0,
  )
  const dir: CapDirection = p.direction ?? { type: 'outward' }
  const offset: CapOffset = p.offset ?? { type: 'ratio', ratio: 0.5 }
  return (
    <PanelShell title="End Cap" {...f}>
      <Section title="Parameters">
        <Refs
          label="End faces"
          value={p.faces}
          onChange={(faces) => set({ faces })}
          kinds={FACE}
          active={box === 'faces'}
          onActivate={() => setBox('faces')}
        />
        <Choice
          label="Thickness direction"
          value={dir.type}
          options={[
            { value: 'outward', label: 'Outward' },
            { value: 'inward', label: 'Inward' },
            { value: 'inset', label: 'Inset' },
          ]}
          onChange={(type) => set({ direction: type === 'inset' ? { type, depth: 3 } : { type } })}
        />
        {dir.type === 'inset' && (
          <LengthField
            label="Inset"
            value={dir.depth}
            onChange={(depth) => set({ direction: { ...dir, depth } })}
          />
        )}
        <LengthField
          label="Thickness"
          value={p.thickness}
          path="thickness"
          onChange={(thickness) => set({ thickness })}
        />
      </Section>
      <Section title="Offset">
        <Choice
          label="Offset by"
          value={offset.type}
          options={[
            { value: 'ratio', label: 'Thickness ratio' },
            { value: 'distance', label: 'Distance' },
          ]}
          onChange={(type) =>
            set({ offset: type === 'ratio' ? { type, ratio: 0.5 } : { type, distance: 1 } })
          }
        />
        {offset.type === 'ratio' ? (
          <NumberField
            label="Ratio of wall"
            value={offset.ratio}
            min={0}
            onChange={(ratio) => set({ offset: { ...offset, ratio } })}
          />
        ) : (
          <LengthField
            label="Offset"
            value={offset.distance}
            onChange={(distance) => set({ offset: { ...offset, distance } })}
          />
        )}
      </Section>
      <Section title="Corner Treatment">
        <Check
          label="Chamfer or fillet corners"
          value={!!p.corners}
          onChange={(on) => set({ corners: on ? { type: 'chamfer', distance: 3 } : null })}
        />
        {p.corners && (
          <CornerBreakEditor value={p.corners} onChange={(corners) => set({ corners })} />
        )}
      </Section>
    </PanelShell>
  )
}

export function WeldGussetPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'f1' | 'f2'>('f1')
  const { p, set, f } = useSheetFeature<SheetFeature<'weld_gusset'>>(
    panel,
    () => ({
      type: 'weld_gusset',
      faces: [UNPICKED, UNPICKED],
      shape: { type: 'triangle', d1: 100, d2: 100 },
      chamfer: 0,
      thickness: 10,
      side: 'both',
      location: 'middle',
      offset: 0,
    }),
    (p) => !!p.faces[0] && !!p.faces[1] && p.thickness > 0,
  )
  const s: GussetShape = p.shape
  return (
    <PanelShell title="Gusset" {...f}>
      <Section title="Supporting Faces">
        <OneRef
          label="First face"
          value={p.faces[0]}
          onChange={(r) => r && set({ faces: [r, p.faces[1]] })}
          kinds={FACE}
          active={box === 'f1'}
          onActivate={() => setBox('f1')}
        />
        <OneRef
          label="Second face"
          value={p.faces[1]}
          onChange={(r) => r && set({ faces: [p.faces[0], r] })}
          kinds={FACE}
          active={box === 'f2'}
          onActivate={() => setBox('f2')}
        />
      </Section>
      <Section title="Profile">
        <Choice
          label="Shape"
          value={s.type}
          options={[
            { value: 'triangle', label: 'Triangular' },
            { value: 'polygon', label: 'Polygonal' },
            { value: 'leg_angle', label: 'Triangular, leg and angle' },
          ]}
          onChange={(type) => {
            const d2 = s.type === 'leg_angle' ? s.d1 : s.d2
            set({
              shape:
                type === 'triangle'
                  ? { type, d1: s.d1, d2 }
                  : type === 'polygon'
                    ? { type, d1: s.d1, d2, d3: 20, d4: 20 }
                    : { type, d1: s.d1, angle: Math.PI / 4 },
            })
          }}
        />
        <LengthField
          label="d1 (along the first face)"
          value={s.d1}
          onChange={(d1) => set({ shape: { ...s, d1 } })}
        />
        {s.type === 'leg_angle' ? (
          <AngleField
            label="Angle from the first face"
            value={s.angle}
            onChange={(angle) => set({ shape: { ...s, angle } })}
          />
        ) : (
          <LengthField
            label="d2 (along the second face)"
            value={s.d2}
            onChange={(d2) => set({ shape: { ...s, d2 } })}
          />
        )}
        {s.type === 'polygon' && (
          <>
            <LengthField label="d3" value={s.d3} onChange={(d3) => set({ shape: { ...s, d3 } })} />
            <LengthField label="d4" value={s.d4} onChange={(d4) => set({ shape: { ...s, d4 } })} />
          </>
        )}
        <LengthField
          label="Inner corner chamfer"
          value={p.chamfer ?? 0}
          onChange={(chamfer) => set({ chamfer })}
        />
        <LengthField
          label="Thickness"
          value={p.thickness}
          path="thickness"
          onChange={(thickness) => set({ thickness })}
        />
        <Choice<GussetSide>
          label="Thickness side"
          value={p.side ?? 'both'}
          options={[
            { value: 'inner', label: 'Inner' },
            { value: 'both', label: 'Both' },
            { value: 'outer', label: 'Outer' },
          ]}
          onChange={(side) => set({ side })}
        />
      </Section>
      <Section title="Location">
        <Choice<GussetLocation>
          label="Location"
          value={p.location ?? 'middle'}
          options={[
            { value: 'start', label: 'Start' },
            { value: 'middle', label: 'Middle' },
            { value: 'end', label: 'End' },
          ]}
          onChange={(location) => set({ location })}
        />
        <LengthField
          label="Offset"
          value={p.offset ?? 0}
          min={-1e6}
          onChange={(offset) => set({ offset })}
        />
      </Section>
    </PanelShell>
  )
}

export function WeldBeadPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'edges' | 'from' | 'to'>('from')
  const { p, set, f } = useSheetFeature<SheetFeature<'weld_bead'>>(
    panel,
    () => ({
      type: 'weld_bead',
      path: { type: 'faces', from: [], to: [] },
      kind: panel.variant === 'fillet' ? 'fillet' : 'cosmetic',
      size: 3,
      sides: 'one',
      tangent: false,
      intermittent: null,
      from_to: null,
      symbol: null,
      material: null,
      process: null,
      mass_per_length: null,
    }),
    (p) =>
      p.size > 0 &&
      (p.path.type === 'edges'
        ? p.path.edges.length > 0
        : p.path.type === 'smart'
          ? p.path.faces.length > 1
          : p.path.from.length > 0 && p.path.to.length > 0),
  )
  const path = p.path
  const i = p.intermittent
  return (
    <PanelShell title={p.kind === 'fillet' ? 'Fillet Bead' : 'Weld Bead'} {...f}>
      <Section title="Weld Path">
        <Choice<BeadKind>
          label="Bead"
          value={p.kind ?? 'cosmetic'}
          options={[
            { value: 'cosmetic', label: 'Cosmetic' },
            { value: 'fillet', label: 'Modeled fillet bead' },
          ]}
          onChange={(kind) => set({ kind })}
        />
        <Choice
          label="Select by"
          value={path.type}
          options={[
            { value: 'faces', label: 'Weld from / to faces' },
            { value: 'smart', label: 'Smart weld selection' },
            { value: 'edges', label: 'Weld path edges' },
          ]}
          onChange={(type) =>
            set({
              path:
                type === 'edges'
                  ? { type, edges: [] }
                  : type === 'smart'
                    ? { type, faces: [] }
                    : { type, from: [], to: [] },
            })
          }
        />
        {path.type === 'smart' ? (
          <>
            <Refs
              label="Faces across the joint"
              value={path.faces}
              onChange={(faces) => set({ path: { type: 'smart', faces } })}
              kinds={FACE}
              active={box === 'from'}
              onActivate={() => setBox('from')}
            />
            <p className="hint">Every seam where two of these faces, on different bodies, meet.</p>
          </>
        ) : path.type === 'edges' ? (
          <>
            <Refs
              label="Edges"
              value={path.edges}
              onChange={(edges) => set({ path: { type: 'edges', edges } })}
              kinds={EDGE}
              active={box === 'edges'}
              onActivate={() => setBox('edges')}
            />
            <Check
              label="Tangent propagation"
              value={!!p.tangent}
              onChange={(tangent) => set({ tangent })}
            />
          </>
        ) : (
          <>
            <Refs
              label="Weld from"
              value={path.from}
              onChange={(from) => set({ path: { ...path, from } })}
              kinds={FACE}
              active={box === 'from'}
              onActivate={() => setBox('from')}
            />
            <Refs
              label="Weld to"
              value={path.to}
              onChange={(to) => set({ path: { ...path, to } })}
              kinds={FACE}
              active={box === 'to'}
              onActivate={() => setBox('to')}
            />
          </>
        )}
      </Section>
      <Section title="Settings">
        <LengthField
          label="Bead size"
          value={p.size}
          path="size"
          onChange={(size) => set({ size })}
        />
        <Choice<BeadSides>
          label="Sides"
          value={p.sides ?? 'one'}
          options={[
            { value: 'one', label: 'This side' },
            { value: 'both', label: 'Both sides' },
          ]}
          onChange={(sides) => set({ sides })}
        />
        <Check
          label="Intermittent"
          value={!!i}
          onChange={(on) =>
            set({ intermittent: on ? { length: 20, gap: 20, staggered: false } : null })
          }
        />
        {i && (
          <>
            <LengthField
              label="Weld length"
              value={i.length}
              onChange={(length) => set({ intermittent: { ...i, length } })}
            />
            <LengthField
              label="Gap"
              value={i.gap}
              onChange={(gap) => set({ intermittent: { ...i, gap } })}
            />
            <Check
              label="Staggered"
              value={!!i.staggered}
              onChange={(staggered) => set({ intermittent: { ...i, staggered } })}
            />
          </>
        )}
        <Check
          label="From/to length"
          value={!!p.from_to}
          onChange={(on) => set({ from_to: on ? [0, 50] : null })}
        />
        {p.from_to && (
          <>
            <LengthField
              label="Start"
              value={p.from_to[0]}
              onChange={(a) => set({ from_to: [a, p.from_to![1]] })}
            />
            <LengthField
              label="End"
              value={p.from_to[1]}
              onChange={(b) => set({ from_to: [p.from_to![0], b] })}
            />
          </>
        )}
      </Section>
      <Section title="Symbol and Weld Folder" open={false}>
        <TextField
          label="Weld symbol"
          value={p.symbol ?? ''}
          onChange={(symbol) => set({ symbol: symbol || null })}
        />
        <TextField
          label="Weld material"
          value={p.material ?? ''}
          onChange={(material) => set({ material: material || null })}
        />
        <TextField
          label="Process"
          value={p.process ?? ''}
          onChange={(process) => set({ process: process || null })}
        />
        <NumberField
          label="Mass per length"
          unit="kg/m"
          value={p.mass_per_length ?? 0}
          min={0}
          onChange={(m) => set({ mass_per_length: m || null })}
        />
      </Section>
    </PanelShell>
  )
}

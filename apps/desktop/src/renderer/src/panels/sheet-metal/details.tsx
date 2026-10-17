import type { FormShape, TabCorner } from '../../../../shared/tools.gen'
import type { Panel } from '../../store'
import {
  AngleField,
  Check,
  Choice,
  IntField,
  LengthField,
  NumberField,
  PanelShell,
  Section,
} from '../fields'
import { CornerBreakEditor } from './corners'
import {
  EDGE,
  FACE,
  OneRef,
  POINT,
  Refs,
  SketchPick,
  TextField,
  UNPICKED,
  useActiveBox,
  useSheetFeature,
  type SheetFeature,
} from './common'

export function TabAndSlotPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'edge' | 'face' | 'start' | 'end'>('edge')
  const { p, set, f } = useSheetFeature<SheetFeature<'tab_and_slot'>>(
    panel,
    () => ({
      type: 'tab_and_slot',
      edge: UNPICKED,
      face: UNPICKED,
      count: 2,
      spacing: null,
      length: 10,
      height: null,
      clearance: 0.1,
      length_clearance: null,
      width_clearance: null,
      start: null,
      end: null,
      corners: { type: 'sharp' },
      through_all: false,
    }),
    (p) => !!p.edge && !!p.face && p.length > 0,
  )
  const c: TabCorner = p.corners ?? { type: 'sharp' }
  return (
    <PanelShell title="Tab and Slot" {...f}>
      <Section title="Selections">
        <OneRef
          label="Tab edge (on the tab body)"
          value={p.edge}
          onChange={(edge) => edge && set({ edge })}
          kinds={EDGE}
          active={box === 'edge'}
          onActivate={() => setBox('edge')}
        />
        <OneRef
          label="Slot face (on the other body)"
          value={p.face}
          onChange={(face) => face && set({ face })}
          kinds={FACE}
          active={box === 'face'}
          onActivate={() => setBox('face')}
        />
        <OneRef
          label="Start point (optional)"
          value={p.start}
          onChange={(start) => set({ start })}
          kinds={POINT}
          active={box === 'start'}
          onActivate={() => setBox('start')}
        />
        <OneRef
          label="End point (optional)"
          value={p.end}
          onChange={(end) => set({ end })}
          kinds={POINT}
          active={box === 'end'}
          onActivate={() => setBox('end')}
        />
      </Section>
      <Section title="Spacing">
        <Choice
          label="Spacing"
          value={p.spacing ? 'distance' : 'count'}
          options={[
            { value: 'count', label: 'Equal, by count' },
            { value: 'distance', label: 'Equal, by distance' },
          ]}
          onChange={(v) => set({ spacing: v === 'distance' ? 30 : null })}
        />
        {p.spacing ? (
          <LengthField
            label="Spacing"
            value={p.spacing}
            path="spacing"
            onChange={(spacing) => set({ spacing })}
          />
        ) : (
          <IntField
            label="Tabs"
            value={p.count ?? 2}
            path="count"
            onChange={(count) => set({ count })}
          />
        )}
      </Section>
      <Section title="Tab">
        <LengthField
          label="Length"
          value={p.length}
          path="length"
          onChange={(length) => set({ length })}
        />
        <Check
          label="Height past the slot body"
          value={p.height !== null && p.height !== undefined}
          onChange={(on) => set({ height: on ? 2 : null })}
        />
        {p.height !== null && p.height !== undefined && (
          <LengthField
            label="Height"
            value={p.height}
            path="height"
            onChange={(height) => set({ height })}
          />
        )}
        <Choice
          label="Corners"
          value={c.type}
          options={[
            { value: 'sharp', label: 'Sharp' },
            { value: 'fillet', label: 'Fillet' },
            { value: 'chamfer', label: 'Chamfer' },
          ]}
          onChange={(type) =>
            set({
              corners:
                type === 'fillet'
                  ? { type, radius: 0.5 }
                  : type === 'chamfer'
                    ? { type, distance: 0.5 }
                    : { type },
            })
          }
        />
        {c.type === 'fillet' && (
          <LengthField
            label="Fillet radius"
            value={c.radius}
            onChange={(radius) => set({ corners: { ...c, radius } })}
          />
        )}
        {c.type === 'chamfer' && (
          <LengthField
            label="Chamfer distance"
            value={c.distance}
            onChange={(distance) => set({ corners: { ...c, distance } })}
          />
        )}
      </Section>
      <Section title="Slot">
        <LengthField
          label="Clearance"
          value={p.clearance ?? 0.1}
          path="clearance"
          onChange={(clearance) => set({ clearance })}
        />
        <Check
          label="Separate length and width clearance"
          value={p.length_clearance !== null && p.length_clearance !== undefined}
          onChange={(on) =>
            set(
              on
                ? { length_clearance: p.clearance ?? 0.1, width_clearance: p.clearance ?? 0.1 }
                : { length_clearance: null, width_clearance: null },
            )
          }
        />
        {p.length_clearance !== null && p.length_clearance !== undefined && (
          <>
            <LengthField
              label="Length clearance"
              value={p.length_clearance}
              onChange={(length_clearance) => set({ length_clearance })}
            />
            <LengthField
              label="Width clearance"
              value={p.width_clearance ?? 0}
              onChange={(width_clearance) => set({ width_clearance })}
            />
          </>
        )}
        <Check
          label="Through all"
          value={!!p.through_all}
          onChange={(through_all) => set({ through_all })}
        />
      </Section>
    </PanelShell>
  )
}

export function GussetPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'bend'>('bend')
  const { p, set, f } = useSheetFeature<SheetFeature<'sheet_metal_gusset'>>(
    panel,
    () => ({
      type: 'sheet_metal_gusset',
      bend: UNPICKED,
      position: 0.5,
      depth: 8,
      depth2: null,
      angle: null,
      rounded_back: false,
      width: 4,
      punch_id: null,
    }),
    (p) => !!p.bend && p.depth > 0 && p.width > 0,
  )
  const mode = p.angle !== null && p.angle !== undefined ? 'angle' : p.depth2 ? 'two' : 'even'
  return (
    <PanelShell title="Sheet Metal Gusset" {...f}>
      <Section title="Position">
        <OneRef
          label="Bend face"
          value={p.bend}
          onChange={(bend) => bend && set({ bend })}
          kinds={FACE}
          active={box === 'bend'}
          onActivate={() => setBox('bend')}
        />
        <NumberField
          label="Along the bend (0 to 1)"
          value={p.position ?? 0.5}
          min={0}
          path="position"
          onChange={(position) => set({ position: Math.min(1, position) })}
        />
      </Section>
      <Section title="Profile">
        <Choice
          label="Size by"
          value={mode}
          options={[
            { value: 'even', label: 'Equal legs' },
            { value: 'two', label: 'Two legs' },
            { value: 'angle', label: 'Leg and angle' },
          ]}
          onChange={(v) =>
            set(
              v === 'angle'
                ? { angle: Math.PI / 4, depth2: null }
                : v === 'two'
                  ? { depth2: p.depth, angle: null }
                  : { depth2: null, angle: null },
            )
          }
        />
        <LengthField
          label={mode === 'even' ? 'Leg' : 'First leg'}
          value={p.depth}
          path="depth"
          onChange={(depth) => set({ depth })}
        />
        {mode === 'two' && (
          <LengthField
            label="Second leg"
            value={p.depth2 ?? p.depth}
            path="depth2"
            onChange={(depth2) => set({ depth2 })}
          />
        )}
        {mode === 'angle' && (
          <AngleField
            label="Angle off the first wall"
            value={p.angle ?? Math.PI / 4}
            path="angle"
            onChange={(angle) => set({ angle })}
          />
        )}
        <LengthField
          label="Width"
          value={p.width}
          path="width"
          onChange={(width) => set({ width })}
        />
        <Check
          label="Rounded back"
          value={!!p.rounded_back}
          onChange={(rounded_back) => set({ rounded_back })}
        />
        <TextField
          label="Punch ID"
          value={p.punch_id ?? ''}
          onChange={(punch_id) => set({ punch_id: punch_id || null })}
        />
      </Section>
    </PanelShell>
  )
}

const SHAPES: { value: FormShape['type']; label: string }[] = [
  { value: 'louver', label: 'Louver' },
  { value: 'lance', label: 'Lance' },
  { value: 'emboss', label: 'Emboss' },
  { value: 'dimple', label: 'Dimple (extruded flange)' },
  { value: 'rib', label: 'Rib' },
  { value: 'file', label: 'Forming tool part…' },
]

function shapeDefaults(type: FormShape['type']): FormShape {
  switch (type) {
    case 'louver':
    case 'lance':
      return { type, length: 20, width: 5, height: 2 }
    case 'rib':
      return { type, length: 30, width: 6, height: 2 }
    case 'emboss':
    case 'dimple':
      return { type, diameter: 10, height: 2 }
    case 'file':
      return { type, path: '', configuration: null }
  }
}

export function FormToolPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'face'>('face')
  const { p, set, f } = useSheetFeature<SheetFeature<'form_tool'>>(
    panel,
    () => ({
      type: 'form_tool',
      shape: shapeDefaults('louver'),
      face: UNPICKED,
      sketch: null,
      rotation: 0,
      flip: false,
      punch_id: null,
    }),
    (p) => !!p.face && (p.shape.type !== 'file' || p.shape.path.length > 0),
  )
  const s = p.shape
  const setShape = (patch: Partial<FormShape>) => set({ shape: { ...s, ...patch } as FormShape })
  return (
    <PanelShell title="Forming Tool" {...f}>
      <Section title="Tool">
        <Choice
          label="Tool"
          value={s.type}
          options={SHAPES}
          onChange={(t) => set({ shape: shapeDefaults(t) })}
        />
        {'length' in s && (
          <LengthField
            label="Length"
            value={s.length}
            onChange={(length) => setShape({ length })}
          />
        )}
        {'width' in s && (
          <LengthField label="Width" value={s.width} onChange={(width) => setShape({ width })} />
        )}
        {'diameter' in s && (
          <LengthField
            label="Diameter"
            value={s.diameter}
            onChange={(diameter) => setShape({ diameter })}
          />
        )}
        {'height' in s && (
          <LengthField
            label="Height"
            value={s.height}
            onChange={(height) => setShape({ height })}
          />
        )}
        {s.type === 'file' && (
          <>
            <TextField
              label="Tool part (.ofp)"
              value={s.path}
              placeholder="path/to/tool.ofp"
              onChange={(path) => setShape({ path })}
            />
            <Check
              label="Configuration"
              value={s.configuration !== null && s.configuration !== undefined}
              onChange={(on) => setShape({ configuration: on ? 1 : null })}
            />
            {s.configuration !== null && s.configuration !== undefined && (
              <IntField
                label="Configuration ID"
                value={s.configuration}
                onChange={(configuration) => setShape({ configuration })}
              />
            )}
          </>
        )}
      </Section>
      <Section title="Placement">
        <OneRef
          label="Face"
          value={p.face}
          onChange={(face) => face && set({ face })}
          kinds={FACE}
          active={box === 'face'}
          onActivate={() => setBox('face')}
        />
        <Check
          label="Place at sketch points"
          value={!!p.sketch}
          onChange={(on) => set({ sketch: on ? (panel.sketch ?? null) : null })}
        />
        {p.sketch !== null && p.sketch !== undefined && (
          <SketchPick
            label="Placement sketch"
            value={p.sketch}
            onChange={(sketch) => set({ sketch })}
          />
        )}
        <AngleField
          label="Rotation"
          value={p.rotation ?? 0}
          path="rotation"
          onChange={(rotation) => set({ rotation })}
        />
        <Check label="Flip tool" value={!!p.flip} onChange={(flip) => set({ flip })} />
        <TextField
          label="Punch ID"
          value={p.punch_id ?? ''}
          onChange={(punch_id) => set({ punch_id: punch_id || null })}
        />
      </Section>
    </PanelShell>
  )
}

export function FormingToolPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'stop' | 'remove' | 'insert'>('stop')
  const { p, set, f } = useSheetFeature<SheetFeature<'forming_tool'>>(
    panel,
    () => ({
      type: 'forming_tool',
      stopping_face: UNPICKED,
      remove_faces: [],
      insertion_point: null,
    }),
    (p) => !!p.stopping_face,
  )
  return (
    <PanelShell title="Forming Tool Definition" {...f}>
      <Section title="Type">
        <OneRef
          label="Stopping face"
          value={p.stopping_face}
          onChange={(stopping_face) => stopping_face && set({ stopping_face })}
          kinds={FACE}
          active={box === 'stop'}
          onActivate={() => setBox('stop')}
        />
        <Refs
          label="Faces to remove (openings)"
          value={p.remove_faces ?? []}
          onChange={(remove_faces) => set({ remove_faces })}
          kinds={FACE}
          active={box === 'remove'}
          onActivate={() => setBox('remove')}
        />
        <OneRef
          label="Insertion point (optional)"
          value={p.insertion_point}
          onChange={(insertion_point) => set({ insertion_point })}
          kinds={POINT}
          active={box === 'insert'}
          onActivate={() => setBox('insert')}
        />
      </Section>
      <p className="hint">
        Makes this part a forming tool: other parts press it into a sheet with the Forming Tool
        command.
      </p>
    </PanelShell>
  )
}

export function FlatPatternPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'fixed' | 'exclude'>('fixed')
  const { p, set, f } = useSheetFeature<SheetFeature<'flat_pattern'>>(
    panel,
    () => ({
      type: 'flat_pattern',
      fixed_face: null,
      break_corners: null,
      split_bend_regions: false,
      simplify_bends: false,
      corner_treatment: false,
      exclude: [],
    }),
    () => true,
  )
  return (
    <PanelShell title="Flat-Pattern" {...f}>
      <Section title="Parameters">
        <OneRef
          label="Fixed face (each body's own when empty)"
          value={p.fixed_face}
          onChange={(fixed_face) => set({ fixed_face })}
          kinds={FACE}
          active={box === 'fixed'}
          onActivate={() => setBox('fixed')}
        />
        <Check
          label="Merge faces"
          value={!p.split_bend_regions}
          onChange={(merge) => set({ split_bend_regions: !merge })}
        />
        <Check
          label="Simplify bends"
          value={!!p.simplify_bends}
          onChange={(simplify_bends) => set({ simplify_bends })}
        />
        <Check
          label="Corner treatment"
          value={!!p.corner_treatment}
          onChange={(corner_treatment) => set({ corner_treatment })}
        />
        <Refs
          label="Faces to exclude"
          value={p.exclude ?? []}
          onChange={(exclude) => set({ exclude })}
          kinds={FACE}
          active={box === 'exclude'}
          onActivate={() => setBox('exclude')}
        />
      </Section>
      <Section title="Corner Options">
        <Check
          label="Break corners"
          value={!!p.break_corners}
          onChange={(on) => set({ break_corners: on ? { type: 'chamfer', distance: 2 } : null })}
        />
        {p.break_corners && (
          <CornerBreakEditor
            value={p.break_corners}
            onChange={(break_corners) => set({ break_corners })}
          />
        )}
      </Section>
    </PanelShell>
  )
}

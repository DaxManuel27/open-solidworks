import type { FlangeLength, HemKind } from '../../../../shared/tools.gen'
import type { Panel } from '../../store'
import { EndEditor } from '../features'
import { AngleField, Check, Choice, IntField, LengthField, PanelShell, Section } from '../fields'
import {
  BendOverrideEditor,
  EDGE,
  OneRef,
  Refs,
  UNPICKED,
  endDraft,
  endFrom,
  POINT,
  PLANE_FACE,
  SheetParamsEditor,
  SketchPick,
  defaultSheet,
  partSheet,
  startSketch,
  useActiveBox,
  useSheetFeature,
  type SheetFeature,
} from './common'

const POSITIONS = [
  { value: 'material_inside', label: 'Material inside' },
  { value: 'material_outside', label: 'Material outside' },
  { value: 'bend_outside', label: 'Bend outside' },
  { value: 'bend_from_virtual_sharp', label: 'Bend from virtual sharp' },
  { value: 'tangent_to_bend', label: 'Tangent to bend' },
] as const

export function BaseFlangePanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'end1' | 'end2'>('end1')
  const { p, set, f } = useSheetFeature<SheetFeature<'base_flange'>>(
    panel,
    () => ({
      type: 'base_flange',
      sketch: startSketch(panel),
      sheet: partSheet() ?? defaultSheet(),
      side: 'normal',
      end1: null,
      end2: null,
      reverse: false,
    }),
    (p) => p.sketch > 0 && p.sheet.thickness > 0,
  )
  return (
    <PanelShell title="Base Flange/Tab" {...f}>
      <Section title="Profile">
        <SketchPick label="Sketch" value={p.sketch} onChange={(sketch) => set({ sketch })} />
        <Choice
          label="Thickness direction"
          value={p.side ?? 'normal'}
          options={[
            { value: 'normal', label: 'Along the sketch normal' },
            { value: 'reverse', label: 'Reverse' },
            { value: 'mid_plane', label: 'Mid plane' },
          ]}
          onChange={(side) => set({ side })}
        />
      </Section>
      <Section title="Open Profile Depth">
        <Check
          label="Extrude an open profile"
          value={!!p.end1}
          onChange={(on) => set({ end1: on ? { type: 'blind', depth: 20 } : null, end2: null })}
        />
        {p.end1 && (
          <>
            <EndEditor
              value={endDraft(p.end1)}
              onChange={(d) => set({ end1: endFrom(d) ?? p.end1 })}
              allowMid
              active={box === 'end1'}
              onActivate={() => setBox('end1')}
              path="end1"
            />
            <Check
              label="Reverse direction"
              value={!!p.reverse}
              onChange={(reverse) => set({ reverse })}
            />
            <Check
              label="Direction 2"
              value={!!p.end2}
              onChange={(on) => set({ end2: on ? { type: 'blind', depth: 10 } : null })}
            />
            {p.end2 && (
              <EndEditor
                value={endDraft(p.end2)}
                onChange={(d) => set({ end2: endFrom(d) ?? p.end2 })}
                allowMid={false}
                active={box === 'end2'}
                onActivate={() => setBox('end2')}
                path="end2"
              />
            )}
          </>
        )}
      </Section>
      <SheetParamsEditor value={p.sheet} onChange={(sheet) => set({ sheet })} path="sheet" />
      <p className="hint">
        A closed sketch makes a tab (merged with a sheet it lies on); an open one is thickened and
        bent at every corner.
      </p>
    </PanelShell>
  )
}

function FlangeLengthEditor(props: {
  value: FlangeLength
  onChange: (l: FlangeLength) => void
  active: boolean
  onActivate: () => void
}) {
  const l = props.value
  const target = 'target' in l ? l.target : null
  return (
    <>
      <Choice
        label="Length end condition"
        value={l.type}
        options={[
          { value: 'blind', label: 'Blind' },
          { value: 'up_to_vertex', label: 'Up to vertex' },
          { value: 'up_to_surface', label: 'Up to surface' },
          { value: 'offset_from_surface', label: 'Offset from surface' },
          { value: 'up_to_edge_and_merge', label: 'Up to edge and merge' },
        ]}
        onChange={(type) =>
          props.onChange(
            type === 'blind'
              ? { type, length: 'length' in l ? l.length : 20 }
              : type === 'offset_from_surface'
                ? { type, target: target ?? UNPICKED, offset: 1 }
                : { type, target: target ?? UNPICKED },
          )
        }
      />
      {l.type === 'blind' && (
        <LengthField
          label="Flange length"
          value={l.length}
          path="length.length"
          onChange={(length) => props.onChange({ ...l, length })}
        />
      )}
      {l.type !== 'blind' && (
        <OneRef
          label={
            l.type === 'up_to_vertex'
              ? 'Vertex'
              : l.type === 'up_to_edge_and_merge'
                ? 'Edge'
                : 'Face or plane'
          }
          value={target}
          onChange={(r) => r && props.onChange({ ...l, target: r } as FlangeLength)}
          kinds={
            l.type === 'up_to_vertex'
              ? POINT
              : l.type === 'up_to_edge_and_merge'
                ? EDGE
                : PLANE_FACE
          }
          active={props.active}
          onActivate={props.onActivate}
        />
      )}
      {l.type === 'offset_from_surface' && (
        <LengthField
          label="Offset"
          value={l.offset}
          path="length.offset"
          onChange={(offset) => props.onChange({ ...l, offset })}
        />
      )}
    </>
  )
}

export function EdgeFlangePanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'edges' | 'length' | 'angle'>('edges')
  const { p, set, f } = useSheetFeature<SheetFeature<'edge_flange'>>(
    panel,
    () => ({
      type: 'edge_flange',
      edges: [],
      angle: Math.PI / 2,
      length: { type: 'blind', length: 20 },
      measure: 'outer_virtual_sharp',
      position: 'material_inside',
      reverse: false,
      gap: 0.1,
      start_offset: 0,
      end_offset: 0,
      position_offset: 0,
      bend: {},
      angle_from: null,
      profiles: [],
    }),
    (p) =>
      p.edges.length > 0 &&
      ('target' in p.length ? !!p.length.target : true) &&
      (!p.angle_from || !!p.angle_from.face),
  )
  return (
    <PanelShell title="Edge-Flange" {...f}>
      <Section title="Flange Parameters">
        <Refs
          label="Edges"
          value={p.edges}
          onChange={(edges) => set({ edges })}
          kinds={EDGE}
          active={box === 'edges'}
          onActivate={() => setBox('edges')}
        />
        <LengthField label="Gap" value={p.gap ?? 0.1} path="gap" onChange={(gap) => set({ gap })} />
        <Check label="Reverse side" value={!!p.reverse} onChange={(reverse) => set({ reverse })} />
      </Section>
      <Section title="Angle">
        <Check
          label="Set from a face"
          value={!!p.angle_from}
          onChange={(on) => set({ angle_from: on ? { face: UNPICKED, parallel: false } : null })}
        />
        {p.angle_from ? (
          <>
            <OneRef
              label="Face"
              value={p.angle_from.face}
              onChange={(face) => face && set({ angle_from: { ...p.angle_from!, face } })}
              kinds={PLANE_FACE}
              active={box === 'angle'}
              onActivate={() => setBox('angle')}
            />
            <Choice
              label="Flange runs"
              value={p.angle_from.parallel ? 'parallel' : 'square'}
              options={[
                { value: 'square', label: 'Square to the face' },
                { value: 'parallel', label: 'Parallel to the face' },
              ]}
              onChange={(v) =>
                set({ angle_from: { ...p.angle_from!, parallel: v === 'parallel' } })
              }
            />
          </>
        ) : (
          <AngleField
            label="Angle"
            value={p.angle ?? Math.PI / 2}
            path="angle"
            onChange={(angle) => set({ angle })}
          />
        )}
      </Section>
      <Section title="Flange Length">
        <FlangeLengthEditor
          value={p.length}
          onChange={(length) => set({ length })}
          active={box === 'length'}
          onActivate={() => setBox('length')}
        />
        {p.length.type === 'blind' && (
          <Choice
            label="Measured from"
            value={p.measure ?? 'outer_virtual_sharp'}
            options={[
              { value: 'outer_virtual_sharp', label: 'Outer virtual sharp' },
              { value: 'inner_virtual_sharp', label: 'Inner virtual sharp' },
              { value: 'tangent_to_bend', label: 'Tangent to bend' },
            ]}
            onChange={(measure) => set({ measure })}
          />
        )}
      </Section>
      <Section title="Flange Position">
        <Choice
          label="Position"
          value={p.position ?? 'material_inside'}
          options={[...POSITIONS]}
          onChange={(position) => set({ position })}
        />
        <LengthField
          label="Offset into the face"
          value={p.position_offset ?? 0}
          path="position_offset"
          onChange={(position_offset) => set({ position_offset })}
        />
        <LengthField
          label="Start offset"
          value={p.start_offset ?? 0}
          path="start_offset"
          onChange={(start_offset) => set({ start_offset })}
        />
        <LengthField
          label="End offset"
          value={p.end_offset ?? 0}
          path="end_offset"
          onChange={(end_offset) => set({ end_offset })}
        />
      </Section>
      <ProfileEditor
        edges={p.edges.length}
        profiles={p.profiles ?? []}
        onChange={(profiles) => set({ profiles })}
      />
      <BendOverrideEditor value={p.bend} onChange={(bend) => set({ bend })} />
    </PanelShell>
  )
}

/** Wall outlines per edge: points along the edge (u) and up the wall (v). */
function ProfileEditor(props: {
  edges: number
  profiles: NonNullable<SheetFeature<'edge_flange'>['profiles']>
  onChange: (p: NonNullable<SheetFeature<'edge_flange'>['profiles']>) => void
}) {
  if (props.edges === 0) return null
  return (
    <Section title="Flange Profiles" open={false}>
      {Array.from({ length: props.edges }, (_, edge) => {
        const profile = props.profiles.find((q) => q.edge === edge)
        const text = profile?.points.map(([u, v]) => `${u} ${v}`).join(', ') ?? ''
        return (
          <label className="field" key={edge}>
            <span>Edge {edge + 1} outline</span>
            <input
              defaultValue={text}
              placeholder="u v, u v, … (rectangle when empty)"
              onBlur={(e) => {
                const points = e.target.value
                  .split(',')
                  .map((pair) => pair.trim().split(/\s+/).map(Number))
                  .filter((q) => q.length === 2 && q.every(Number.isFinite)) as [number, number][]
                const rest = props.profiles.filter((q) => q.edge !== edge)
                props.onChange(points.length >= 3 ? [...rest, { edge, points }] : rest)
              }}
            />
          </label>
        )
      })}
      <p className="hint">
        u runs along the edge from its start, v up the wall from the bend; the outline must rest on
        v = 0.
      </p>
    </Section>
  )
}

export function MiterFlangePanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'edges'>('edges')
  const { p, set, f } = useSheetFeature<SheetFeature<'miter_flange'>>(
    panel,
    () => ({
      type: 'miter_flange',
      sketch: startSketch(panel),
      edges: [],
      position: 'material_inside',
      gap: 0.1,
      start_offset: 0,
      end_offset: 0,
      bend: {},
    }),
    (p) => p.sketch > 0 && p.edges.length > 0,
  )
  return (
    <PanelShell title="Miter Flange" {...f}>
      <Section title="Miter Parameters">
        <SketchPick label="Sketch" value={p.sketch} onChange={(sketch) => set({ sketch })} />
        <Refs
          label="Edges along the profile's path"
          value={p.edges}
          onChange={(edges) => set({ edges })}
          kinds={EDGE}
          active={box === 'edges'}
          onActivate={() => setBox('edges')}
        />
        <Choice
          label="Position"
          value={p.position ?? 'material_inside'}
          options={[...POSITIONS]}
          onChange={(position) => set({ position })}
        />
        <LengthField label="Gap" value={p.gap ?? 0.1} path="gap" onChange={(gap) => set({ gap })} />
      </Section>
      <Section title="Start/End Offset">
        <LengthField
          label="Start offset"
          value={p.start_offset ?? 0}
          path="start_offset"
          onChange={(start_offset) => set({ start_offset })}
        />
        <LengthField
          label="End offset"
          value={p.end_offset ?? 0}
          path="end_offset"
          onChange={(end_offset) => set({ end_offset })}
        />
      </Section>
      <BendOverrideEditor value={p.bend} onChange={(bend) => set({ bend })} />
      <p className="hint">
        Sketch the flange's profile on a plane square to the first edge, at its end.
      </p>
    </PanelShell>
  )
}

const HEM_KINDS: { value: HemKind['type']; label: string }[] = [
  { value: 'closed', label: 'Closed' },
  { value: 'open', label: 'Open' },
  { value: 'tear_drop', label: 'Tear drop' },
  { value: 'rolled', label: 'Rolled' },
  { value: 'double', label: 'Double' },
]

export function HemPanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'edges'>('edges')
  const { p, set, f } = useSheetFeature<SheetFeature<'hem'>>(
    panel,
    () => ({
      type: 'hem',
      edges: [],
      kind: { type: 'closed' },
      length: 10,
      position: 'material_inside',
      reverse: false,
      gap: 0.1,
      allowance: null,
    }),
    (p) => p.edges.length > 0,
  )
  const k = p.kind
  return (
    <PanelShell title="Hem" {...f}>
      <Section title="Edges">
        <Refs
          label="Edges"
          value={p.edges}
          onChange={(edges) => set({ edges })}
          kinds={EDGE}
          active={box === 'edges'}
          onActivate={() => setBox('edges')}
        />
        <Choice
          label="Position"
          value={p.position ?? 'material_inside'}
          options={[
            { value: 'material_inside', label: 'Material inside' },
            { value: 'bend_outside', label: 'Bend outside' },
          ]}
          onChange={(position) => set({ position })}
        />
        <Check
          label="Reverse direction"
          value={!!p.reverse}
          onChange={(reverse) => set({ reverse })}
        />
      </Section>
      <Section title="Type and Size">
        <Choice
          label="Type"
          value={k.type}
          options={HEM_KINDS}
          onChange={(type) =>
            set({
              kind:
                type === 'open' || type === 'double'
                  ? { type, gap: 1 }
                  : type === 'tear_drop'
                    ? { type, angle: (Math.PI * 4) / 3, radius: 1 }
                    : type === 'rolled'
                      ? { type, angle: (Math.PI * 3) / 2, radius: 2 }
                      : { type },
            })
          }
        />
        {k.type !== 'rolled' && (
          <LengthField
            label="Length"
            value={p.length ?? 10}
            path="length"
            onChange={(length) => set({ length })}
          />
        )}
        {(k.type === 'open' || k.type === 'double') && (
          <LengthField
            label="Gap distance"
            value={k.gap}
            path="kind.gap"
            onChange={(gap) => set({ kind: { ...k, gap } })}
          />
        )}
        {(k.type === 'tear_drop' || k.type === 'rolled') && (
          <>
            <AngleField
              label="Angle"
              value={k.angle}
              path="kind.angle"
              onChange={(angle) => set({ kind: { ...k, angle } })}
            />
            <LengthField
              label="Radius"
              value={k.radius}
              path="kind.radius"
              onChange={(radius) => set({ kind: { ...k, radius } })}
            />
          </>
        )}
      </Section>
      <Section title="Miter Gap" open={false}>
        <LengthField
          label="Gap where hems meet"
          value={p.gap ?? 0.1}
          path="gap"
          onChange={(gap) => set({ gap })}
        />
      </Section>
    </PanelShell>
  )
}

export function SweptFlangePanel({ panel }: { panel: Panel }) {
  const [box, setBox] = useActiveBox<'path'>('path')
  const { p, set, f } = useSheetFeature<SheetFeature<'swept_flange'>>(
    panel,
    () => ({
      type: 'swept_flange',
      profile: startSketch(panel),
      path: UNPICKED,
      side: 'normal',
      bend: {},
      sheet: partSheet() ? null : defaultSheet(),
    }),
    (p) => p.profile > 0 && !!p.path,
  )
  return (
    <PanelShell title="Swept Flange" {...f}>
      <Section title="Profile and Path">
        <SketchPick label="Profile" value={p.profile} onChange={(profile) => set({ profile })} />
        <OneRef
          label="Path (sketch or edges)"
          value={p.path}
          onChange={(path) => path && set({ path })}
          kinds={['edge', 'datum']}
          active={box === 'path'}
          onActivate={() => setBox('path')}
        />
        <Choice
          label="Thickness direction"
          value={p.side ?? 'normal'}
          options={[
            { value: 'normal', label: 'Normal' },
            { value: 'reverse', label: 'Reverse' },
            { value: 'mid_plane', label: 'Mid plane' },
          ]}
          onChange={(side) => set({ side })}
        />
      </Section>
      {p.sheet && (
        <SheetParamsEditor value={p.sheet} onChange={(sheet) => set({ sheet })} path="sheet" />
      )}
      <BendOverrideEditor value={p.bend} onChange={(bend) => set({ bend })} relief={false} />
    </PanelShell>
  )
}

export function LoftedBendPanel({ panel }: { panel: Panel }) {
  const { p, set, f } = useSheetFeature<SheetFeature<'lofted_bend'>>(
    panel,
    () => ({
      type: 'lofted_bend',
      profile1: startSketch(panel),
      profile2: 0,
      sheet: partSheet() ?? defaultSheet(),
      method: 'formed',
      facets: 12,
      reverse: false,
    }),
    (p) => p.profile1 > 0 && p.profile2 > 0 && p.profile1 !== p.profile2,
  )
  return (
    <PanelShell title="Lofted-Bend" {...f}>
      <Section title="Profiles">
        <SketchPick
          label="First profile"
          value={p.profile1}
          onChange={(profile1) => set({ profile1 })}
        />
        <SketchPick
          label="Second profile"
          value={p.profile2}
          onChange={(profile2) => set({ profile2 })}
        />
        <Check
          label="Reverse thickness"
          value={!!p.reverse}
          onChange={(reverse) => set({ reverse })}
        />
      </Section>
      <Section title="Manufacturing Method">
        <Choice
          label="Method"
          value={p.method ?? 'formed'}
          options={[
            { value: 'formed', label: 'Formed' },
            { value: 'bent', label: 'Bent (facets)' },
          ]}
          onChange={(method) => set({ method })}
        />
        {p.method === 'bent' && (
          <IntField
            label="Facets per bend"
            value={p.facets ?? 12}
            path="facets"
            onChange={(facets) => set({ facets })}
          />
        )}
      </Section>
      <SheetParamsEditor value={p.sheet} onChange={(sheet) => set({ sheet })} path="sheet" />
    </PanelShell>
  )
}

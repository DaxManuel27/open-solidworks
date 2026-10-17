import { useEffect, useState } from 'react'
import type {
  CompRef,
  DetailStyle,
  DisplayStyle,
  FlatView,
  Orientation,
  Profile,
  Scale,
  SectionKind,
  TangentEdges,
  ViewKind,
} from '../../../../shared/tools.gen'
import { call } from '../../core'
import { Check, Choice, LengthField, NumberField, PanelShell, Section } from '../../panels/fields'
import { useStore } from '../../store'
import { type SheetPick, useDrawing } from '../state'
import type { Break, BreakStyle, DrawingView, P2 } from '../types'
import { scaleLabel } from '../types'
import {
  Prompt,
  ScaleField,
  TextField,
  closeDrawingPanel,
  entityLabel,
  fromViewPlane,
  placementToward,
  toViewPlane,
  useOverlay,
  useSheet,
  useSheetPicks,
  viewInfo,
} from './common'

const ORIENTATIONS: Orientation['type'][] = [
  'front',
  'back',
  'left',
  'right',
  'top',
  'bottom',
  'isometric',
  'dimetric',
  'trimetric',
  'relative',
]

const DISPLAY: { value: DisplayStyle; label: string }[] = [
  { value: 'hidden_removed', label: 'Hidden lines removed' },
  { value: 'hidden_visible', label: 'Hidden lines visible' },
  { value: 'wireframe', label: 'Wireframe' },
  { value: 'shaded_edges', label: 'Shaded with edges' },
  { value: 'shaded', label: 'Shaded' },
]

export const HATCHES = [
  { value: 'general', label: 'General (45° lines)' },
  { value: 'steel', label: 'Steel' },
  { value: 'bronze', label: 'Bronze' },
  { value: 'aluminium', label: 'Aluminium' },
  { value: 'plastic', label: 'Plastic' },
  { value: 'insulation', label: 'Insulation' },
  { value: 'solid', label: 'Solid fill' },
]

const TANGENT: { value: TangentEdges; label: string }[] = [
  { value: 'visible', label: 'Tangent edges visible' },
  { value: 'font', label: 'Tangent edges with font' },
  { value: 'removed', label: 'Tangent edges removed' },
]

function title(t: string): string {
  return t.charAt(0).toUpperCase() + t.slice(1)
}

/** Add a view and select it. */
async function addView(
  view: ViewKind,
  origin: P2,
  extra: Record<string, unknown> = {},
): Promise<boolean> {
  const sheet = useDrawing.getState().sheet
  if (sheet === null) return false
  const r = await useStore.getState().run('drawing.view.add', { sheet, view, origin, ...extra })
  if (r) {
    const id = (useStore.getState().drawing as { id?: number } | null)?.id
    if (id) useDrawing.getState().select({ type: 'view', view: id, at: origin })
  }
  return !!r
}

/** Choose a model file: a part or assembly saved on disk. */
function ModelField(props: { value: string | null; onChange: (path: string) => void }) {
  return (
    <div className="field">
      <span>Model</span>
      <div className="button-row">
        <button
          onClick={async () => {
            const path = await window.openforge.openDialog('model')
            if (path) props.onChange(path)
          }}
        >
          {props.value ? 'Change…' : 'Browse…'}
        </button>
      </div>
      {props.value && <small title={props.value}>{props.value.split(/[\\/]/).pop()}</small>}
    </div>
  )
}

/** The model shown on the current sheet, for defaults. */
function lastModel(): string | null {
  const views = useStore.getState().drawing?.drawing.sheets.flatMap((s) => s.views) ?? []
  for (const v of [...views].reverse()) if (v.kind.type === 'model') return v.kind.model.path
  return null
}

/** Front, top and side views per the sheet's projection, plus an isometric. */
export function StandardViewsPanel() {
  const sheet = useSheet()
  const [model, setModel] = useState<string | null>(lastModel())
  const [iso, setIso] = useState(true)
  const [scale, setScale] = useState<Scale | null>(null)
  const ok = async () => {
    if (!model || !sheet) return
    const r = await useStore.getState().run('drawing.views.standard', {
      sheet: sheet.id,
      model,
      isometric: iso,
      ...(scale ? { scale } : {}),
    })
    if (r) await closeDrawingPanel()
  }
  return (
    <PanelShell
      title="Standard 3 View"
      onOk={() => void ok()}
      okDisabled={!model}
      onCancel={() => void closeDrawingPanel()}
    >
      <ModelField value={model} onChange={setModel} />
      <Check label="Add an isometric view" value={iso} onChange={setIso} />
      <ScaleField label="Scale" value={scale} inheritLabel="Fit to sheet" onChange={setScale} />
      <small>
        Views are laid out per the sheet's {sheet?.projection === 'third' ? 'third' : 'first'} angle
        projection. Without a scale, the largest preferred scale that fits becomes the sheet scale.
      </small>
    </PanelShell>
  )
}

/** A model view: file, orientation, scale and display; click to place it. */
export function ModelViewPanel() {
  const [model, setModel] = useState<string | null>(lastModel())
  const [orient, setOrient] = useState<Orientation['type']>('front')
  const [faces, setFaces] = useState<{ entity: CompRef; normal: number[]; area: number }[]>([])
  const [front, setFront] = useState(0)
  const [top, setTop] = useState(1)
  const [scale, setScale] = useState<Scale | null>(null)
  const [display, setDisplay] = useState<DisplayStyle>('hidden_removed')
  const [exploded, setExploded] = useState(false)
  const [flat, setFlat] = useState<FlatView | null>(null)
  const assembly = !!model?.toLowerCase().endsWith('.ofa')

  useEffect(() => {
    if (orient !== 'relative' || !model) return
    void call<{ faces: typeof faces }>('drawing.model.faces', { model }).then(
      (r) => setFaces(r.faces.slice(0, 60)),
      (e: Error) => useStore.getState().toast(e.message),
    )
  }, [orient, model])

  const orientation = (): Orientation | null => {
    if (orient !== 'relative') return { type: orient } as Orientation
    const f = faces[front]
    const t = faces[top]
    if (!f || !t) return null
    return { type: 'relative', front: f.entity, top: t.entity }
  }

  useSheetPicks(
    ['point'],
    model ? 'Click where the view goes' : 'Choose a model first',
    async (p) => {
      const o = orientation()
      if (!model || !o) return
      const ok = await addView(
        {
          type: 'model',
          model: { path: model, ...(flat && !assembly ? { flat } : {}) },
          orientation: o,
          ...(assembly ? { exploded } : {}),
        },
        p.at,
        { ...(scale ? { scale } : {}), display },
      )
      if (ok) await closeDrawingPanel()
    },
    !!model,
  )

  const faceLabel = (f: (typeof faces)[number], i: number) =>
    `${i + 1}. ${entityLabel(f.entity)} (${f.normal.map((n) => n.toFixed(2)).join(', ')})`

  return (
    <PanelShell title="Model View" onCancel={() => void closeDrawingPanel()}>
      <ModelField value={model} onChange={setModel} />
      <Choice
        label="Orientation"
        value={orient}
        options={ORIENTATIONS.map((o) => ({
          value: o,
          label: o === 'relative' ? 'Relative to model (two faces)' : title(o),
        }))}
        onChange={setOrient}
      />
      {orient === 'relative' && (
        <>
          <Choice
            label="Facing the viewer"
            value={front}
            options={faces.map((f, i) => ({ value: i, label: faceLabel(f, i) }))}
            onChange={setFront}
          />
          <Choice
            label="Facing up"
            value={top}
            options={faces.map((f, i) => ({ value: i, label: faceLabel(f, i) }))}
            onChange={setTop}
          />
        </>
      )}
      <ScaleField label="Scale" value={scale} onChange={setScale} />
      <Choice label="Display" value={display} options={DISPLAY} onChange={setDisplay} />
      {assembly && <Check label="Exploded" value={exploded} onChange={setExploded} />}
      {!assembly && model && <FlatOptions value={flat} onChange={setFlat} />}
      <Prompt>
        {model ? 'Click on the sheet to place the view.' : 'Choose a part or assembly.'}
      </Prompt>
    </PanelShell>
  )
}

/** Pick a parent view, then click where the projected view goes. */
export function ProjectedViewPanel() {
  const selected = useDrawing((s) =>
    s.selection.find((p) => p.type === 'view' || p.type === 'entity'),
  )
  const [parent, setParent] = useState<number | null>(
    selected && (selected.type === 'view' || selected.type === 'entity') ? selected.view : null,
  )
  const [hover, setHover] = useState<P2 | null>(null)
  const origin = parent !== null ? viewInfo(parent)?.origin : undefined
  useOverlay({
    lines: origin && hover ? [[origin, hover]] : [],
    circles: [],
    points: [],
  })
  useSheetPicks(
    parent === null ? ['view', 'entity'] : ['point', 'view', 'entity'],
    parent === null ? 'Pick the view to project from' : 'Click where the projected view goes',
    async (p) => {
      if (parent === null) {
        if (p.type === 'view' || p.type === 'entity') setParent(p.view)
        return
      }
      const from = viewInfo(parent)?.origin ?? p.at
      const placement = placementToward(from, p.at) as Extract<
        ViewKind,
        { type: 'projected' }
      >['placement']
      if (await addView({ type: 'projected', parent, placement }, p.at)) await closeDrawingPanel()
    },
    true,
    setHover,
  )
  const sheet = useSheet()
  return (
    <PanelShell title="Projected View" onCancel={() => void closeDrawingPanel()}>
      <Prompt>
        {parent === null
          ? 'Pick the view to project from.'
          : `Projecting from ${sheet?.views.find((v) => v.id === parent)?.name}. Click left, right, above, below or diagonally from it.`}
      </Prompt>
      {parent !== null && <button onClick={() => setParent(null)}>Pick another view</button>}
    </PanelShell>
  )
}

/** Pick a straight edge, then click where the auxiliary view goes. */
export function AuxiliaryViewPanel() {
  const [edge, setEdge] = useState<(SheetPick & { type: 'entity' }) | null>(null)
  const [flip, setFlip] = useState(false)
  useSheetPicks(
    edge ? ['point'] : ['entity'],
    edge ? 'Click where the auxiliary view goes' : 'Pick a straight edge the view looks normal to',
    async (p) => {
      if (!edge) {
        if (p.type !== 'entity') return
        if (p.prim && p.prim.type !== 'line') {
          useStore.getState().toast('Pick a straight edge', 'info')
          return
        }
        setEdge(p)
        return
      }
      if (await addView({ type: 'auxiliary', parent: edge.view, edge: edge.entity, flip }, p.at))
        await closeDrawingPanel()
    },
  )
  return (
    <PanelShell title="Auxiliary View" onCancel={() => void closeDrawingPanel()}>
      <Prompt>
        {edge
          ? `Normal to ${entityLabel(edge.entity)}. Click beside the parent view to place it.`
          : 'Pick a straight edge in a view.'}
      </Prompt>
      <Check label="Flip direction" value={flip} onChange={setFlip} />
      {edge && <button onClick={() => setEdge(null)}>Pick another edge</button>}
    </PanelShell>
  )
}

/** Draw a cutting line on a view, then place the section. */
export function SectionViewPanel({ variant }: { variant?: string }) {
  const [parent, setParent] = useState<number | null>(null)
  const [points, setPoints] = useState<P2[]>([])
  const [hover, setHover] = useState<P2 | null>(null)
  const [placing, setPlacing] = useState(false)
  const [kind, setKind] = useState<SectionKind>(variant === 'aligned' ? 'aligned' : 'full')
  const [flip, setFlip] = useState(false)
  const [partial, setPartial] = useState(false)
  const [depth, setDepth] = useState(10)
  const [slice, setSlice] = useState(false)
  const [fasteners, setFasteners] = useState(true)
  const need = kind === 'aligned' ? 3 : 2
  const sheetPts = parent === null ? [] : points.map((q) => fromViewPlane(parent, q))
  useOverlay({
    lines: [
      ...sheetPts.slice(1).map((q, i) => [sheetPts[i]!, q] as [P2, P2]),
      ...(hover && sheetPts.length && !placing
        ? [[sheetPts[sheetPts.length - 1]!, hover] as [P2, P2]]
        : []),
    ],
    circles: [],
    points: sheetPts,
  })
  useSheetPicks(
    placing ? ['point'] : ['point', 'entity', 'view'],
    placing
      ? 'Click where the section view goes'
      : points.length < need
        ? `Click point ${points.length + 1} of the cutting line on a view`
        : 'Click more points for a jogged cut, or Place View',
    async (p) => {
      if (placing) {
        if (parent === null) return
        const ok = await addView(
          {
            type: 'section',
            parent,
            line: { points },
            kind,
            flip,
            slice,
            exclude_fasteners: fasteners,
            ...(partial ? { depth } : {}),
          },
          p.at,
        )
        if (ok) await closeDrawingPanel()
        return
      }
      const v = parent ?? ('view' in p ? p.view : null)
      if (v === null) {
        useStore.getState().toast('Click on a view to start the cutting line', 'info')
        return
      }
      setParent(v)
      setPoints([...points, toViewPlane(v, p.at)])
      if (kind === 'aligned' && points.length + 1 === 3) setPlacing(true)
    },
    true,
    setHover,
  )
  return (
    <PanelShell
      title={kind === 'aligned' ? 'Aligned Section View' : 'Section View'}
      onCancel={() => void closeDrawingPanel()}
    >
      <Choice
        label="Section"
        value={kind}
        options={[
          { value: 'full', label: 'Full or offset (jogged)' },
          { value: 'half', label: 'Half' },
          { value: 'aligned', label: 'Aligned' },
        ]}
        onChange={(k) => {
          setKind(k)
          setPoints([])
          setPlacing(false)
        }}
      />
      <Prompt>
        {placing
          ? 'Click where the section view goes.'
          : kind === 'half'
            ? 'Half section: the first segment cuts up to the second point.'
            : `${points.length} point(s) picked.`}
      </Prompt>
      <div className="button-row">
        <button disabled={points.length < need || placing} onClick={() => setPlacing(true)}>
          Place View
        </button>
        <button
          disabled={!points.length}
          onClick={() => (setPoints(points.slice(0, -1)), setPlacing(false))}
        >
          Undo Point
        </button>
      </div>
      <Check label="Flip direction" value={flip} onChange={setFlip} />
      <Check label="Partial depth" value={partial} onChange={setPartial} />
      {partial && <LengthField label="Depth" value={depth} onChange={setDepth} min={0.001} />}
      <Check label="Slice (cut faces only)" value={slice} onChange={setSlice} />
      <Check label="Leave hardware uncut" value={fasteners} onChange={setFasteners} />
    </PanelShell>
  )
}

/** Circle an area of a view, then place the magnified detail. */
export function DetailViewPanel() {
  const [parent, setParent] = useState<number | null>(null)
  const [center, setCenter] = useState<P2 | null>(null)
  const [radius, setRadius] = useState<number | null>(null)
  const [hover, setHover] = useState<P2 | null>(null)
  const [style, setStyle] = useState<DetailStyle>('standard')
  const [scale, setScale] = useState<Scale | null>(null)
  const [outline, setOutline] = useState(false)
  const c = parent !== null && center ? fromViewPlane(parent, center) : null
  const r =
    c && radius !== null
      ? radius * (viewInfo(parent!)?.scale ?? 1)
      : c && hover
        ? Math.hypot(hover[0] - c[0], hover[1] - c[1])
        : 0
  useOverlay({
    lines: [],
    circles: c && r > 0 ? [{ center: c, radius: r }] : [],
    points: c ? [c] : [],
  })
  const step = !center ? 0 : radius === null ? 1 : 2
  useSheetPicks(
    step === 0 ? ['point', 'entity', 'view'] : ['point'],
    [
      'Click the center of the detail circle on a view',
      'Click to set the radius',
      'Click where the detail view goes',
    ][step]!,
    async (p) => {
      if (step === 0) {
        const v = 'view' in p ? p.view : null
        if (v === null) return useStore.getState().toast('Click on a view', 'info')
        setParent(v)
        setCenter(toViewPlane(v, p.at))
        return
      }
      if (step === 1) {
        const cc = fromViewPlane(parent!, center!)
        const rr = Math.hypot(p.at[0] - cc[0], p.at[1] - cc[1]) / (viewInfo(parent!)?.scale ?? 1)
        if (rr > 0) setRadius(rr)
        return
      }
      const profile: Profile = { type: 'circle', center: center!, radius: radius! }
      const ok = await addView(
        { type: 'detail', parent: parent!, profile, style, full_outline: outline },
        p.at,
        scale ? { scale } : {},
      )
      if (ok) await closeDrawingPanel()
    },
    true,
    setHover,
  )
  return (
    <PanelShell title="Detail View" onCancel={() => void closeDrawingPanel()}>
      <Prompt>
        {
          [
            'Click the center of the area to magnify.',
            'Click to set the circle size.',
            'Click where the detail goes.',
          ][step]
        }
      </Prompt>
      <Choice
        label="Style"
        value={style}
        options={[
          { value: 'standard', label: 'Per standard' },
          { value: 'broken', label: 'Broken (jagged outline)' },
          { value: 'leader', label: 'With leader' },
          { value: 'no_leader', label: 'No leader' },
          { value: 'connected', label: 'Connected' },
        ]}
        onChange={setStyle}
      />
      <ScaleField label="Scale" value={scale} inheritLabel="Twice the parent" onChange={setScale} />
      <Check label="Full outline" value={outline} onChange={setOutline} />
      {step > 0 && <button onClick={() => (setCenter(null), setRadius(null))}>Start over</button>}
    </PanelShell>
  )
}

/** A closed profile drawn on a view: a circle (two clicks) or a polygon. */
function useProfile(onDone: (view: number, profile: Profile) => void) {
  const [view, setView] = useState<number | null>(null)
  const [shape, setShape] = useState<'circle' | 'polygon'>('circle')
  const [points, setPoints] = useState<P2[]>([])
  const [hover, setHover] = useState<P2 | null>(null)
  const sheetPts = view === null ? [] : points.map((q) => fromViewPlane(view, q))
  const circle =
    shape === 'circle' && sheetPts.length === 1 && hover
      ? [
          {
            center: sheetPts[0]!,
            radius: Math.hypot(hover[0] - sheetPts[0]![0], hover[1] - sheetPts[0]![1]),
          },
        ]
      : []
  useOverlay({
    lines:
      shape === 'polygon'
        ? [
            ...sheetPts.slice(1).map((q, i) => [sheetPts[i]!, q] as [P2, P2]),
            ...(hover && sheetPts.length
              ? [[sheetPts[sheetPts.length - 1]!, hover] as [P2, P2]]
              : []),
          ]
        : [],
    circles: circle,
    points: sheetPts,
  })
  useSheetPicks(
    ['point', 'entity', 'view'],
    shape === 'circle'
      ? points.length === 0
        ? 'Click the center of the circle on a view'
        : 'Click to set the radius'
      : 'Click the corners of the profile; Close Profile when done',
    (p) => {
      const v = view ?? ('view' in p ? p.view : null)
      if (v === null) return useStore.getState().toast('Click on a view', 'info')
      setView(v)
      const q = toViewPlane(v, p.at)
      if (shape === 'circle' && points.length === 1) {
        const r = Math.hypot(q[0] - points[0]![0], q[1] - points[0]![1])
        setPoints([])
        setView(null)
        onDone(v, { type: 'circle', center: points[0]!, radius: r })
        return
      }
      setPoints([...points, q])
    },
    true,
    setHover,
  )
  const close = () => {
    if (view === null || points.length < 3) return
    onDone(view, { type: 'polygon', points })
    setPoints([])
    setView(null)
  }
  const controls = (
    <>
      <Choice
        label="Profile"
        value={shape}
        options={[
          { value: 'circle', label: 'Circle' },
          { value: 'polygon', label: 'Polygon' },
        ]}
        onChange={(s) => (setShape(s), setPoints([]))}
      />
      {shape === 'polygon' && (
        <div className="button-row">
          <button disabled={points.length < 3} onClick={close}>
            Close Profile
          </button>
          <button disabled={!points.length} onClick={() => setPoints(points.slice(0, -1))}>
            Undo Point
          </button>
        </div>
      )}
    </>
  )
  return controls
}

function viewById(id: number): DrawingView | undefined {
  return useStore
    .getState()
    .drawing?.drawing.sheets.flatMap((s) => s.views)
    .find((v) => v.id === id)
}

/** Crop a view to a closed profile. */
export function CropViewPanel() {
  const controls = useProfile(async (view, profile) => {
    const r = await useStore.getState().run('drawing.view.update', { id: view, crop: profile })
    if (r) await closeDrawingPanel()
  })
  return (
    <PanelShell title="Crop View" onCancel={() => void closeDrawingPanel()}>
      {controls}
      <small>Draw the area to keep on a view. Remove a crop in the view's properties.</small>
    </PanelShell>
  )
}

/** Cut away a closed profile of a view to a depth. */
export function BrokenOutPanel() {
  const [depth, setDepth] = useState(5)
  const controls = useProfile(async (view, profile) => {
    const v = viewById(view)
    const r = await useStore.getState().run('drawing.view.update', {
      id: view,
      broken_out: [...(v?.broken_out ?? []), { profile, depth }],
    })
    if (r) await closeDrawingPanel()
  })
  return (
    <PanelShell title="Broken-out Section" onCancel={() => void closeDrawingPanel()}>
      <LengthField label="Depth from the front" value={depth} onChange={setDepth} min={0.001} />
      {controls}
    </PanelShell>
  )
}

/** Remove a band of a view between two break lines. */
export function BreakViewPanel() {
  const [vertical, setVertical] = useState(true)
  const [style, setStyle] = useState<BreakStyle>('zig_zag')
  const [gap, setGap] = useState(10)
  const [view, setView] = useState<number | null>(null)
  const [first, setFirst] = useState<number | null>(null)
  const [hover, setHover] = useState<P2 | null>(null)
  const box = view !== null ? viewInfo(view)?.bbox : null
  const line = (at: P2): [P2, P2] | null =>
    box
      ? vertical
        ? [
            [at[0], box[0][1] - 3],
            [at[0], box[1][1] + 3],
          ]
        : [
            [box[0][0] - 3, at[1]],
            [box[1][0] + 3, at[1]],
          ]
      : null
  const firstSheet =
    view !== null && first !== null ? fromViewPlane(view, vertical ? [first, 0] : [0, first]) : null
  useOverlay({
    lines: [line(firstSheet ?? [0, 0]), hover ? line(hover) : null].filter(
      (l, i): l is [P2, P2] => !!l && (i === 1 || firstSheet !== null),
    ),
    circles: [],
    points: [],
  })
  useSheetPicks(
    ['point', 'entity', 'view'],
    first === null ? 'Click where the break starts on a view' : 'Click where the break ends',
    async (p) => {
      const v = view ?? ('view' in p ? p.view : null)
      if (v === null) return useStore.getState().toast('Click on a view', 'info')
      setView(v)
      const q = toViewPlane(v, p.at)
      const at = vertical ? q[0] : q[1]
      if (first === null) return setFirst(at)
      const b: Break = { vertical, from: Math.min(first, at), to: Math.max(first, at) }
      const existing = viewById(v)?.breaks ?? []
      const r = await useStore.getState().run('drawing.view.update', {
        id: v,
        breaks: [...existing, b],
        break_style: style,
        break_gap: gap,
      })
      if (r) await closeDrawingPanel()
    },
    true,
    setHover,
  )
  return (
    <PanelShell title="Break" onCancel={() => void closeDrawingPanel()}>
      <Choice
        label="Break lines"
        value={vertical ? 'v' : 'h'}
        options={[
          { value: 'v', label: 'Vertical (shortens horizontally)' },
          { value: 'h', label: 'Horizontal (shortens vertically)' },
        ]}
        onChange={(v) => (setVertical(v === 'v'), setFirst(null))}
      />
      <Choice
        label="Style"
        value={style}
        options={[
          { value: 'straight', label: 'Straight' },
          { value: 'curved', label: 'Curved' },
          { value: 'zig_zag', label: 'Zig zag' },
          { value: 'small_zig_zag', label: 'Small zig zag' },
          { value: 'jagged', label: 'Jagged' },
        ]}
        onChange={setStyle}
      />
      <NumberField label="Gap (mm)" value={gap} min={0} onChange={setGap} />
      <small>Dimensions across a break keep their true length.</small>
    </PanelShell>
  )
}

/** Everything about one view. */
export function ViewPropertiesPanel({ editing }: { editing?: number }) {
  const drawing = useStore((s) => s.drawing)!
  const view = drawing.drawing.sheets.flatMap((s) => s.views).find((v) => v.id === editing)
  const [name, setName] = useState(view?.name ?? '')
  const [scale, setScale] = useState<Scale | null>(view?.scale ?? null)
  const [display, setDisplay] = useState<DisplayStyle | 'inherit'>(view?.display ?? 'inherit')
  const [tangent, setTangent] = useState<TangentEdges | 'inherit'>(view?.tangent ?? 'inherit')
  const [label, setLabel] = useState(view?.label ?? '')
  const [aligned, setAligned] = useState(view?.aligned ?? true)
  const [orient, setOrient] = useState<Orientation['type'] | null>(
    view?.kind.type === 'model' ? view.kind.orientation.type : null,
  )
  const [exploded, setExploded] = useState(
    view?.kind.type === 'model' ? !!view.kind.exploded : false,
  )
  const [flat, setFlat] = useState<FlatView | null>(
    view?.kind.type === 'model' ? (view.kind.model.flat ?? null) : null,
  )
  const [hatchScale, setHatchScale] = useState(view?.hatch?.scale ?? 1)
  const [hatchAngle, setHatchAngle] = useState(((view?.hatch?.angle ?? 0) * 180) / Math.PI)
  const [hatchPattern, setHatchPattern] = useState<string>(view?.hatch?.pattern ?? 'material')
  const [hidingEdges, setHidingEdges] = useState(false)
  const [hidden, setHidden] = useState<CompRef[]>(view?.hidden_edges ?? [])
  useSheetPicks(
    ['entity'],
    'Click edges to hide or show them',
    (p) => {
      if (p.type !== 'entity' || p.view !== editing) return
      const key = JSON.stringify(p.entity)
      setHidden((h) =>
        h.some((x) => JSON.stringify(x) === key)
          ? h.filter((x) => JSON.stringify(x) !== key)
          : [...h, p.entity],
      )
    },
    hidingEdges,
  )
  if (!view) return null
  const isModel = view.kind.type === 'model'
  const isAssembly =
    isModel && view.kind.type === 'model' && view.kind.model.path.toLowerCase().endsWith('.ofa')
  const sectioned = view.kind.type === 'section' || (view.broken_out?.length ?? 0) > 0
  const ok = async () => {
    const params: Record<string, unknown> = {
      id: view.id,
      name,
      scale,
      display: display === 'inherit' ? null : display,
      tangent: tangent === 'inherit' ? null : tangent,
      label: label.trim() ? label.trim() : null,
      aligned,
      hidden_edges: hidden,
    }
    if (isModel && view.kind.type === 'model' && orient && orient !== 'relative')
      params['view'] = {
        ...view.kind,
        model: { ...view.kind.model, flat },
        orientation:
          orient === view.kind.orientation.type ? view.kind.orientation : { type: orient },
        exploded,
      }
    if (sectioned)
      params['hatch'] = {
        pattern: hatchPattern === 'material' ? null : hatchPattern,
        scale: hatchScale,
        angle: (hatchAngle * Math.PI) / 180,
      }
    const r = await useStore.getState().run('drawing.view.update', params)
    if (r) await closeDrawingPanel()
  }
  const clear = (field: string, value: unknown) =>
    void useStore.getState().run('drawing.view.update', { id: view.id, [field]: value })
  return (
    <PanelShell title={view.name} onOk={() => void ok()} onCancel={() => void closeDrawingPanel()}>
      <TextField label="Name" value={name} onChange={setName} />
      {view.kind.type === 'model' && (
        <small title={view.kind.model.path}>
          Model: {view.kind.model.path.split(/[\\/]/).pop()}
        </small>
      )}
      {isModel && orient && orient !== 'relative' && (
        <Choice
          label="Orientation"
          value={orient}
          options={ORIENTATIONS.filter((o) => o !== 'relative').map((o) => ({
            value: o,
            label: title(o),
          }))}
          onChange={setOrient}
        />
      )}
      {isAssembly && <Check label="Exploded" value={exploded} onChange={setExploded} />}
      {isModel && !isAssembly && <FlatOptions value={flat} onChange={setFlat} />}
      <ScaleField
        label="Scale"
        value={scale}
        inheritLabel={isModel ? 'Sheet scale' : "Parent's scale"}
        onChange={setScale}
      />
      <Choice
        label="Display"
        value={display}
        options={[
          { value: 'inherit' as const, label: isModel ? 'Default' : 'As parent' },
          ...DISPLAY,
        ]}
        onChange={setDisplay}
      />
      <Choice
        label="Tangent edges"
        value={tangent}
        options={[
          { value: 'inherit' as const, label: isModel ? 'Default' : 'As parent' },
          ...TANGENT,
        ]}
        onChange={setTangent}
      />
      {!isModel && <Check label="Aligned with parent" value={aligned} onChange={setAligned} />}
      {(view.kind.type === 'section' ||
        view.kind.type === 'detail' ||
        view.kind.type === 'auxiliary') && (
        <TextField label="Label" value={label} onChange={setLabel} />
      )}
      <Section title="Edges" open={hidingEdges || hidden.length > 0}>
        <Check label="Pick edges to hide or show" value={hidingEdges} onChange={setHidingEdges} />
        <small>{hidden.length} edge(s) hidden.</small>
        {hidden.length > 0 && <button onClick={() => setHidden([])}>Show all edges</button>}
      </Section>
      {sectioned && (
        <Section title="Hatching">
          <Choice
            label="Pattern"
            value={hatchPattern}
            options={[{ value: 'material', label: 'By material' }, ...HATCHES]}
            onChange={setHatchPattern}
          />
          <NumberField label="Scale" value={hatchScale} min={0.01} onChange={setHatchScale} />
          <NumberField label="Angle (°)" value={hatchAngle} onChange={setHatchAngle} />
        </Section>
      )}
      {((view.breaks?.length ?? 0) > 0 ||
        view.crop ||
        (view.broken_out?.length ?? 0) > 0 ||
        (view.hidden_components?.length ?? 0) > 0) && (
        <Section title="Modifications">
          {(view.breaks?.length ?? 0) > 0 && (
            <button onClick={() => clear('breaks', [])}>
              Remove {view.breaks!.length} break(s)
            </button>
          )}
          {view.crop && <button onClick={() => clear('crop', null)}>Remove crop</button>}
          {(view.broken_out?.length ?? 0) > 0 && (
            <button onClick={() => clear('broken_out', [])}>Remove broken-out sections</button>
          )}
          {(view.hidden_components?.length ?? 0) > 0 && (
            <button onClick={() => clear('hidden_components', [])}>Show hidden components</button>
          )}
        </Section>
      )}
      {isAssembly && <HideComponents view={view} />}
      <small>
        Scale {view.scale ? scaleLabel(view.scale) : 'inherited'} · drag the view on the sheet to
        move it.
      </small>
    </PanelShell>
  )
}

/** Hide assembly components picked in the view. */
function HideComponents({ view }: { view: DrawingView }) {
  const [on, setOn] = useState(false)
  useSheetPicks(
    ['entity'],
    'Click a component to hide it in this view',
    (p) => {
      if (p.type !== 'entity' || p.view !== view.id || !p.entity.component?.length) return
      const key = p.entity.component.slice(0, 1)
      void useStore.getState().run('drawing.view.update', {
        id: view.id,
        hidden_components: [...(view.hidden_components ?? []), key],
      })
    },
    on,
  )
  return <Check label="Pick components to hide" value={on} onChange={setOn} />
}

/** A sheet metal part's flat pattern instead of the formed part. */
function FlatOptions(props: { value: FlatView | null; onChange: (f: FlatView | null) => void }) {
  const f = props.value
  return (
    <>
      <Check
        label="Flat pattern (sheet metal)"
        value={!!f}
        onChange={(on) => props.onChange(on ? { bend_lines: true, bend_notes: true } : null)}
      />
      {f && (
        <>
          <Check
            label="Bend lines"
            value={f.bend_lines ?? true}
            onChange={(bend_lines) => props.onChange({ ...f, bend_lines })}
          />
          <Check
            label="Bend notes"
            value={f.bend_notes ?? true}
            onChange={(bend_notes) => props.onChange({ ...f, bend_notes })}
          />
        </>
      )}
    </>
  )
}

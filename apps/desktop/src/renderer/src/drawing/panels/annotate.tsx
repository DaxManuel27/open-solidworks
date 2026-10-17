import { useState } from 'react'
import type {
  Anchor,
  AnnotationKind,
  BalloonLayout,
  BalloonShape,
  BomType,
  Border,
  Characteristic,
  CloudShape,
  Column,
  Corner,
  FinishProcess,
  GtolRow,
  MarkPattern,
  Modifier,
  Numbering,
  RevisionRow,
  Tags,
  TargetKind,
  ToleranceClass,
  ViewRef,
  WeldSide,
  WeldSymbol,
} from '../../../../shared/tools.gen'
import { call } from '../../core'
import { Check, Choice, IntField, NumberField, PanelShell, Section } from '../../panels/fields'
import { useStore } from '../../store'
import { type SheetPick, useDrawing } from '../state'
import type { Annotation, P2 } from '../types'
import { HATCHES } from './views'
import {
  Prompt,
  TextField,
  addAnnotation,
  allViews,
  closeDrawingPanel,
  entityLabel,
  modelAnchor,
  relativeTo,
  toViewPlane,
  useOverlay,
  useSheet,
  useSheetPicks,
  viewAt,
  viewRef,
} from './common'
import { DimensionOptions, type DimOptions } from './dimensions'

type EntityPick = SheetPick & { type: 'entity' }

/**
 * Placement with an optional leader: pick the model edge the leader points
 * to (when wanted), then click where the annotation goes. The annotation
 * moves with the leader's view, or the view it is placed in.
 */
function usePlacement(props: {
  leader: 'none' | 'optional' | 'required'
  leaderOn: boolean
  what: string
  place: (at: P2, view: number | null, leader: EntityPick | null) => Promise<boolean>
  disabled?: boolean
}) {
  const [leader, setLeader] = useState<EntityPick | null>(null)
  const wantLeader = props.leader === 'required' || (props.leader === 'optional' && props.leaderOn)
  const step = wantLeader && !leader ? 'leader' : 'place'
  useSheetPicks(
    step === 'leader' ? ['entity'] : ['point'],
    step === 'leader'
      ? `Pick the model edge the ${props.what} points to`
      : `Click where the ${props.what} goes`,
    async (p) => {
      if (step === 'leader') {
        if (p.type === 'entity') setLeader(p)
        return
      }
      const view = leader?.view ?? viewAt(p.at)
      const ok = await props.place(relativeTo(view, p.at), view, wantLeader ? leader : null)
      if (ok) setLeader(null)
    },
    !props.disabled,
  )
  const prompt = (
    <Prompt>
      {step === 'leader'
        ? `Pick the edge the ${props.what} points to.`
        : leader
          ? `Leader to ${entityLabel(leader.entity)}. Click where the ${props.what} goes.`
          : `Click where the ${props.what} goes.`}
    </Prompt>
  )
  return { leader, setLeader, prompt }
}

const SYMBOLS = ['⌀', '⌴', '⌵', '↧', '±', '°', '□']
const LINKS = [
  '$PRP:"PartNo"',
  '$PRP:"Description"',
  '$PRPSHEET:"DrawnBy"',
  '$SHEET',
  '$SCALE',
  '$DATE',
]

/** Notes: text with symbols and property links, border and leader. */
export function NotePanel() {
  const [text, setText] = useState('')
  const [height, setHeight] = useState<number | null>(null)
  const [border, setBorder] = useState<Border>('none')
  const [withLeader, setWithLeader] = useState(false)
  const [jog, setJog] = useState(false)
  const { prompt } = usePlacement({
    leader: 'optional',
    leaderOn: withLeader,
    what: 'note',
    disabled: !text.trim(),
    place: (at, view, leader) =>
      addAnnotation(
        {
          type: 'note',
          at,
          text,
          border,
          jog,
          ...(height ? { height } : {}),
          ...(leader ? { leader: modelAnchor(leader) } : {}),
        },
        view,
      ),
  })
  return (
    <PanelShell
      title="Note"
      onCancel={() => void closeDrawingPanel()}
      onOk={() => void closeDrawingPanel()}
    >
      <TextField
        label="Text"
        multiline
        value={text}
        onChange={setText}
        placeholder="Type the note"
      />
      <div className="button-row symbols">
        {SYMBOLS.map((s) => (
          <button key={s} title={`Insert ${s}`} onClick={() => setText(text + s)}>
            {s}
          </button>
        ))}
      </div>
      <Choice
        label="Insert field"
        value=""
        options={[
          { value: '', label: 'Property link…' },
          ...LINKS.map((l) => ({ value: l, label: l })),
        ]}
        onChange={(l) => l && setText(text + l)}
      />
      <Choice
        label="Border"
        value={border}
        options={[
          { value: 'none', label: 'None' },
          { value: 'box', label: 'Box' },
          { value: 'circle', label: 'Circle' },
        ]}
        onChange={setBorder}
      />
      <NumberField
        label="Text height (mm, 0 = standard)"
        value={height ?? 0}
        min={0}
        onChange={(h) => setHeight(h > 0 ? h : null)}
      />
      <Check label="Leader" value={withLeader} onChange={setWithLeader} />
      {withLeader && <Check label="Jogged leader" value={jog} onChange={setJog} />}
      {text.trim() ? prompt : <Prompt>Type the note text.</Prompt>}
    </PanelShell>
  )
}

const SHAPES: { value: BalloonShape; label: string }[] = [
  { value: 'circle', label: 'Circle' },
  { value: 'split_circle', label: 'Split circle (item over quantity)' },
  { value: 'triangle', label: 'Triangle' },
  { value: 'hexagon', label: 'Hexagon' },
  { value: 'box', label: 'Box' },
  { value: 'diamond', label: 'Diamond' },
  { value: 'flag', label: 'Flag' },
  { value: 'none', label: 'No border' },
]

/** A balloon on a component (BOM item number) or with free text. */
export function BalloonPanel() {
  const [shape, setShape] = useState<BalloonShape>('circle')
  const [content, setContent] = useState<'item' | 'quantity' | 'text' | 'property'>('item')
  const [text, setText] = useState('')
  const [property, setProperty] = useState('PartNo')
  const { prompt } = usePlacement({
    leader: 'required',
    leaderOn: true,
    what: 'balloon',
    place: async (at, view, leader) => {
      if (!leader) return false
      const component = leader.entity.component?.slice(0, 1) ?? []
      if (content !== 'text' && !component.length) {
        useStore.getState().toast('Pick an edge of an assembly component, or use text', 'info')
        return false
      }
      const c =
        content === 'text'
          ? { type: 'text' as const, text }
          : content === 'property'
            ? { type: 'property' as const, component, name: property }
            : { type: content, component }
      return addAnnotation(
        { type: 'balloon', at, leader: modelAnchor(leader), content: c, shape },
        view,
      )
    },
  })
  return (
    <PanelShell
      title="Balloon"
      onCancel={() => void closeDrawingPanel()}
      onOk={() => void closeDrawingPanel()}
    >
      <Choice
        label="Shows"
        value={content}
        options={[
          { value: 'item', label: 'BOM item number' },
          { value: 'quantity', label: 'Quantity' },
          { value: 'property', label: 'Part property' },
          { value: 'text', label: 'Text' },
        ]}
        onChange={setContent}
      />
      {content === 'text' && <TextField label="Text" value={text} onChange={setText} />}
      {content === 'property' && (
        <TextField label="Property" value={property} onChange={setProperty} />
      )}
      <Choice label="Shape" value={shape} options={SHAPES} onChange={setShape} />
      {prompt}
    </PanelShell>
  )
}

/** Balloon every component of an assembly view. */
export function AutoBalloonPanel() {
  const selected = useDrawing((s) =>
    s.selection.find((p) => p.type === 'view' || p.type === 'entity'),
  )
  const sheet = useSheet()
  const [view, setView] = useState<number | null>(
    selected && 'view' in selected ? selected.view : null,
  )
  const [layout, setLayout] = useState<BalloonLayout>('square')
  const [ignore, setIgnore] = useState(true)
  useSheetPicks(
    ['view', 'entity'],
    'Pick an assembly view',
    (p) => 'view' in p && setView(p.view),
    view === null,
  )
  const ok = async () => {
    if (view === null || !sheet) return
    const r = await useStore.getState().run('drawing.balloons.auto', {
      sheet: sheet.id,
      view,
      layout,
      ignore_multiple: ignore,
    })
    if (r) await closeDrawingPanel()
  }
  return (
    <PanelShell
      title="Auto Balloon"
      onOk={() => void ok()}
      okDisabled={view === null}
      onCancel={() => void closeDrawingPanel()}
    >
      <Prompt>
        {view === null
          ? 'Pick an assembly view.'
          : `View: ${sheet?.views.find((v) => v.id === view)?.name}`}
      </Prompt>
      <Choice
        label="Layout"
        value={layout}
        options={[
          { value: 'square', label: 'Around the view' },
          { value: 'circular', label: 'Circular' },
          { value: 'top', label: 'Above' },
          { value: 'bottom', label: 'Below' },
          { value: 'left', label: 'Left' },
          { value: 'right', label: 'Right' },
        ]}
        onChange={setLayout}
      />
      <Check label="One balloon per BOM item" value={ignore} onChange={setIgnore} />
      <small>Balloons show the item numbers of the sheet's BOM table and follow it.</small>
    </PanelShell>
  )
}

const CHARACTERISTICS: { value: Characteristic; label: string }[] = [
  { value: 'straightness', label: '⏤ Straightness' },
  { value: 'flatness', label: '⏥ Flatness' },
  { value: 'circularity', label: '○ Circularity' },
  { value: 'cylindricity', label: '⌭ Cylindricity' },
  { value: 'line_profile', label: '⌒ Profile of a line' },
  { value: 'surface_profile', label: '⌓ Profile of a surface' },
  { value: 'angularity', label: '∠ Angularity' },
  { value: 'perpendicularity', label: '⟂ Perpendicularity' },
  { value: 'parallelism', label: '∥ Parallelism' },
  { value: 'position', label: '⌖ Position' },
  { value: 'concentricity', label: '◎ Concentricity' },
  { value: 'symmetry', label: '⌯ Symmetry' },
  { value: 'circular_runout', label: '↗ Circular runout' },
  { value: 'total_runout', label: '⌰ Total runout' },
]

const MODIFIERS: { value: Modifier | ''; label: string }[] = [
  { value: '', label: 'None' },
  { value: 'mmc', label: 'Ⓜ MMC' },
  { value: 'lmc', label: 'Ⓛ LMC' },
  { value: 'projected', label: 'Ⓟ Projected' },
  { value: 'free_state', label: 'Ⓕ Free state' },
  { value: 'tangent_plane', label: 'Ⓣ Tangent plane' },
  { value: 'rfs', label: 'Ⓢ RFS' },
  { value: 'unequal', label: 'Ⓤ Unequally disposed' },
]

function GtolRowEditor(props: { row: GtolRow; onChange: (r: GtolRow) => void }) {
  const r = props.row
  const datums = r.datums ?? []
  const datum = (i: number) => datums[i]?.label ?? ''
  const setDatum = (i: number, label: string) => {
    const next = [0, 1, 2].map((k) => (k === i ? label : datum(k)))
    while (next.length && !next[next.length - 1]) next.pop()
    props.onChange({
      ...r,
      datums: next.map((l, k) => ({ label: l, modifier: datums[k]?.modifier ?? null })),
    })
  }
  return (
    <>
      <Choice
        label="Characteristic"
        value={r.characteristic}
        options={CHARACTERISTICS}
        onChange={(characteristic) => props.onChange({ ...r, characteristic })}
      />
      <Check
        label="⌀ zone"
        value={!!r.diameter}
        onChange={(diameter) => props.onChange({ ...r, diameter })}
      />
      <TextField
        label="Tolerance"
        value={r.tolerance}
        onChange={(tolerance) => props.onChange({ ...r, tolerance })}
      />
      <Choice
        label="Modifier"
        value={r.modifiers?.[0] ?? ''}
        options={MODIFIERS}
        onChange={(m) => props.onChange({ ...r, modifiers: m ? [m] : [] })}
      />
      <div className="button-row">
        {['Primary', 'Secondary', 'Tertiary'].map((label, i) => (
          <label key={label} className="field compact">
            <span>{label}</span>
            <input
              value={datum(i)}
              maxLength={3}
              onChange={(e) => setDatum(i, e.target.value.toUpperCase())}
              onKeyDown={(e) => e.stopPropagation()}
            />
          </label>
        ))}
      </div>
    </>
  )
}

const WELDS: { value: WeldSymbol | ''; label: string }[] = [
  { value: '', label: 'None' },
  { value: 'fillet', label: 'Fillet' },
  { value: 'square', label: 'Square groove' },
  { value: 'v', label: 'V groove' },
  { value: 'bevel', label: 'Bevel groove' },
  { value: 'u', label: 'U groove' },
  { value: 'j', label: 'J groove' },
  { value: 'flare_v', label: 'Flare V' },
  { value: 'flare_bevel', label: 'Flare bevel' },
  { value: 'plug', label: 'Plug or slot' },
  { value: 'spot', label: 'Spot' },
  { value: 'seam', label: 'Seam' },
  { value: 'backing', label: 'Backing' },
  { value: 'surfacing', label: 'Surfacing' },
  { value: 'edge', label: 'Edge' },
]

function WeldSideEditor(props: {
  label: string
  side: WeldSide | null
  onChange: (s: WeldSide | null) => void
}) {
  const s = props.side
  return (
    <Section title={props.label}>
      <Choice
        label="Symbol"
        value={s?.symbol ?? ''}
        options={WELDS}
        onChange={(symbol) => props.onChange(symbol ? { ...(s ?? {}), symbol } : null)}
      />
      {s && (
        <>
          <TextField
            label="Size"
            value={s.size ?? ''}
            onChange={(size) => props.onChange({ ...s, size })}
          />
          <TextField
            label="Length-pitch"
            value={s.length ?? ''}
            onChange={(length) => props.onChange({ ...s, length })}
          />
          <Choice
            label="Contour"
            value={s.contour ?? 'none'}
            options={[
              { value: 'none', label: 'None' },
              { value: 'flat', label: 'Flat' },
              { value: 'convex', label: 'Convex' },
              { value: 'concave', label: 'Concave' },
            ]}
            onChange={(contour) => props.onChange({ ...s, contour })}
          />
        </>
      )}
    </Section>
  )
}

/** Surface finish, weld, geometric tolerance, datum feature and datum target symbols. */
export function SymbolPanel({ variant = 'surface_finish' }: { variant?: string }) {
  const [process, setProcess] = useState<FinishProcess>('removal')
  const [roughness, setRoughness] = useState('Ra 3.2')
  const [method, setMethod] = useState('')
  const [lay, setLay] = useState('')
  const [allAround, setAllAround] = useState(false)
  const [rows, setRows] = useState<GtolRow[]>([
    {
      characteristic: 'position',
      tolerance: '0.1',
      diameter: true,
      datums: [{ label: 'A' }, { label: 'B' }],
    },
  ])
  const [above, setAbove] = useState('')
  const [label, setLabel] = useState('A')
  const [size, setSize] = useState('')
  const [targetKind, setTargetKind] = useState<TargetKind>('point')
  const [arrow, setArrow] = useState<WeldSide | null>({ symbol: 'fillet', size: '5' })
  const [other, setOther] = useState<WeldSide | null>(null)
  const [field, setField] = useState(false)
  const [tail, setTail] = useState('')
  const [withLeader, setWithLeader] = useState(variant !== 'surface_finish')
  const needs: 'none' | 'optional' | 'required' =
    variant === 'weld' || variant === 'datum_feature' || variant === 'datum_target'
      ? 'required'
      : 'optional'
  const titles: Record<string, string> = {
    surface_finish: 'Surface Finish',
    weld: 'Weld Symbol',
    gtol: 'Geometric Tolerance',
    datum_feature: 'Datum Feature',
    datum_target: 'Datum Target',
  }
  const { prompt } = usePlacement({
    leader: needs,
    leaderOn: withLeader,
    what: 'symbol',
    place: async (at, view, leader) => {
      const anchor: Anchor | null = leader ? modelAnchor(leader) : null
      let kind: AnnotationKind
      switch (variant) {
        case 'weld':
          kind = {
            type: 'weld',
            at,
            leader: anchor!,
            arrow,
            other,
            all_around: allAround,
            field,
            tail,
          }
          break
        case 'gtol':
          kind = { type: 'gtol', at, rows, above, ...(anchor ? { leader: anchor } : {}) }
          break
        case 'datum_feature':
          kind = { type: 'datum_feature', at, label, target: anchor! }
          break
        case 'datum_target':
          kind = { type: 'datum_target', at, label, size, target: anchor!, kind: targetKind }
          break
        default:
          kind = {
            type: 'surface_finish',
            at,
            process,
            roughness,
            method,
            lay,
            all_around: allAround,
            ...(anchor ? { leader: anchor } : {}),
          }
      }
      return addAnnotation(kind, view)
    },
  })
  return (
    <PanelShell
      title={titles[variant] ?? 'Symbol'}
      onCancel={() => void closeDrawingPanel()}
      onOk={() => void closeDrawingPanel()}
    >
      {variant === 'surface_finish' && (
        <>
          <Choice
            label="Process"
            value={process}
            options={[
              { value: 'any', label: 'Any process' },
              { value: 'removal', label: 'Material removal required' },
              { value: 'no_removal', label: 'Material removal prohibited' },
            ]}
            onChange={setProcess}
          />
          <TextField label="Roughness" value={roughness} onChange={setRoughness} />
          <TextField label="Production method" value={method} onChange={setMethod} />
          <Choice
            label="Lay"
            value={lay}
            options={['', '=', '⊥', 'X', 'M', 'C', 'R', 'P'].map((l) => ({
              value: l,
              label: l || 'None',
            }))}
            onChange={setLay}
          />
          <Check label="All around" value={allAround} onChange={setAllAround} />
        </>
      )}
      {variant === 'gtol' && (
        <>
          {rows.map((r, i) => (
            <Section key={i} title={i === 0 ? 'Frame' : 'Composite row'}>
              <GtolRowEditor
                row={r}
                onChange={(nr) => setRows(rows.map((x, j) => (j === i ? nr : x)))}
              />
            </Section>
          ))}
          <div className="button-row">
            {rows.length < 2 && (
              <button
                onClick={() =>
                  setRows([
                    ...rows,
                    { ...rows[0]!, tolerance: '0.05', datums: rows[0]!.datums?.slice(0, 1) },
                  ])
                }
              >
                Add composite row
              </button>
            )}
            {rows.length > 1 && (
              <button onClick={() => setRows(rows.slice(0, 1))}>Remove composite row</button>
            )}
          </div>
          <TextField
            label="Above the frame"
            value={above}
            onChange={setAbove}
            placeholder="e.g. 4X"
          />
        </>
      )}
      {(variant === 'datum_feature' || variant === 'datum_target') && (
        <TextField label="Label" value={label} onChange={(l) => setLabel(l.toUpperCase())} />
      )}
      {variant === 'datum_target' && (
        <>
          <TextField label="Size" value={size} onChange={setSize} placeholder="e.g. ⌀10" />
          <Choice
            label="Target"
            value={targetKind}
            options={[
              { value: 'point', label: 'Point' },
              { value: 'line', label: 'Line' },
              { value: 'area', label: 'Area' },
            ]}
            onChange={setTargetKind}
          />
        </>
      )}
      {variant === 'weld' && (
        <>
          <WeldSideEditor label="Arrow side" side={arrow} onChange={setArrow} />
          <WeldSideEditor label="Other side" side={other} onChange={setOther} />
          <Check label="All around" value={allAround} onChange={setAllAround} />
          <Check label="Field weld" value={field} onChange={setField} />
          <TextField label="Tail (process)" value={tail} onChange={setTail} />
        </>
      )}
      {needs === 'optional' && <Check label="Leader" value={withLeader} onChange={setWithLeader} />}
      {prompt}
    </PanelShell>
  )
}

/** A hole callout from Hole Wizard data (or the hole's size). */
export function HoleCalloutPanel() {
  const [text, setText] = useState('')
  const { prompt } = usePlacement({
    leader: 'required',
    leaderOn: true,
    what: 'callout',
    place: async (at, view, leader) => {
      if (!leader) return false
      if (leader.prim?.type !== 'arc') {
        useStore.getState().toast('Pick the circular edge of a hole', 'info')
        return false
      }
      return addAnnotation(
        { type: 'hole_callout', at, target: viewRef(leader), ...(text.trim() ? { text } : {}) },
        view,
      )
    },
  })
  return (
    <PanelShell
      title="Hole Callout"
      onCancel={() => void closeDrawingPanel()}
      onOk={() => void closeDrawingPanel()}
    >
      {prompt}
      <TextField
        label="Override text"
        value={text}
        onChange={setText}
        placeholder="From the hole"
      />
    </PanelShell>
  )
}

/** Center marks on circles: single, or a linear or circular pattern. */
export function CenterMarkPanel() {
  const [targets, setTargets] = useState<EntityPick[]>([])
  const [pattern, setPattern] = useState<MarkPattern>('single')
  const [extended, setExtended] = useState(true)
  useSheetPicks(['entity'], 'Pick circular edges', (p) => {
    if (p.type !== 'entity') return
    if (p.prim?.type !== 'arc') return useStore.getState().toast('Pick a circle or arc', 'info')
    if (targets.length && targets[0]!.view !== p.view)
      return useStore.getState().toast('Pick circles in one view', 'info')
    setTargets([...targets, p])
  })
  const ok = async () => {
    if (!targets.length) return
    const ok = await addAnnotation(
      { type: 'center_mark', targets: targets.map(viewRef), pattern, extended },
      targets[0]!.view,
    )
    if (ok) await closeDrawingPanel()
  }
  return (
    <PanelShell
      title="Center Mark"
      onOk={() => void ok()}
      okDisabled={!targets.length}
      onCancel={() => void closeDrawingPanel()}
    >
      <Prompt>
        {targets.length ? `${targets.length} circle(s) picked.` : 'Pick circles or arcs.'}
      </Prompt>
      <Choice
        label="Marks"
        value={pattern}
        options={[
          { value: 'single', label: 'One per circle' },
          { value: 'linear', label: 'Linear pattern' },
          { value: 'circular', label: 'Circular pattern' },
        ]}
        onChange={setPattern}
      />
      <Check label="Extended lines" value={extended} onChange={setExtended} />
    </PanelShell>
  )
}

/** A centerline between two straight edges. */
export function CenterlinePanel() {
  const [first, setFirst] = useState<EntityPick | null>(null)
  useSheetPicks(
    ['entity'],
    first ? 'Pick the second edge' : 'Pick the first of two edges',
    async (p) => {
      if (p.type !== 'entity') return
      if (p.prim?.type !== 'line') return useStore.getState().toast('Pick straight edges', 'info')
      if (!first) return setFirst(p)
      if (first.view !== p.view) return useStore.getState().toast('Pick edges in one view', 'info')
      const ok = await addAnnotation(
        { type: 'centerline', a: viewRef(first), b: viewRef(p) },
        p.view,
      )
      if (ok) setFirst(null)
    },
  )
  return (
    <PanelShell
      title="Centerline"
      onCancel={() => void closeDrawingPanel()}
      onOk={() => void closeDrawingPanel()}
    >
      <Prompt>
        {first
          ? `First edge: ${entityLabel(first.entity)}. Pick the second.`
          : 'Pick two straight edges.'}
      </Prompt>
      <small>Model Items can insert centerlines and center marks for every cylinder.</small>
    </PanelShell>
  )
}

/** Click a polygon on the sheet; returns the overlay controls. */
function usePolygon(prompt: string, onClose: (points: P2[]) => void, enabled = true) {
  const [points, setPoints] = useState<P2[]>([])
  const [hover, setHover] = useState<P2 | null>(null)
  useOverlay({
    lines: [
      ...points.slice(1).map((q, i) => [points[i]!, q] as [P2, P2]),
      ...(hover && points.length ? [[points[points.length - 1]!, hover] as [P2, P2]] : []),
    ],
    circles: [],
    points,
  })
  useSheetPicks(['point'], prompt, (p) => setPoints([...points, p.at]), enabled, setHover)
  const controls = (
    <div className="button-row">
      <button
        disabled={points.length < 3}
        onClick={() => {
          onClose(points)
          setPoints([])
        }}
      >
        Close
      </button>
      <button disabled={!points.length} onClick={() => setPoints(points.slice(0, -1))}>
        Undo Point
      </button>
    </div>
  )
  return { points, controls }
}

/** Hatch or fill a closed region. */
export function AreaHatchPanel() {
  const [pattern, setPattern] = useState('general')
  const [scale, setScale] = useState(1)
  const [angle, setAngle] = useState(0)
  const { controls } = usePolygon(
    'Click the corners of the region; Close when done',
    async (points) => {
      const view = viewAt(points[0]!)
      await addAnnotation(
        {
          type: 'area_hatch',
          boundary: points.map((q) => relativeTo(view, q)),
          pattern: pattern as never,
          scale,
          angle: (angle * Math.PI) / 180,
        },
        view,
      )
    },
  )
  return (
    <PanelShell
      title="Area Hatch/Fill"
      onCancel={() => void closeDrawingPanel()}
      onOk={() => void closeDrawingPanel()}
    >
      <Choice label="Pattern" value={pattern} options={HATCHES} onChange={setPattern} />
      <NumberField label="Scale" value={scale} min={0.01} onChange={setScale} />
      <NumberField label="Angle (°)" value={angle} onChange={setAngle} />
      {controls}
    </PanelShell>
  )
}

/** Revision clouds and revision symbols. */
export function RevisionCloudPanel({ variant }: { variant?: string }) {
  const symbol = variant === 'symbol'
  const [shape, setShape] = useState<'rectangle' | 'ellipse' | 'polygon'>('rectangle')
  const [revision, setRevision] = useState('A')
  const [arc, setArc] = useState(6)
  const [corner, setCorner] = useState<P2 | null>(null)
  const [hover, setHover] = useState<P2 | null>(null)
  const drag = !symbol && shape !== 'polygon'
  useOverlay({
    lines:
      drag && corner && hover
        ? shape === 'rectangle'
          ? [
              [corner, [hover[0], corner[1]]],
              [[hover[0], corner[1]], hover],
              [hover, [corner[0], hover[1]]],
              [[corner[0], hover[1]], corner],
            ]
          : []
        : [],
    circles:
      drag && corner && hover && shape === 'ellipse'
        ? [{ center: corner, radius: Math.hypot(hover[0] - corner[0], hover[1] - corner[1]) }]
        : [],
    points: corner ? [corner] : [],
  })
  const add = (cloud: CloudShape) =>
    addAnnotation({ type: 'revision_cloud', shape: cloud, arc, revision: revision || null })
  useSheetPicks(
    ['point'],
    symbol
      ? 'Click where the revision symbol goes'
      : corner
        ? shape === 'ellipse'
          ? 'Click to set the size'
          : 'Click the opposite corner'
        : shape === 'ellipse'
          ? 'Click the center of the cloud'
          : 'Click a corner of the cloud',
    async (p) => {
      if (symbol) {
        const view = viewAt(p.at)
        await addAnnotation({ type: 'revision_symbol', at: relativeTo(view, p.at), revision }, view)
        return
      }
      if (!corner) return setCorner(p.at)
      const ok =
        shape === 'rectangle'
          ? await add({ type: 'rectangle', a: corner, b: p.at })
          : await add({
              type: 'ellipse',
              center: corner,
              rx: Math.max(Math.abs(p.at[0] - corner[0]), 1),
              ry: Math.max(Math.abs(p.at[1] - corner[1]), 1),
            })
      if (ok) setCorner(null)
    },
    symbol || shape !== 'polygon',
    setHover,
  )
  const poly = usePolygon(
    'Click the corners of the cloud; Close when done',
    (points) => void add({ type: 'polygon', points }),
    !symbol && shape === 'polygon',
  )
  return (
    <PanelShell
      title={symbol ? 'Revision Symbol' : 'Revision Cloud'}
      onCancel={() => void closeDrawingPanel()}
      onOk={() => void closeDrawingPanel()}
    >
      <TextField label="Revision" value={revision} onChange={setRevision} />
      {!symbol && (
        <>
          <Choice
            label="Shape"
            value={shape}
            options={[
              { value: 'rectangle', label: 'Rectangle' },
              { value: 'ellipse', label: 'Ellipse' },
              { value: 'polygon', label: 'Polygon' },
            ]}
            onChange={(s) => (setShape(s), setCorner(null))}
          />
          <NumberField label="Arc size (mm)" value={arc} min={1} onChange={setArc} />
          {shape === 'polygon' && poly.controls}
        </>
      )}
    </PanelShell>
  )
}

const COLUMNS: { value: Column['type']; label: string }[] = [
  { value: 'item', label: 'Item number' },
  { value: 'part_number', label: 'Part number' },
  { value: 'description', label: 'Description' },
  { value: 'quantity', label: 'Quantity' },
  { value: 'material', label: 'Material' },
  { value: 'mass', label: 'Mass' },
]

function RevisionRows(props: { rows: RevisionRow[]; onChange: (r: RevisionRow[]) => void }) {
  const set = (i: number, patch: Partial<RevisionRow>) =>
    props.onChange(props.rows.map((r, j) => (j === i ? { ...r, ...patch } : r)))
  return (
    <>
      <table className="prop-table">
        <thead>
          <tr>
            <th>Rev</th>
            <th>Description</th>
            <th>Date</th>
            <th>By</th>
            <th>Zone</th>
          </tr>
        </thead>
        <tbody>
          {props.rows.map((r, i) => (
            <tr key={i}>
              {(['revision', 'description', 'date', 'approved', 'zone'] as const).map((k) => (
                <td key={k}>
                  <input
                    value={r[k] ?? ''}
                    placeholder={k === 'revision' ? 'auto' : ''}
                    aria-label={`${k} of row ${i + 1}`}
                    onChange={(e) => set(i, { [k]: e.target.value })}
                    onKeyDown={(e) => e.stopPropagation()}
                  />
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      <div className="button-row">
        <button
          onClick={() =>
            props.onChange([
              ...props.rows,
              { description: '', date: new Date().toISOString().slice(0, 10) },
            ])
          }
        >
          Add revision
        </button>
        {props.rows.length > 0 && (
          <button onClick={() => props.onChange(props.rows.slice(0, -1))}>Remove last</button>
        )}
      </div>
    </>
  )
}

function GridEditor(props: { cells: string[][]; onChange: (c: string[][]) => void }) {
  const cols = Math.max(1, ...props.cells.map((r) => r.length))
  return (
    <>
      <table className="prop-table">
        <tbody>
          {props.cells.map((r, i) => (
            <tr key={i}>
              {Array.from({ length: cols }, (_, j) => (
                <td key={j}>
                  <input
                    value={r[j] ?? ''}
                    aria-label={`Cell ${i + 1}, ${j + 1}`}
                    onChange={(e) =>
                      props.onChange(
                        props.cells.map((row, a) =>
                          a === i
                            ? Array.from({ length: cols }, (_, b) =>
                                b === j ? e.target.value : (row[b] ?? ''),
                              )
                            : row,
                        ),
                      )
                    }
                    onKeyDown={(e) => e.stopPropagation()}
                  />
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      <div className="button-row">
        <button onClick={() => props.onChange([...props.cells, Array(cols).fill('')])}>
          Add row
        </button>
        <button onClick={() => props.onChange(props.cells.map((r) => [...r, '']))}>
          Add column
        </button>
      </div>
    </>
  )
}

/** BOM, hole, revision, general and general tolerance tables. */
export function TablePanel({ variant = 'bom' }: { variant?: string }) {
  const sheet = useSheet()
  const views = allViews()
  const assemblies = views.filter(
    (v) => v.kind.type === 'model' && v.kind.model.path.toLowerCase().endsWith('.ofa'),
  )
  const [bomView, setBomView] = useState<number | null>(assemblies[0]?.id ?? null)
  const [bom, setBom] = useState<BomType>('top_level')
  const [numbering, setNumbering] = useState<Numbering>('detailed')
  const [columns, setColumns] = useState<Column['type'][]>([
    'item',
    'part_number',
    'description',
    'quantity',
  ])
  const [extra, setExtra] = useState('')
  const [anchor, setAnchor] = useState<Corner>(variant === 'revision' ? 'top_right' : 'top_left')
  const [rows, setRows] = useState<RevisionRow[]>([
    { description: 'Released', date: new Date().toISOString().slice(0, 10) },
  ])
  const [numeric, setNumeric] = useState(false)
  const [cells, setCells] = useState<string[][]>([
    ['', ''],
    ['', ''],
  ])
  const [header, setHeader] = useState(true)
  const [cls, setCls] = useState<ToleranceClass>('m')
  const [origin, setOrigin] = useState<EntityPick | null>(null)
  const [holes, setHoles] = useState<EntityPick[]>([])
  const [tags, setTags] = useState<Tags>('alphabetic')
  const [combine, setCombine] = useState(false)
  const [placing, setPlacing] = useState(variant !== 'hole')
  const flatViews = (sheet?.views ?? []).filter(
    (v) =>
      v.kind.type === 'model' &&
      (variant === 'cut_list' || variant === 'weld'
        ? !v.kind.model.path.toLowerCase().endsWith('.ofa')
        : !!v.kind.model.flat),
  )
  const [flatView, setFlatView] = useState<number | null>(flatViews[0]?.id ?? null)
  const [showTags, setShowTags] = useState(true)
  const [punchOrigin, setPunchOrigin] = useState(false)

  const originStep = variant === 'punch' && punchOrigin && !origin
  const holeStep = (variant === 'hole' && !placing) || originStep
  useSheetPicks(
    holeStep ? ['entity'] : ['point'],
    holeStep
      ? origin
        ? 'Pick hole edges, then Place Table'
        : 'Pick the origin edge (click near the corner to measure from)'
      : 'Click where the table goes',
    async (p) => {
      if (holeStep) {
        if (p.type !== 'entity') return
        if (!origin) return setOrigin(p)
        if (variant === 'punch') return
        if (p.prim?.type !== 'arc')
          return useStore.getState().toast('Pick the circular edge of a hole', 'info')
        setHoles([...holes, p])
        return
      }
      let kind: Record<string, unknown>
      switch (variant) {
        case 'hole':
          kind = {
            table: 'hole',
            origin: {
              view: origin!.view,
              entity: origin!.entity,
              near: toViewPlane(origin!.view, origin!.at),
            },
            holes: holes.map(viewRef) as ViewRef[],
            tags,
            combine,
          }
          break
        case 'revision':
          kind = { table: 'revision', rows, numeric }
          break
        case 'general':
          kind = { table: 'general', cells, header }
          break
        case 'general_tolerance':
          kind = { table: 'general_tolerance', class: cls }
          break
        case 'cut_list':
        case 'weld':
          if (flatView === null)
            return useStore.getState().toast('Add a view of a weldment part first', 'info')
          kind =
            variant === 'cut_list'
              ? { table: 'cut_list', view: flatView }
              : { table: 'weld', view: flatView }
          break
        case 'bend':
        case 'punch':
          if (flatView === null)
            return useStore.getState().toast('Add a flat pattern view first', 'info')
          kind =
            variant === 'bend'
              ? { table: 'bend', view: flatView, show_tags: showTags }
              : {
                  table: 'punch',
                  view: flatView,
                  ...(origin
                    ? {
                        origin: {
                          view: origin.view,
                          entity: origin.entity,
                          near: toViewPlane(origin.view, origin.at),
                        },
                      }
                    : {}),
                }
          break
        default: {
          if (bomView === null) return useStore.getState().toast('Choose an assembly view', 'info')
          const cols: Column[] = columns.map((c) => ({ type: c }) as Column)
          for (const name of extra
            .split(',')
            .map((s) => s.trim())
            .filter(Boolean))
            cols.push({ type: 'property', name })
          kind = { table: 'bom', view: bomView, bom, numbering, columns: cols }
        }
      }
      const ok = await addAnnotation({ type: 'table', at: p.at, anchor, ...kind } as AnnotationKind)
      if (ok) await closeDrawingPanel()
    },
    !!sheet,
  )
  const titles: Record<string, string> = {
    bom: 'Bill of Materials',
    hole: 'Hole Table',
    revision: 'Revision Table',
    general: 'General Table',
    general_tolerance: 'General Tolerance Table',
    bend: 'Bend Table',
    punch: 'Punch Table',
    cut_list: 'Weldment Cut List',
    weld: 'Weld Table',
  }
  return (
    <PanelShell title={titles[variant] ?? 'Table'} onCancel={() => void closeDrawingPanel()}>
      {variant === 'bom' && (
        <>
          <Choice
            label="Assembly view"
            value={bomView ?? 0}
            options={
              assemblies.length
                ? assemblies.map((v) => ({ value: v.id, label: v.name }))
                : [{ value: 0, label: 'Add a view of an assembly first' }]
            }
            onChange={(v) => setBomView(v || null)}
          />
          <Choice
            label="Type"
            value={bom}
            options={[
              { value: 'top_level', label: 'Top-level only' },
              { value: 'parts_only', label: 'Parts only' },
              { value: 'indented', label: 'Indented' },
            ]}
            onChange={setBom}
          />
          {bom === 'indented' && (
            <Choice
              label="Numbering"
              value={numbering}
              options={[
                { value: 'detailed', label: 'Detailed (3.2)' },
                { value: 'flat', label: 'Flat' },
                { value: 'none', label: 'Top level only' },
              ]}
              onChange={setNumbering}
            />
          )}
          <Section title="Columns">
            {COLUMNS.map((c) => (
              <Check
                key={c.value}
                label={c.label}
                value={columns.includes(c.value)}
                onChange={(on) =>
                  setColumns(
                    on
                      ? COLUMNS.map((x) => x.value).filter(
                          (x) => x === c.value || columns.includes(x),
                        )
                      : columns.filter((x) => x !== c.value),
                  )
                }
              />
            ))}
            <TextField
              label="Property columns"
              value={extra}
              onChange={setExtra}
              placeholder="Vendor, Finish"
            />
          </Section>
        </>
      )}
      {variant === 'hole' && (
        <>
          <Prompt>
            {origin
              ? `Origin: ${entityLabel(origin.entity)}. ${holes.length} hole(s).`
              : 'Pick the origin edge near its corner.'}
          </Prompt>
          <Choice
            label="Tags"
            value={tags}
            options={[
              { value: 'alphabetic', label: 'A1, A2, B1 (by size)' },
              { value: 'numeric', label: '1, 2, 3' },
            ]}
            onChange={setTags}
          />
          <Check label="Combine same sizes" value={combine} onChange={setCombine} />
          {!placing && (
            <button disabled={!origin || !holes.length} onClick={() => setPlacing(true)}>
              Place Table
            </button>
          )}
        </>
      )}
      {variant === 'revision' && (
        <>
          <RevisionRows rows={rows} onChange={setRows} />
          <Check label="Numbered revisions (1, 2, 3)" value={numeric} onChange={setNumeric} />
          <small>The latest revision fills the title block's revision.</small>
        </>
      )}
      {variant === 'general' && (
        <>
          <GridEditor cells={cells} onChange={setCells} />
          <Check label="First row is a header" value={header} onChange={setHeader} />
        </>
      )}
      {(variant === 'cut_list' || variant === 'weld') && (
        <Choice
          label="Part view"
          value={flatView ?? 0}
          options={
            flatViews.length
              ? flatViews.map((v) => ({ value: v.id, label: v.name }))
              : [{ value: 0, label: 'Add a view of a weldment part first' }]
          }
          onChange={(v) => setFlatView(v || null)}
        />
      )}
      {(variant === 'bend' || variant === 'punch') && (
        <>
          <Choice
            label="Flat pattern view"
            value={flatView ?? 0}
            options={
              flatViews.length
                ? flatViews.map((v) => ({ value: v.id, label: v.name }))
                : [{ value: 0, label: 'Add a flat pattern view first' }]
            }
            onChange={(v) => setFlatView(v || null)}
          />
          {variant === 'bend' && (
            <Check label="Tag bends on the view" value={showTags} onChange={setShowTags} />
          )}
          {variant === 'punch' && (
            <>
              <Check
                label="Measure from a picked vertex"
                value={punchOrigin}
                onChange={(on) => {
                  setPunchOrigin(on)
                  setOrigin(null)
                }}
              />
              {punchOrigin && (
                <Prompt>
                  {origin
                    ? `Origin: ${entityLabel(origin.entity)}.`
                    : 'Pick the origin edge near its corner.'}
                </Prompt>
              )}
              {!punchOrigin && (
                <small>Positions are measured from the flat's lower left corner.</small>
              )}
            </>
          )}
        </>
      )}
      {variant === 'general_tolerance' && (
        <Choice
          label="ISO 2768-1 class"
          value={cls}
          options={[
            { value: 'f', label: 'f (fine)' },
            { value: 'm', label: 'm (medium)' },
            { value: 'c', label: 'c (coarse)' },
            { value: 'v', label: 'v (very coarse)' },
          ]}
          onChange={setCls}
        />
      )}
      <Choice
        label="Anchor corner"
        value={anchor}
        options={[
          { value: 'top_left', label: 'Top left' },
          { value: 'top_right', label: 'Top right' },
          { value: 'bottom_left', label: 'Bottom left' },
          { value: 'bottom_right', label: 'Bottom right' },
        ]}
        onChange={setAnchor}
      />
      {placing && !originStep && <Prompt>Click on the sheet to place the table.</Prompt>}
    </PanelShell>
  )
}

/** Edit an annotation: forms for common kinds, the definition for the rest. */
export function AnnotationPropertiesPanel({ editing }: { editing?: number }) {
  const drawing = useStore((s) => s.drawing)!
  const a = drawing.drawing.sheets.flatMap((s) => s.annotations ?? []).find((x) => x.id === editing)
  const [draft, setDraft] = useState<Annotation | undefined>(a)
  const [json, setJson] = useState(() => (a ? JSON.stringify(strip(a), null, 2) : ''))
  const [jsonError, setJsonError] = useState<string | null>(null)
  const [drive, setDrive] = useState('')
  if (!a || !draft) return null
  const run = useStore.getState().run
  const save = async (kind: AnnotationKind) => {
    const r = await run('drawing.annotation.update', { id: a.id, annotation: kind })
    if (r) await closeDrawingPanel()
  }
  const ok = () => void save(strip(draft))
  const set = (patch: Partial<Annotation>) => {
    const next = { ...draft, ...patch } as Annotation
    setDraft(next)
    setJson(JSON.stringify(strip(next), null, 2))
  }
  return (
    <PanelShell title="Annotation" onOk={ok} onCancel={() => void closeDrawingPanel()}>
      {draft.type === 'dimension' && (
        <>
          {draft.dim === 'model' && (
            <Section title="Model value">
              <small>
                This dimension drives {draft.key}: a new value edits, rebuilds and saves the part.
              </small>
              <div className="button-row">
                <input
                  value={drive}
                  placeholder="New value (mm or °)"
                  aria-label="New model value"
                  onChange={(e) => setDrive(e.target.value)}
                  onKeyDown={(e) => e.stopPropagation()}
                />
                <button
                  disabled={!Number.isFinite(Number(drive)) || !drive.trim()}
                  onClick={async () => {
                    const r = await run('drawing.dimension.drive', {
                      id: a.id,
                      value: Number(drive),
                    })
                    if (r) {
                      for (const path of useStore.getState().drawing?.saved_models ?? [])
                        window.openforge.notifySaved(path)
                      useStore.getState().toast('The part was changed and saved', 'info')
                      setDrive('')
                    }
                  }}
                >
                  Apply to model
                </button>
              </div>
            </Section>
          )}
          <DimensionOptions
            value={draft as unknown as DimOptions}
            onChange={(o) => set(o as Partial<Annotation>)}
            angle={draft.dim === 'angular' || draft.dim === 'angular_running'}
          />
        </>
      )}
      {draft.type === 'note' && (
        <>
          <TextField
            label="Text"
            multiline
            value={draft.text}
            onChange={(text) => set({ text } as Partial<Annotation>)}
          />
          <Choice
            label="Border"
            value={draft.border ?? 'none'}
            options={[
              { value: 'none', label: 'None' },
              { value: 'box', label: 'Box' },
              { value: 'circle', label: 'Circle' },
            ]}
            onChange={(border) => set({ border } as Partial<Annotation>)}
          />
          <NumberField
            label="Angle (°)"
            value={((draft.angle ?? 0) * 180) / Math.PI}
            onChange={(deg) => set({ angle: (deg * Math.PI) / 180 } as Partial<Annotation>)}
          />
        </>
      )}
      {draft.type === 'balloon' && (
        <Choice
          label="Shape"
          value={draft.shape ?? 'circle'}
          options={SHAPES}
          onChange={(shape) => set({ shape } as Partial<Annotation>)}
        />
      )}
      {draft.type === 'table' && (
        <>
          {draft.table === 'revision' && (
            <RevisionRows
              rows={draft.rows}
              onChange={(rows) => set({ rows } as Partial<Annotation>)}
            />
          )}
          {draft.table === 'general' && (
            <GridEditor
              cells={draft.cells}
              onChange={(cells) => set({ cells } as Partial<Annotation>)}
            />
          )}
          <IntField
            label="Rows per piece (0 = all)"
            value={draft.max_rows ?? 0}
            min={0}
            onChange={(n) => set({ max_rows: n > 0 ? n : null } as Partial<Annotation>)}
          />
          <button
            onClick={async () => {
              const path = await window.openforge.saveDialog('csv', 'table.csv')
              if (!path) return
              try {
                await call('drawing.table.csv', { id: a.id, path })
                useStore.getState().toast('Saved the table as CSV', 'info')
              } catch (e) {
                useStore.getState().toast((e as Error).message)
              }
            }}
          >
            Save as CSV…
          </button>
        </>
      )}
      <Section
        title="Definition"
        open={!['dimension', 'note', 'balloon', 'table'].includes(draft.type)}
      >
        <textarea
          className="json-editor"
          value={json}
          rows={12}
          spellCheck={false}
          aria-label="Annotation definition"
          onChange={(e) => {
            setJson(e.target.value)
            try {
              const parsed = JSON.parse(e.target.value)
              setDraft({ ...parsed, id: a.id, attach: a.attach, layer: a.layer })
              setJsonError(null)
            } catch (err) {
              setJsonError((err as Error).message)
            }
          }}
          onKeyDown={(e) => e.stopPropagation()}
        />
        {jsonError && <small className="panel-error">{jsonError}</small>}
      </Section>
    </PanelShell>
  )
}

/** An annotation's kind without its ID, view and layer. */
function strip(a: Annotation): AnnotationKind {
  const kind: Partial<Annotation> = { ...a }
  delete kind.id
  delete kind.attach
  delete kind.layer
  return kind as AnnotationKind
}

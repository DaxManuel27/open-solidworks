import { useState } from 'react'
import type { Arrows, Orient, Pick, Tolerance } from '../../../../shared/tools.gen'
import { Check, Choice, IntField, NumberField, PanelShell, Section } from '../../panels/fields'
import { useStore } from '../../store'
import type { SheetPick } from '../state'
import { useDrawing } from '../state'
import {
  Prompt,
  TextField,
  addAnnotation,
  closeDrawingPanel,
  entityLabel,
  relativeTo,
  toViewPlane,
  useSheet,
  useSheetPicks,
} from './common'

type EntityPick = SheetPick & { type: 'entity' }

/** Display options shared by every dimension. */
export interface DimOptions {
  tolerance: Tolerance
  precision?: number | null
  tolerance_precision?: number | null
  prefix?: string
  suffix?: string
  below?: string
  text?: string | null
  parentheses?: boolean
  inspection?: boolean
  dual?: boolean | null
  foreshortened?: boolean
  arrows?: Arrows
}

const TOLERANCES: { value: Tolerance['type']; label: string }[] = [
  { value: 'none', label: 'None' },
  { value: 'basic', label: 'Basic (boxed)' },
  { value: 'bilateral', label: 'Bilateral' },
  { value: 'limit', label: 'Limit' },
  { value: 'symmetric', label: 'Symmetric (±)' },
  { value: 'min', label: 'MIN' },
  { value: 'max', label: 'MAX' },
  { value: 'fit', label: 'Fit (ISO 286)' },
  { value: 'general', label: 'General (tolerance table)' },
]

const HOLE_CLASSES = [
  '',
  'H6',
  'H7',
  'H8',
  'H9',
  'H11',
  'G7',
  'F8',
  'E9',
  'D10',
  'JS7',
  'K7',
  'M7',
  'N7',
  'P7',
]
const SHAFT_CLASSES = [
  '',
  'h6',
  'h7',
  'h9',
  'h11',
  'g6',
  'f7',
  'e8',
  'd9',
  'js6',
  'k6',
  'm6',
  'n6',
  'p6',
]

function toleranceOf(type: Tolerance['type'], previous: Tolerance): Tolerance {
  if (type === previous.type) return previous
  switch (type) {
    case 'bilateral':
    case 'limit':
      return { type, plus: 0.1, minus: 0.1 }
    case 'symmetric':
      return { type, value: 0.1 }
    case 'fit':
      return { type, hole: 'H7', shaft: null, display: 'classes' }
    default:
      return { type } as Tolerance
  }
}

/** Tolerance, precision and text fields of a dimension. */
export function DimensionOptions(props: {
  value: DimOptions
  onChange: (o: DimOptions) => void
  angle?: boolean
}) {
  const o = props.value
  const set = (patch: Partial<DimOptions>) => props.onChange({ ...o, ...patch })
  const t = o.tolerance
  return (
    <>
      <Section title="Tolerance">
        <Choice
          label="Type"
          value={t.type}
          options={TOLERANCES.filter((x) => !props.angle || x.value !== 'fit')}
          onChange={(type) => set({ tolerance: toleranceOf(type, t) })}
        />
        {(t.type === 'bilateral' || t.type === 'limit') && (
          <>
            <NumberField
              label="Upper (+)"
              value={t.plus}
              onChange={(plus) => set({ tolerance: { ...t, plus } })}
            />
            <NumberField
              label="Lower (−)"
              value={t.minus}
              onChange={(minus) => set({ tolerance: { ...t, minus } })}
            />
          </>
        )}
        {t.type === 'symmetric' && (
          <NumberField
            label="±"
            value={t.value}
            min={0}
            onChange={(value) => set({ tolerance: { ...t, value } })}
          />
        )}
        {t.type === 'fit' && (
          <>
            <Choice
              label="Hole class"
              value={t.hole ?? ''}
              options={HOLE_CLASSES.map((c) => ({ value: c, label: c || 'None' }))}
              onChange={(c) => set({ tolerance: { ...t, hole: c || null } })}
            />
            <Choice
              label="Shaft class"
              value={t.shaft ?? ''}
              options={SHAFT_CLASSES.map((c) => ({ value: c, label: c || 'None' }))}
              onChange={(c) => set({ tolerance: { ...t, shaft: c || null } })}
            />
            <Choice
              label="Show"
              value={t.display ?? 'classes'}
              options={[
                { value: 'classes', label: 'Classes' },
                { value: 'classes_with_tolerance', label: 'Classes with limits' },
                { value: 'tolerance_only', label: 'Limits only' },
              ]}
              onChange={(display) => set({ tolerance: { ...t, display } })}
            />
          </>
        )}
      </Section>
      <Section title="Value" open={false}>
        <OptionalInt
          label="Decimals"
          value={o.precision}
          onChange={(precision) => set({ precision })}
        />
        <OptionalInt
          label="Tolerance decimals"
          value={o.tolerance_precision}
          onChange={(tolerance_precision) => set({ tolerance_precision })}
        />
        <TextField label="Prefix" value={o.prefix ?? ''} onChange={(prefix) => set({ prefix })} />
        <TextField label="Suffix" value={o.suffix ?? ''} onChange={(suffix) => set({ suffix })} />
        <TextField label="Below" value={o.below ?? ''} onChange={(below) => set({ below })} />
        <TextField
          label="Text override"
          value={o.text ?? ''}
          placeholder="<DIM> stands for the value"
          onChange={(text) => set({ text: text || null })}
        />
        <Check
          label="Reference (parentheses)"
          value={!!o.parentheses}
          onChange={(parentheses) => set({ parentheses })}
        />
        <Check
          label="Inspection"
          value={!!o.inspection}
          onChange={(inspection) => set({ inspection })}
        />
        {!props.angle && (
          <Choice
            label="Dual units"
            value={o.dual === null || o.dual === undefined ? 'std' : o.dual ? 'yes' : 'no'}
            options={[
              { value: 'std', label: 'Per standard' },
              { value: 'yes', label: 'Show inches' },
              { value: 'no', label: 'Off' },
            ]}
            onChange={(v) => set({ dual: v === 'std' ? null : v === 'yes' })}
          />
        )}
        <Choice
          label="Arrows"
          value={o.arrows ?? 'auto'}
          options={[
            { value: 'auto', label: 'Automatic' },
            { value: 'inside', label: 'Inside' },
            { value: 'outside', label: 'Outside' },
          ]}
          onChange={(arrows) => set({ arrows })}
        />
        <Check
          label="Foreshortened radius"
          value={!!o.foreshortened}
          onChange={(foreshortened) => set({ foreshortened })}
        />
      </Section>
    </>
  )
}

function OptionalInt(props: {
  label: string
  value?: number | null
  onChange: (v: number | null) => void
}) {
  return props.value === null || props.value === undefined ? (
    <label className="field">
      <span>{props.label}</span>
      <button onClick={() => props.onChange(2)}>Per standard — change</button>
    </label>
  ) : (
    <div>
      <IntField label={props.label} value={props.value} min={0} onChange={props.onChange} />
      <button onClick={() => props.onChange(null)}>Per standard</button>
    </div>
  )
}

const VARIANTS: Record<
  string,
  { title: string; first: string; more?: string; min: number; max: number }
> = {
  smart: { title: 'Smart Dimension', first: 'Pick one or two edges', min: 1, max: 2 },
  ordinate: {
    title: 'Ordinate Dimension',
    first: 'Pick the origin edge (click near the end to measure from)',
    more: 'Pick edges to dimension',
    min: 2,
    max: 99,
  },
  baseline: {
    title: 'Baseline Dimension',
    first: 'Pick the base edge',
    more: 'Pick edges to dimension from it',
    min: 2,
    max: 99,
  },
  chain: {
    title: 'Chain Dimension',
    first: 'Pick the first edge',
    more: 'Pick the next edges',
    min: 2,
    max: 99,
  },
  chamfer: {
    title: 'Chamfer Dimension',
    first: 'Pick the chamfer edge',
    more: 'Pick an edge next to it',
    min: 2,
    max: 2,
  },
  angular_running: {
    title: 'Angular Running Dimension',
    first: 'Pick the base edge',
    more: 'Pick edges to measure angles to',
    min: 2,
    max: 99,
  },
  arc_length: { title: 'Arc Length', first: 'Pick an arc', min: 1, max: 1 },
}

function isArc(p: EntityPick): 'circle' | 'arc' | null {
  if (p.prim?.type !== 'arc') return null
  return p.prim.end - p.prim.start >= Math.PI * 2 - 1e-6 ? 'circle' : 'arc'
}

/** Paper direction of a picked straight edge. */
function lineDir(p: EntityPick): [number, number] | null {
  return p.prim?.type === 'line' ? [p.prim.b[0] - p.prim.a[0], p.prim.b[1] - p.prim.a[1]] : null
}

function near(p: EntityPick, point: Pick['point'] = 'auto'): Pick {
  return {
    view: p.view,
    entity: p.entity,
    near: toViewPlane(p.view, p.at),
    ...(point !== 'auto' ? { point } : {}),
  }
}

/** Driven (reference) dimensions measured on view geometry. */
export function SmartDimensionPanel({ variant = 'smart' }: { variant?: string }) {
  const v = VARIANTS[variant] ?? VARIANTS['smart']!
  const [picks, setPicks] = useState<EntityPick[]>([])
  const [orient, setOrient] = useState<Orient>('auto')
  const [radial, setRadial] = useState<'auto' | 'radius' | 'diameter'>('auto')
  const [vertical, setVertical] = useState(false)
  const [opts, setOpts] = useState<DimOptions>({ tolerance: { type: 'none' } })
  const sheet = useSheet()
  const ready = picks.length >= v.min

  const build = (): Record<string, unknown> | null => {
    const [a, b] = picks
    if (!a) return null
    switch (variant) {
      case 'smart': {
        if (picks.length === 1) {
          const arc = isArc(a)
          if (arc) {
            const kind = radial === 'auto' ? (arc === 'circle' ? 'diameter' : 'radius') : radial
            return { dim: kind, target: near(a) }
          }
          return { dim: 'linear', a: near(a), orientation: orient }
        }
        const da = lineDir(a)
        const db = lineDir(b!)
        if (da && db) {
          const cross = da[0] * db[1] - da[1] * db[0]
          const parallel = Math.abs(cross) < 1e-6 * Math.hypot(...da) * Math.hypot(...db)
          const pa = { view: a.view, entity: a.entity }
          const pb = { view: b!.view, entity: b!.entity }
          if (!parallel) return { dim: 'angular', a: pa, b: pb }
          return { dim: 'linear', a: pa, b: pb, orientation: orient }
        }
        return { dim: 'linear', a: near(a), b: near(b!), orientation: orient }
      }
      case 'ordinate':
        return {
          dim: 'ordinate',
          origin: near(a),
          points: picks.slice(1).map((p) => near(p)),
          vertical,
        }
      case 'baseline':
        return {
          dim: 'baseline',
          base: near(a),
          points: picks.slice(1).map((p) => near(p)),
          orientation: orient,
        }
      case 'chain':
        return { dim: 'chain', points: picks.map((p) => near(p)), orientation: orient }
      case 'chamfer':
        return {
          dim: 'chamfer',
          edge: { view: a.view, entity: a.entity },
          adjacent: { view: b!.view, entity: b!.entity },
        }
      case 'angular_running':
        return {
          dim: 'angular_running',
          base: { view: a.view, entity: a.entity },
          lines: picks.slice(1).map((p) => ({ view: p.view, entity: p.entity })),
        }
      case 'arc_length':
        return { dim: 'arc_length', target: { view: a.view, entity: a.entity } }
    }
    return null
  }

  useSheetPicks(
    ready ? ['entity', 'point'] : ['entity'],
    picks.length >= v.max || (ready && variant !== 'smart')
      ? picks.length >= v.max
        ? 'Click where the dimension goes'
        : `${v.more ?? 'Pick more edges'}, or click empty paper to place the dimension`
      : ready
        ? 'Pick a second edge, or click empty paper to place the dimension'
        : picks.length
          ? (v.more ?? v.first)
          : v.first,
    async (p) => {
      if (p.type === 'entity' && picks.length < v.max) {
        if (picks.length && p.view !== picks[0]!.view) {
          useStore.getState().toast('Pick edges in the same view', 'info')
          return
        }
        if (variant === 'arc_length' && isArc(p) !== 'arc') {
          useStore.getState().toast('Pick an arc', 'info')
          return
        }
        setPicks([...picks, p])
        return
      }
      if (!ready) return
      const kind = build()
      if (!kind || !sheet) return
      const view = picks[0]!.view
      const at = relativeTo(view, p.at)
      const ok = await addAnnotation({ type: 'dimension', ...kind, at, ...opts } as never, view)
      if (ok) {
        setPicks([])
        useDrawing.getState().select(null)
      }
    },
  )

  const angle =
    variant === 'angular_running' || (picks.length === 2 && build()?.['dim'] === 'angular')
  return (
    <PanelShell
      title={v.title}
      onCancel={() => void closeDrawingPanel()}
      onOk={() => void closeDrawingPanel()}
    >
      <Prompt>
        {picks.length
          ? picks.map((p, i) => <div key={i}>{entityLabel(p.entity)}</div>)
          : `${v.first}.`}
      </Prompt>
      {picks.length > 0 && <button onClick={() => setPicks([])}>Clear picks</button>}
      {['smart', 'baseline', 'chain'].includes(variant) && (
        <Choice
          label="Measure"
          value={orient}
          options={[
            { value: 'auto', label: 'Where the text goes' },
            { value: 'horizontal', label: 'Horizontal' },
            { value: 'vertical', label: 'Vertical' },
            { value: 'aligned', label: 'Aligned' },
          ]}
          onChange={setOrient}
        />
      )}
      {variant === 'smart' && (
        <Choice
          label="Circles"
          value={radial}
          options={[
            { value: 'auto', label: 'Diameter for circles, radius for arcs' },
            { value: 'radius', label: 'Radius' },
            { value: 'diameter', label: 'Diameter' },
          ]}
          onChange={setRadial}
        />
      )}
      {variant === 'ordinate' && (
        <Check label="Vertical ordinates" value={vertical} onChange={setVertical} />
      )}
      <DimensionOptions value={opts} onChange={setOpts} angle={angle} />
      <small>Dimensions stay with their geometry. Press ✓ when done.</small>
    </PanelShell>
  )
}

/** Import model dimensions, hole callouts, thread callouts and axes. */
export function ModelItemsPanel() {
  const sheet = useSheet()
  const selected = useDrawing((s) => s.selection)
  const chosen = [
    ...new Set(
      selected
        .filter((p) => p.type === 'view' || p.type === 'entity')
        .map((p) => (p as { view: number }).view),
    ),
  ]
  const [scope, setScope] = useState<'sheet' | 'selected'>(chosen.length ? 'selected' : 'sheet')
  const [items, setItems] = useState({
    dimensions: true,
    hole_callouts: true,
    threads: true,
    axes: false,
  })
  const ok = async () => {
    if (!sheet) return
    const r = await useStore.getState().run('drawing.model_items', {
      sheet: sheet.id,
      ...(scope === 'selected' ? { views: chosen } : {}),
      items,
    })
    if (r) {
      const added = (useStore.getState().drawing as { added?: number[] } | null)?.added ?? []
      useStore.getState().toast(`Imported ${added.length} item(s)`, 'info')
      await closeDrawingPanel()
    }
  }
  return (
    <PanelShell
      title="Model Items"
      onOk={() => void ok()}
      onCancel={() => void closeDrawingPanel()}
    >
      <Choice
        label="Into"
        value={scope}
        options={[
          { value: 'sheet', label: 'Every part view on this sheet' },
          { value: 'selected', label: `Selected views (${chosen.length})` },
        ]}
        onChange={setScope}
      />
      <Check
        label="Dimensions marked for drawings"
        value={items.dimensions}
        onChange={(dimensions) => setItems({ ...items, dimensions })}
      />
      <Check
        label="Hole callouts"
        value={items.hole_callouts}
        onChange={(hole_callouts) => setItems({ ...items, hole_callouts })}
      />
      <Check
        label="Cosmetic thread callouts"
        value={items.threads}
        onChange={(threads) => setItems({ ...items, threads })}
      />
      <Check
        label="Centerlines and center marks"
        value={items.axes}
        onChange={(axes) => setItems({ ...items, axes })}
      />
      <small>
        Each dimension goes into the view that shows it truest, once per drawing. Imported
        dimensions drive the model: change one in its properties.
      </small>
    </PanelShell>
  )
}

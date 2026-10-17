import { useEffect, useState } from 'react'
import type {
  Allowance,
  BendOverride,
  CalculationRange,
  EndCondition as GenEnd,
  Feature,
  Ref as GenRef,
  Relief,
  ReliefKind,
  SheetMetalParams,
} from '../../../../shared/tools.gen'
import { call } from '../../core'
import type { FeatureParams, Ref } from '../../protocol'
import { type Panel, type PickKind, useStore } from '../../store'
import { type EndDraft, fromDraft, toDraft, useFeature, useParams } from '../features'
import {
  Check,
  Choice,
  LengthField,
  NumberField,
  RefBox,
  Section,
  defaultSketch,
  useSketches,
} from '../fields'

/** The parameters of one sheet metal feature type. */
export type SheetFeature<T extends Feature['type']> = Extract<Feature, { type: T }>

export const EDGE: PickKind[] = ['edge']
export const FACE: PickKind[] = ['face']
export const POINT: PickKind[] = ['vertex', 'datum', 'edge']
export const PLANE_FACE: PickKind[] = ['face', 'datum', 'std_plane']

/** Defaults for a new sheet metal body. */
export function defaultSheet(): SheetMetalParams {
  return {
    thickness: 1.5,
    radius: 1.5,
    allowance: { type: 'k_factor', k: 0.44 },
    relief: { kind: 'rectangular', ratio: 0.5 },
    gauge: null,
    use_material: false,
  }
}

/** The sheet metal parameters of the part's first sheet metal body. */
export function partSheet(): SheetMetalParams | null {
  return useStore.getState().snapshot?.sheet_metal?.[0]?.parameters ?? null
}

/** A text box. */
export function TextField(props: {
  label: string
  value: string
  onChange: (v: string) => void
  placeholder?: string
}) {
  return (
    <label className="field">
      <span>{props.label}</span>
      <input
        value={props.value}
        placeholder={props.placeholder}
        onChange={(e) => props.onChange(e.target.value)}
      />
    </label>
  )
}

/** Which of a panel's selection boxes takes viewport picks. */
export function useActiveBox<K extends string>(first: K): [K, (k: K) => void] {
  return useState<K>(first)
}

/** A selection box for references (in the generated schema's types). */
export function Refs(props: {
  label: string
  value: GenRef[]
  onChange: (r: GenRef[]) => void
  kinds: PickKind[]
  active: boolean
  onActivate: () => void
}) {
  return (
    <RefBox
      label={props.label}
      refs={props.value as unknown as Ref[]}
      onChange={(refs) => props.onChange(refs as unknown as GenRef[])}
      kinds={props.kinds}
      active={props.active}
      onActivate={props.onActivate}
    />
  )
}

/** A selection box for one reference. */
export function OneRef(props: {
  label: string
  value: GenRef | null | undefined
  onChange: (r: GenRef | null) => void
  kinds: PickKind[]
  active: boolean
  onActivate: () => void
}) {
  return (
    <RefBox
      label={props.label}
      refs={props.value ? [props.value as unknown as Ref] : []}
      onChange={(refs) => props.onChange((refs[0] as unknown as GenRef) ?? null)}
      kinds={props.kinds}
      single
      active={props.active}
      onActivate={props.onActivate}
    />
  )
}

/** End conditions between the editor's draft and the schema. */
export function endDraft(e: GenEnd | null | undefined): EndDraft {
  return toDraft(e as never)
}
export function endFrom(d: EndDraft): GenEnd | null {
  return fromDraft(d) as unknown as GenEnd | null
}

/** A reference not picked yet: panels stay invalid until it is. */
export const UNPICKED = null as unknown as GenRef

export function AllowanceEditor(props: {
  value: Allowance
  onChange: (a: Allowance) => void
  path?: string
}) {
  const a = props.value
  return (
    <>
      <Choice
        label="Bend allowance"
        value={a.type}
        options={[
          { value: 'k_factor', label: 'K-factor' },
          { value: 'bend_allowance', label: 'Bend allowance' },
          { value: 'bend_deduction', label: 'Bend deduction' },
          { value: 'bend_table', label: 'Bend table' },
          { value: 'bend_calculation', label: 'Bend calculation' },
        ]}
        onChange={(type) =>
          props.onChange(
            type === 'k_factor'
              ? { type, k: 0.44 }
              : type === 'bend_allowance' || type === 'bend_deduction'
                ? { type, value: 0 }
                : type === 'bend_table'
                  ? { type, table: '' }
                  : {
                      type,
                      ranges: [
                        { from: 0, to: 180, formula: 'angle * (radius + 0.44 * thickness)' },
                      ],
                    },
          )
        }
      />
      {a.type === 'k_factor' && (
        <NumberField
          label="K-factor"
          value={a.k}
          min={0}
          path={props.path && `${props.path}.k`}
          onChange={(k) => props.onChange({ ...a, k })}
        />
      )}
      {(a.type === 'bend_allowance' || a.type === 'bend_deduction') && (
        <LengthField
          label={a.type === 'bend_allowance' ? 'Allowance' : 'Deduction'}
          value={a.value}
          path={props.path && `${props.path}.value`}
          onChange={(value) => props.onChange({ ...a, value })}
        />
      )}
      {a.type === 'bend_table' && (
        <TextField
          label="Bend table (CSV)"
          value={a.table}
          placeholder="path/to/bend-table.csv"
          onChange={(table) => props.onChange({ ...a, table })}
        />
      )}
      {a.type === 'bend_calculation' && (
        <RangesEditor ranges={a.ranges} onChange={(ranges) => props.onChange({ ...a, ranges })} />
      )}
    </>
  )
}

function RangesEditor(props: {
  ranges: CalculationRange[]
  onChange: (r: CalculationRange[]) => void
}) {
  const set = (i: number, r: Partial<CalculationRange>) =>
    props.onChange(props.ranges.map((x, k) => (k === i ? { ...x, ...r } : x)))
  return (
    <div className="ranges">
      {props.ranges.map((r, i) => (
        <div key={i} className="range-row">
          <NumberField label="From" value={r.from} unit="°" onChange={(from) => set(i, { from })} />
          <NumberField label="To" value={r.to} unit="°" onChange={(to) => set(i, { to })} />
          <TextField
            label="Allowance"
            value={r.formula}
            onChange={(formula) => set(i, { formula })}
          />
          <button
            className="link"
            onClick={() => props.onChange(props.ranges.filter((_, k) => k !== i))}
          >
            Remove
          </button>
        </div>
      ))}
      <button
        className="link"
        onClick={() =>
          props.onChange([
            ...props.ranges,
            { from: props.ranges.at(-1)?.to ?? 0, to: 180, formula: 'angle * radius' },
          ])
        }
      >
        Add range
      </button>
      <p className="hint">
        Formulas use angle (radians), degrees, radius and thickness; the range is in degrees.
      </p>
    </div>
  )
}

export function ReliefEditor(props: { value: Relief; onChange: (r: Relief) => void }) {
  const r = props.value
  const custom = r.width !== undefined && r.width !== null
  return (
    <>
      <Choice<ReliefKind>
        label="Auto relief"
        value={r.kind ?? 'rectangular'}
        options={[
          { value: 'rectangular', label: 'Rectangular' },
          { value: 'obround', label: 'Obround' },
          { value: 'tear', label: 'Tear' },
          { value: 'none', label: 'None' },
        ]}
        onChange={(kind) => props.onChange({ ...r, kind })}
      />
      {r.kind !== 'none' && r.kind !== 'tear' && (
        <>
          <Check
            label="Size by value instead of ratio"
            value={custom}
            onChange={(on) =>
              props.onChange(on ? { ...r, width: 1, depth: 1 } : { ...r, width: null, depth: null })
            }
          />
          {custom ? (
            <>
              <LengthField
                label="Relief width"
                value={r.width ?? 1}
                onChange={(width) => props.onChange({ ...r, width })}
              />
              <LengthField
                label="Relief depth"
                value={r.depth ?? 1}
                onChange={(depth) => props.onChange({ ...r, depth })}
              />
            </>
          ) : (
            <NumberField
              label="Relief ratio"
              value={r.ratio ?? 0.5}
              min={0}
              onChange={(ratio) => props.onChange({ ...r, ratio })}
            />
          )}
        </>
      )}
    </>
  )
}

interface Gauge {
  gauge: string
  thickness: number
  radii: number[]
  k_factor: number
}

/** Built-in gauge table names, and the gauges of `table`. */
function useGauges(table: string | null): { tables: string[]; gauges: Gauge[]; error?: string } {
  const [out, setOut] = useState<{ tables: string[]; gauges: Gauge[]; error?: string }>({
    tables: [],
    gauges: [],
  })
  useEffect(() => {
    let live = true
    call<{ tables: string[]; gauges: Gauge[] | null }>('sheet_metal.gauges', { table })
      .then((r) => live && setOut({ tables: r.tables, gauges: r.gauges ?? [] }))
      .catch((e: Error) => live && setOut((o) => ({ ...o, gauges: [], error: e.message })))
    return () => {
      live = false
    }
  }, [table])
  return out
}

/** Thickness, bend radius, allowance, relief and gauge of a sheet metal body. */
export function SheetParamsEditor(props: {
  value: SheetMetalParams
  onChange: (p: SheetMetalParams) => void
  path?: string
}) {
  const p = props.value
  const g = useGauges(p.gauge?.table ?? null)
  const gauge = g.gauges.find((x) => x.gauge === p.gauge?.gauge)
  return (
    <Section title="Sheet Metal Parameters">
      <Check
        label="Use gauge table"
        value={!!p.gauge}
        onChange={(on) =>
          props.onChange({ ...p, gauge: on ? { table: 'steel', gauge: '16', radius: null } : null })
        }
      />
      {p.gauge ? (
        <>
          <Choice
            label="Table"
            value={g.tables.includes(p.gauge.table) ? p.gauge.table : '__file'}
            options={[
              ...g.tables.map((t) => ({ value: t, label: t })),
              { value: '__file', label: 'CSV file…' },
            ]}
            onChange={(table) =>
              props.onChange({
                ...p,
                gauge: { ...p.gauge!, table: table === '__file' ? '' : table },
              })
            }
          />
          {!g.tables.includes(p.gauge.table) && (
            <TextField
              label="Gauge table (CSV)"
              value={p.gauge.table}
              placeholder="path/to/gauges.csv"
              onChange={(table) => props.onChange({ ...p, gauge: { ...p.gauge!, table } })}
            />
          )}
          <Choice
            label="Gauge"
            value={p.gauge.gauge}
            options={g.gauges.map((x) => ({
              value: x.gauge,
              label: `${x.gauge} (${x.thickness} mm)`,
            }))}
            onChange={(name) =>
              props.onChange({ ...p, gauge: { ...p.gauge!, gauge: name, radius: null } })
            }
          />
          {gauge && (
            <Choice
              label="Bend radius"
              value={p.gauge.radius ?? Math.min(...gauge.radii)}
              options={gauge.radii.map((r) => ({ value: r, label: `${r} mm` }))}
              onChange={(radius) => props.onChange({ ...p, gauge: { ...p.gauge!, radius } })}
            />
          )}
          {gauge && (
            <p className="hint">
              Thickness {gauge.thickness} mm, K-factor {gauge.k_factor} from the table.
            </p>
          )}
          {g.error && <p className="panel-error">{g.error}</p>}
        </>
      ) : (
        <>
          <LengthField
            label="Thickness"
            value={p.thickness}
            path={props.path && `${props.path}.thickness`}
            onChange={(thickness) => props.onChange({ ...p, thickness })}
          />
          <LengthField
            label="Bend radius"
            value={p.radius}
            path={props.path && `${props.path}.radius`}
            onChange={(radius) => props.onChange({ ...p, radius })}
          />
        </>
      )}
      <Check
        label="Use material's K-factor and radius"
        value={!!p.use_material}
        onChange={(use_material) => props.onChange({ ...p, use_material })}
      />
      {!p.use_material && (
        <AllowanceEditor
          value={p.allowance ?? { type: 'k_factor', k: 0.44 }}
          path={props.path && `${props.path}.allowance`}
          onChange={(allowance) => props.onChange({ ...p, allowance })}
        />
      )}
      <ReliefEditor
        value={p.relief ?? { kind: 'rectangular', ratio: 0.5 }}
        onChange={(relief) => props.onChange({ ...p, relief })}
      />
    </Section>
  )
}

/** A feature's own bend radius, allowance and relief, over the body's. */
export function BendOverrideEditor(props: {
  value: BendOverride | undefined
  onChange: (b: BendOverride) => void
  relief?: boolean
}) {
  const b = props.value ?? {}
  const sheet = partSheet()
  return (
    <Section title="Bend" open={false}>
      <Check
        label="Custom bend radius"
        value={b.radius !== undefined && b.radius !== null}
        onChange={(on) => props.onChange({ ...b, radius: on ? (sheet?.radius ?? 1) : null })}
      />
      {b.radius !== undefined && b.radius !== null && (
        <LengthField
          label="Bend radius"
          value={b.radius}
          path="bend.radius"
          onChange={(radius) => props.onChange({ ...b, radius })}
        />
      )}
      <Check
        label="Custom bend allowance"
        value={!!b.allowance}
        onChange={(on) =>
          props.onChange({
            ...b,
            allowance: on ? (sheet?.allowance ?? { type: 'k_factor', k: 0.44 }) : null,
          })
        }
      />
      {b.allowance && (
        <AllowanceEditor
          value={b.allowance}
          path="bend.allowance"
          onChange={(allowance) => props.onChange({ ...b, allowance })}
        />
      )}
      {props.relief !== false && (
        <>
          <Check
            label="Custom relief"
            value={!!b.relief}
            onChange={(on) =>
              props.onChange({
                ...b,
                relief: on ? (sheet?.relief ?? { kind: 'rectangular', ratio: 0.5 }) : null,
              })
            }
          />
          {b.relief && (
            <ReliefEditor
              value={b.relief}
              onChange={(relief) => props.onChange({ ...b, relief })}
            />
          )}
        </>
      )}
    </Section>
  )
}

/** A sheet metal panel's parameters: the edited feature's, or `defaults`. */
export function useSheetFeature<T extends { type: string }>(
  panel: Panel,
  defaults: () => T,
  valid: (p: T) => boolean,
) {
  const [init] = useParams<T>(panel, defaults)
  const [p, setP] = useState<T>(init)
  const set = (patch: Partial<T>) => setP((cur) => ({ ...cur, ...patch }))
  const f = useFeature(panel, valid(p) ? (p as unknown as FeatureParams) : null)
  return { p, set, f }
}

/** The sketch a new feature should start from. */
export function startSketch(panel: Panel): number {
  const s = useStore.getState()
  return panel.sketch ?? defaultSketch(s.snapshot?.features ?? [], s.treeSelection) ?? 0
}

/** A sketch chooser with its own label. */
export function SketchPick(props: {
  label: string
  value: number
  onChange: (id: number) => void
}) {
  const sketches = useSketches()
  return (
    <Choice
      label={props.label}
      value={props.value}
      options={[
        ...(props.value ? [] : [{ value: 0, label: 'Choose a sketch…' }]),
        ...sketches.map((s) => ({ value: s.id, label: s.name })),
      ]}
      onChange={(v) => v && props.onChange(v)}
    />
  )
}

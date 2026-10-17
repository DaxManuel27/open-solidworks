/**
 * A feature panel built from the core's schema for the feature type: every
 * Phase 6 feature (surfaces, face editing, body tools, mould tools and
 * meshes) is edited here, with live preview, OK and Cancel like the
 * hand-made panels.
 */
import { useEffect, useMemo, useState } from 'react'
import { call } from '../core'
import type { FeatureParams, Ref } from '../protocol'
import { type Panel, type PickKind, useStore } from '../store'
import { FORM_FEATURES } from '../toolbars/phase6'
import { useFeature } from './features'
import {
  AngleField,
  Check,
  Choice,
  IntField,
  LengthField,
  NumberField,
  PanelShell,
  RefBox,
  Section,
  defaultSketch,
  useSketches,
} from './fields'

export interface Schema {
  type?: string | string[]
  $ref?: string
  anyOf?: Schema[]
  oneOf?: Schema[]
  properties?: Record<string, Schema>
  required?: string[]
  items?: Schema
  enum?: unknown[]
  const?: unknown
  default?: unknown
  description?: string
  minItems?: number
  maxItems?: number
  minimum?: number
  exclusiveMinimum?: number
}

/** The smallest value a number field takes, from its schema. */
function lowest(s: Schema): number {
  return s.minimum ?? s.exclusiveMinimum ?? -1e12
}

interface Catalog {
  defs: Record<string, Schema>
  features: Record<string, Schema>
}

let catalog: Promise<Catalog> | null = null

/** The feature schemas, fetched from the core once. */
function loadCatalog(): Promise<Catalog> {
  catalog ??= call<{
    tools: { name: string; parameters: Schema & { $defs: Record<string, Schema> } }[]
  }>('assistant.tools').then((r) => {
    const add = r.tools.find((t) => t.name === 'add_feature')!.parameters
    const defs = add.$defs
    const feature = defs['Feature']!
    const features: Record<string, Schema> = {}
    for (const v of feature.oneOf ?? feature.anyOf ?? []) {
      const t = v.properties?.['type']?.const
      if (typeof t === 'string') features[t] = v
    }
    return { defs, features }
  })
  return catalog
}

function useCatalog(): Catalog | string | null {
  const [c, setC] = useState<Catalog | string | null>(null)
  useEffect(() => {
    let live = true
    loadCatalog().then(
      (x) => live && setC(x),
      (e: Error) => {
        catalog = null
        if (live) setC(`The feature schemas couldn't be read: ${e.message}`)
      },
    )
    return () => {
      live = false
    }
  }, [])
  return c
}

type Kind =
  | 'ref'
  | 'refs'
  | 'body'
  | 'bodies'
  | 'facets'
  | 'vec3'
  | 'points'
  | 'list'
  | 'object'
  | 'tagged'
  | 'choice'
  | 'bool'
  | 'length'
  | 'angle'
  | 'number'
  | 'int'
  | 'sketch'
  | 'feature'
  | 'text'

function refName(s: Schema): string | null {
  return s.$ref ? s.$ref.split('/').pop()! : null
}

function resolve(c: Catalog, s: Schema): Schema {
  let cur = s
  for (let i = 0; i < 8 && cur.$ref; i++) {
    const name = refName(cur)!
    if (name === 'Ref') return cur
    cur = { ...(c.defs[name] ?? {}), description: cur.description ?? c.defs[name]?.description }
  }
  return cur
}

/** The non-null branch of an optional value, if it is one. */
function optional(s: Schema): Schema | null {
  const alts = s.anyOf ?? (Array.isArray(s.type) && s.type.includes('null') ? null : undefined)
  if (alts) {
    const rest = alts.filter((a) => a.type !== 'null')
    if (rest.length === 1 && rest.length < alts.length)
      return { ...rest[0]!, description: s.description }
  }
  if (Array.isArray(s.type) && s.type.includes('null')) {
    const t = s.type.filter((x) => x !== 'null')
    return { ...s, type: t.length === 1 ? t[0] : t }
  }
  return null
}

const BODY_NAMES = /^(body|bodies|target|tool|mesh|a|b)$/

function kindOf(c: Catalog, name: string, raw: Schema): Kind {
  if (refName(raw) === 'Ref') return 'ref'
  const s = resolve(c, raw)
  if (refName(s) === 'Ref') return 'ref'
  const variants = s.oneOf ?? s.anyOf
  if (variants?.length && variants.every((v) => v.properties?.['type']?.const !== undefined))
    return 'tagged'
  if (variants?.length && variants.every((v) => v.const !== undefined)) return 'choice'
  if (s.enum) return 'choice'
  switch (s.type) {
    case 'boolean':
      return 'bool'
    case 'integer':
      if (/sketch/.test(name)) return 'sketch'
      if (name === 'parting_line') return 'feature'
      return 'int'
    case 'number': {
      const d = s.description ?? ''
      if (/radian/i.test(d)) return 'angle'
      if (/mm²|0 to 1|fraction|factor/i.test(d)) return 'number'
      if (/\bmm\b/.test(d)) return 'length'
      return 'number'
    }
    case 'string':
      return BODY_NAMES.test(name) || /body/i.test(s.description ?? '') ? 'body' : 'text'
    case 'array': {
      const items = s.items ? resolve(c, s.items) : {}
      if (s.items && (refName(s.items) === 'Ref' || refName(items) === 'Ref')) return 'refs'
      if (items.type === 'string') return 'bodies'
      if (items.type === 'integer') return name === 'facets' ? 'facets' : 'list'
      if (items.type === 'number' && s.minItems === 3 && s.maxItems === 3) return 'vec3'
      if (items.type === 'array') return 'points'
      return 'list'
    }
    case 'object':
      return 'object'
  }
  return 'text'
}

const LABELS: Record<string, string> = {
  end1: 'Direction 1',
  end2: 'Direction 2',
  depth1: 'Depth along the pull',
  depth2: 'Depth against the pull',
  cap_ends: 'Cap ends',
  a: 'Body A',
  b: 'Body B',
  curves1: 'Direction 1 curves',
  curves2: 'Direction 2 curves',
}

function label(name: string): string {
  if (LABELS[name]) return LABELS[name]
  const s = name.replace(/_/g, ' ')
  return s.charAt(0).toUpperCase() + s.slice(1)
}

function help(s: Schema): string | undefined {
  return s.description?.split('\n').join(' ')
}

function variantLabel(v: Schema): string {
  const t = String(v.properties?.['type']?.const ?? v.const ?? '')
  return label(t)
}

function defaultValue(c: Catalog, name: string, raw: Schema): unknown {
  if (raw.default !== undefined) return structuredClone(raw.default)
  if (optional(raw)) return null
  const s = resolve(c, raw)
  if (s.default !== undefined) return structuredClone(s.default)
  switch (kindOf(c, name, raw)) {
    case 'ref':
      return null
    case 'bool':
      return false
    case 'length':
      return 10
    case 'angle':
      return (5 * Math.PI) / 180
    case 'number':
      return 1
    case 'int':
      return 1
    case 'sketch': {
      const st = useStore.getState()
      return defaultSketch(st.snapshot?.features ?? [], st.treeSelection) ?? 0
    }
    case 'feature': {
      const f = useStore.getState().snapshot?.features.filter((x) => x.type === name)
      return f?.length ? f[f.length - 1]!.id : 0
    }
    case 'body':
      return ''
    case 'choice': {
      const v = s.oneOf ?? s.anyOf
      return v?.length ? v[0]!.const : s.enum?.[0]
    }
    case 'tagged':
      return variantDefault(c, (s.oneOf ?? s.anyOf)![0]!)
    case 'vec3':
      return [0, 0, 0]
    case 'refs':
    case 'bodies':
    case 'facets':
    case 'points':
    case 'list':
      return []
    case 'object':
      return objectDefault(c, s)
    default:
      return ''
  }
}

function objectDefault(c: Catalog, s: Schema): Record<string, unknown> {
  const out: Record<string, unknown> = {}
  for (const [k, v] of Object.entries(s.properties ?? {})) {
    if (k === 'scope') continue
    if (s.required?.includes(k) || v.default !== undefined) out[k] = defaultValue(c, k, v)
  }
  return out
}

function variantDefault(c: Catalog, v: Schema): Record<string, unknown> {
  return { ...objectDefault(c, v), type: v.properties!['type']!.const }
}

/** Whether every required value is given. */
function complete(
  c: Catalog,
  name: string,
  raw: Schema,
  value: unknown,
  required: boolean,
): boolean {
  if (optional(raw)) return true
  const kind = kindOf(c, name, raw)
  const s = resolve(c, raw)
  if (value === null || value === undefined) return !required
  switch (kind) {
    case 'ref':
      return true
    case 'body':
      return !required || (typeof value === 'string' && value.length > 0)
    case 'sketch':
    case 'feature':
      return !required || (typeof value === 'number' && value > 0)
    case 'refs':
    case 'bodies':
    case 'facets':
    case 'points':
    case 'list':
      return !Array.isArray(value) || value.length >= (s.minItems ?? 0)
    case 'length':
    case 'angle':
    case 'number':
    case 'int':
      return typeof value === 'number' && Number.isFinite(value)
    case 'object':
      return objectComplete(c, s, value as Record<string, unknown>)
    case 'tagged': {
      const v = (s.oneOf ?? s.anyOf)!.find(
        (x) => x.properties?.['type']?.const === (value as { type?: unknown }).type,
      )
      return !!v && objectComplete(c, v, value as Record<string, unknown>)
    }
    default:
      return true
  }
}

function objectComplete(c: Catalog, s: Schema, value: Record<string, unknown>): boolean {
  return Object.entries(s.properties ?? {}).every(
    ([k, v]) =>
      k === 'type' || k === 'scope' || complete(c, k, v, value[k], !!s.required?.includes(k)),
  )
}

// Stable pick kinds for selection boxes, by what a field's name asks for.
const KINDS: Record<string, PickKind[]> = {
  face: ['face'],
  edge: ['edge'],
  plane: ['face', 'datum', 'std_plane'],
  point: ['vertex', 'datum', 'edge', 'face'],
  axis: ['edge', 'datum', 'face'],
  direction: ['edge', 'datum', 'std_plane', 'face'],
  surface: ['face', 'datum', 'std_plane'],
  curve: ['edge', 'datum'],
  any: ['vertex', 'edge', 'face', 'datum', 'std_plane'],
}

function pickKinds(name: string): PickKind[] {
  if (/face|faces|replacement/.test(name)) return KINDS['face']!
  if (/^(edges|edge|tears)$/.test(name)) return KINDS['edge']!
  if (/plane|neutral/.test(name)) return KINDS['plane']!
  if (/point|origin|fixed|up_to/.test(name)) return KINDS['point']!
  if (/axis/.test(name)) return KINDS['axis']!
  if (/direction|pull|toward/.test(name)) return KINDS['direction']!
  if (/surface|tool|target/.test(name)) return KINDS['surface']!
  if (/profile|path|curve|from|to|constraints/.test(name)) return KINDS['curve']!
  return KINDS['any']!
}

/** Whether a field can take whole sketch contours. */
function takesContours(name: string): boolean {
  return /profile|path|curve|from|to|constraints|edge|boundary/.test(name)
}

function ContourAdd(props: { onAdd: (r: Ref) => void }) {
  const sketches = useSketches()
  const [sketch, setSketch] = useState(0)
  const [loaded, setLoaded] = useState<{
    sketch: number
    contours: { reference: Ref; closed: boolean }[]
  }>({
    sketch: 0,
    contours: [],
  })
  useEffect(() => {
    if (!sketch) return
    let live = true
    void call<{ contours: { reference: Ref; closed: boolean }[] }>('sketch.get', {
      id: sketch,
    }).then((v) => live && setLoaded({ sketch, contours: v.contours ?? [] }))
    return () => {
      live = false
    }
  }, [sketch])
  const contours = sketch && loaded.sketch === sketch ? loaded.contours : []
  return (
    <div className="field-row">
      <Choice
        label="From sketch"
        value={sketch}
        options={[
          { value: 0, label: 'Choose…' },
          ...sketches.map((s) => ({ value: s.id, label: s.name })),
        ]}
        onChange={setSketch}
      />
      {contours.map((c, i) => (
        <button key={i} className="link" onClick={() => props.onAdd(c.reference)}>
          + {c.closed ? 'closed' : 'open'} contour {i + 1}
        </button>
      ))}
    </div>
  )
}

function BodyChoice(props: {
  label: string
  value: string
  onChange: (v: string) => void
  title?: string
}) {
  const bodies = useStore((s) => s.scene?.bodies ?? [])
  return (
    <Choice
      label={props.label}
      value={props.value}
      options={[
        ...(props.value ? [] : [{ value: '', label: 'Choose a body…' }]),
        ...bodies.map((b) => ({ value: b.id, label: `${b.name} (${b.kind ?? 'solid'})` })),
      ]}
      onChange={props.onChange}
    />
  )
}

function BodyList(props: { label: string; value: string[]; onChange: (v: string[]) => void }) {
  const bodies = useStore((s) => s.scene?.bodies ?? [])
  return (
    <div className="field">
      <span>{props.label}</span>
      {bodies.map((b) => (
        <Check
          key={b.id}
          label={`${b.name} (${b.kind ?? 'solid'})`}
          value={props.value.includes(b.id)}
          onChange={(on) =>
            props.onChange(on ? [...props.value, b.id] : props.value.filter((x) => x !== b.id))
          }
        />
      ))}
    </div>
  )
}

function Vec3Field(props: { label: string; value: number[]; onChange: (v: number[]) => void }) {
  const v = props.value.length === 3 ? props.value : [0, 0, 0]
  return (
    <div className="field">
      <span>{props.label}</span>
      {(['X', 'Y', 'Z'] as const).map((axis, i) => (
        <NumberField
          key={axis}
          label={axis}
          value={v[i]!}
          onChange={(n) => props.onChange(v.map((x, k) => (k === i ? n : x)))}
        />
      ))}
    </div>
  )
}

function PointList(props: {
  label: string
  value: number[][]
  onChange: (v: number[][]) => void
  active: boolean
  onActivate: () => void
}) {
  const setPickTarget = useStore((s) => s.setPickTarget)
  const { active, value, onChange } = props
  useEffect(() => {
    if (!active) return
    setPickTarget({
      kinds: KINDS['face']!,
      onPick: (_pick, ref) => {
        void call<{ faces: { ref: Ref; centroid: number[] }[] }>('doc.topology', {}).then((t) => {
          const key = JSON.stringify(ref)
          const f = t.faces.find((x) => JSON.stringify(x.ref) === key)
          if (f) onChange([...value, f.centroid])
        })
      },
    })
    return () => setPickTarget(null)
  }, [active, value, onChange, setPickTarget])
  return (
    <div className={`field refbox${active ? ' active' : ''}`} onClick={props.onActivate}>
      <span>{props.label}</span>
      {value.map((p, i) => (
        <div key={i} className="ref-chip">
          ({p.map((x) => x.toFixed(1)).join(', ')})
          <button className="link" onClick={() => onChange(value.filter((_, k) => k !== i))}>
            ✕
          </button>
        </div>
      ))}
      {active && <em>Click faces of the pieces</em>}
    </div>
  )
}

function FacetField(props: {
  label: string
  value: number[]
  onChange: (v: number[]) => void
  body: string
}) {
  const facets = useStore((s) => s.facets)
  const setFacets = useStore((s) => s.setFacets)
  const painting = !!facets?.painting && facets.body === props.body
  const { onChange } = props
  useEffect(() => {
    if (facets && facets.body === props.body) onChange(facets.triangles)
  }, [facets, props.body, onChange])
  useEffect(() => () => useStore.getState().setFacets(null), [])
  return (
    <div className="field">
      <span>
        {props.label}: {props.value.length} triangles
      </span>
      <button
        disabled={!props.body}
        onClick={() =>
          setFacets(
            painting
              ? { ...facets!, painting: false }
              : {
                  body: props.body,
                  triangles: props.value,
                  painting: true,
                  radius: facets?.radius ?? 2,
                },
          )
        }
      >
        {painting ? 'Stop painting' : 'Paint triangles'}
      </button>
      {painting && (
        <LengthField
          label="Brush radius"
          value={facets!.radius}
          onChange={(radius) => setFacets({ ...facets!, radius })}
        />
      )}
      {props.value.length > 0 && (
        <button
          className="link"
          onClick={() =>
            setFacets({ body: props.body, triangles: [], painting, radius: facets?.radius ?? 2 })
          }
        >
          Clear
        </button>
      )}
    </div>
  )
}

interface FieldProps {
  catalog: Catalog
  name: string
  schema: Schema
  value: unknown
  onChange: (v: unknown) => void
  path: string
  active: string
  setActive: (p: string) => void
  /** The feature's `body` value, for facet painting. */
  body: string
}

function Field(props: FieldProps) {
  const { catalog: c, name, schema, value, onChange, path, active, setActive } = props
  const inner = optional(schema)
  if (inner) {
    const on = value !== null && value !== undefined
    return (
      <>
        <Check
          label={`${label(name)}${inner.description && !on ? '' : ''}`}
          value={on}
          onChange={(v) =>
            onChange(v ? defaultValue(c, name, { ...inner, default: undefined }) : null)
          }
        />
        {on && <Field {...props} schema={inner} />}
      </>
    )
  }
  const kind = kindOf(c, name, schema)
  const s = resolve(c, schema)
  const title = label(name)
  const isActive = active === path
  const activate = () => setActive(path)
  switch (kind) {
    case 'ref':
      return (
        <>
          <RefBox
            label={title}
            refs={value ? [value as Ref] : []}
            onChange={(r) => onChange(r[0] ?? null)}
            kinds={pickKinds(name)}
            single
            origin={/point|origin/.test(name)}
            active={isActive}
            onActivate={activate}
          />
          {takesContours(name) && <ContourAdd onAdd={onChange} />}
        </>
      )
    case 'refs':
      return (
        <>
          <RefBox
            label={title}
            refs={(value as Ref[]) ?? []}
            onChange={onChange}
            kinds={pickKinds(name)}
            active={isActive}
            onActivate={activate}
          />
          {takesContours(name) && (
            <ContourAdd onAdd={(r) => onChange([...((value as Ref[]) ?? []), r])} />
          )}
        </>
      )
    case 'body':
      return <BodyChoice label={title} value={(value as string) ?? ''} onChange={onChange} />
    case 'bodies':
      return <BodyList label={title} value={(value as string[]) ?? []} onChange={onChange} />
    case 'facets':
      return (
        <FacetField
          label={title}
          value={(value as number[]) ?? []}
          onChange={onChange}
          body={props.body}
        />
      )
    case 'vec3':
      return (
        <Vec3Field label={title} value={(value as number[]) ?? [0, 0, 0]} onChange={onChange} />
      )
    case 'points':
      return (
        <PointList
          label={title}
          value={(value as number[][]) ?? []}
          onChange={onChange}
          active={isActive}
          onActivate={activate}
        />
      )
    case 'bool':
      return <Check label={title} value={!!value} onChange={onChange} />
    case 'length':
      return (
        <LengthField
          label={title}
          value={(value as number) ?? 0}
          path={path}
          min={lowest(s)}
          onChange={onChange}
        />
      )
    case 'angle':
      return (
        <AngleField label={title} value={(value as number) ?? 0} path={path} onChange={onChange} />
      )
    case 'number':
      return (
        <NumberField
          label={title}
          value={(value as number) ?? 0}
          min={lowest(s)}
          onChange={onChange}
        />
      )
    case 'int':
      return (
        <IntField
          label={title}
          value={(value as number) ?? 0}
          path={path}
          min={Math.max(lowest(s), -1e9)}
          onChange={onChange}
        />
      )
    case 'sketch':
      return <SketchChoice label={title} value={(value as number) ?? 0} onChange={onChange} />
    case 'feature':
      return (
        <FeatureChoice
          label={title}
          type={name}
          value={(value as number) ?? 0}
          onChange={onChange}
        />
      )
    case 'choice': {
      const options =
        (s.oneOf ?? s.anyOf)?.map((v) => ({
          value: String(v.const),
          label: label(String(v.const)),
        })) ?? (s.enum ?? []).map((v) => ({ value: String(v), label: label(String(v)) }))
      return <Choice label={title} value={String(value)} options={options} onChange={onChange} />
    }
    case 'tagged': {
      const variants = (s.oneOf ?? s.anyOf)!
      const cur = value as Record<string, unknown>
      const v = variants.find((x) => x.properties!['type']!.const === cur?.['type']) ?? variants[0]!
      return (
        <div className="field-group" title={help(s)}>
          <Choice
            label={title}
            value={String(cur?.['type'] ?? v.properties!['type']!.const)}
            options={variants.map((x) => ({
              value: String(x.properties!['type']!.const),
              label: variantLabel(x),
            }))}
            onChange={(t) => {
              const next = variants.find((x) => x.properties!['type']!.const === t)!
              onChange(variantDefault(c, next))
            }}
          />
          <ObjectFields {...props} schema={v} value={cur ?? {}} />
        </div>
      )
    }
    case 'object':
      return (
        <Section title={title}>
          <ObjectFields {...props} schema={s} value={(value as Record<string, unknown>) ?? {}} />
        </Section>
      )
    case 'list': {
      const items = s.items ?? {}
      const list = (value as unknown[]) ?? []
      return (
        <Section title={title}>
          {list.map((item, i) => (
            <div key={i} className="list-item">
              <Field
                {...props}
                name={`${name.replace(/s$/, '')}`}
                schema={items}
                value={item}
                path={`${path}.${i}`}
                onChange={(v) => onChange(list.map((x, k) => (k === i ? v : x)))}
              />
              <button className="link" onClick={() => onChange(list.filter((_, k) => k !== i))}>
                Remove
              </button>
            </div>
          ))}
          <button
            className="link"
            onClick={() => onChange([...list, defaultValue(c, name.replace(/s$/, ''), items)])}
          >
            + Add
          </button>
        </Section>
      )
    }
    default:
      return (
        <label className="field">
          <span>{title}</span>
          <input value={String(value ?? '')} onChange={(e) => onChange(e.target.value)} />
        </label>
      )
  }
}

function ObjectFields(props: FieldProps) {
  const { schema, value, onChange, path } = props
  const obj = (value as Record<string, unknown>) ?? {}
  return (
    <>
      {Object.entries(schema.properties ?? {})
        .filter(([k]) => k !== 'type' && k !== 'scope')
        .map(([k, v]) => (
          <Field
            key={k}
            {...props}
            name={k}
            schema={v}
            value={obj[k]}
            path={path ? `${path}.${k}` : k}
            onChange={(x) => onChange({ ...obj, [k]: x })}
          />
        ))}
    </>
  )
}

function SketchChoice(props: { label: string; value: number; onChange: (v: number) => void }) {
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

function FeatureChoice(props: {
  label: string
  type: string
  value: number
  onChange: (v: number) => void
}) {
  const features = useStore((s) => s.snapshot?.features ?? [])
  const options = features.filter((f) => f.type === props.type)
  return (
    <Choice
      label={props.label}
      value={props.value}
      options={[
        ...(props.value ? [] : [{ value: 0, label: 'Choose…' }]),
        ...options.map((f) => ({ value: f.id, label: f.name })),
      ]}
      onChange={(v) => v && props.onChange(v)}
    />
  )
}

/** The generic panel for one feature type. */
export function FeatureFormPanel({ panel }: { panel: Panel }) {
  const c = useCatalog()
  const type = panel.variant ?? ''
  const loaded = c && typeof c !== 'string' ? c : null
  const schema = loaded?.features[type]
  if (!loaded || !schema) {
    return (
      <PanelShell
        title={FORM_FEATURES[type]?.label ?? 'Feature'}
        error={typeof c === 'string' ? c : null}
        onCancel={() => void useStore.getState().closePanel()}
      >
        <div className="field">
          {!c ? 'Loading…' : loaded ? `The core has no feature "${type}".` : ''}
        </div>
      </PanelShell>
    )
  }
  return <FormBody panel={panel} catalog={loaded} schema={schema} type={type} />
}

function FormBody(props: { panel: Panel; catalog: Catalog; schema: Schema; type: string }) {
  const { panel, catalog: c, schema, type } = props
  const [params, setParams] = useState<Record<string, unknown>>(() => {
    const f = useStore.getState().snapshot?.features.find((x) => x.id === panel.editing)
    if (f) return structuredClone(f.params) as Record<string, unknown>
    const d = objectDefault(c, schema)
    if (panel.sketch && 'sketch' in d) d['sketch'] = panel.sketch
    return { ...d, type }
  })
  // The first selection box takes picks to start with.
  const [active, setActive] = useState(
    () =>
      Object.entries(schema.properties ?? {}).find(([k, v]) =>
        ['ref', 'refs', 'points'].includes(kindOf(c, k, v)),
      )?.[0] ?? '',
  )
  const valid = useMemo(() => objectComplete(c, schema, params), [c, schema, params])
  const f = useFeature(panel, valid ? (params as unknown as FeatureParams) : null)
  const body = typeof params['body'] === 'string' ? (params['body'] as string) : ''
  return (
    <PanelShell title={FORM_FEATURES[type]?.label ?? label(type)} {...f}>
      <div className="panel-help">{help(schema)}</div>
      <ObjectFields
        catalog={c}
        name=""
        schema={schema}
        value={params}
        onChange={(v) => setParams(v as Record<string, unknown>)}
        path=""
        active={active}
        setActive={setActive}
        body={body}
      />
    </PanelShell>
  )
}

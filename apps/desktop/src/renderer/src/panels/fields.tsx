import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useId,
  useRef,
  useState,
} from 'react'
import { call } from '../core'
import type { EquationValue, FeatureParams, FeatureView, Parameter, Ref } from '../protocol'
import { type PickKind, useStore } from '../store'
import { formatAngle, formatLength, parseAngle, parseLength } from '../units'

const FieldValidation = createContext<(id: string, invalid: boolean) => void>(() => {})

/** Formulas typed into a feature's fields, by parameter path; null clears one. */
export type FormulaScope = {
  formulas: Record<string, string | null>
  set: (path: string, formula: string | null) => void
}

const FormulaContext = createContext<FormulaScope | null>(null)

type Quantity = { value: number; length: number; angle: number }
type FieldKind = 'length' | 'angle' | 'scalar' | 'integer'

function fits(q: Quantity, kind: FieldKind): boolean {
  if (kind === 'length') return q.length === 1 && q.angle === 0
  if (kind === 'angle') return q.angle === 1 && q.length === 0
  return q.length === 0 && q.angle === 0 && (kind === 'scalar' || Number.isInteger(q.value))
}

export function PanelShell(props: {
  title: string
  onOk?: () => void
  onCancel: () => void
  okDisabled?: boolean
  error?: string | null
  children: ReactNode
  footer?: ReactNode
  editor?: ReactNode
  /** Lets fields with a parameter path take `=formula` entries. */
  formulaScope?: FormulaScope
}) {
  const [invalid, setInvalid] = useState<Record<string, boolean>>({})
  const register = useCallback(
    (id: string, bad: boolean) =>
      setInvalid((current) => {
        if (!!current[id] === bad) return current
        const next = { ...current }
        if (bad) next[id] = true
        else delete next[id]
        return next
      }),
    [],
  )
  const okDisabled = props.okDisabled || Object.values(invalid).some(Boolean)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement).tagName === 'INPUT') return
      if (e.key === 'Escape') props.onCancel()
      if (e.key === 'Enter' && props.onOk && !okDisabled) props.onOk()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })
  return (
    <div className="panel">
      <div className="panel-head">
        <span className="panel-title">{props.title}</span>
        {props.onOk && (
          <button className="ok" disabled={okDisabled} onClick={props.onOk} title="OK (Enter)">
            ✓
          </button>
        )}
        <button className="cancel" onClick={props.onCancel} title="Cancel (Esc)">
          ✕
        </button>
      </div>
      {props.error && <div className="panel-error">{props.error}</div>}
      <FieldValidation.Provider value={register}>
        <FormulaContext.Provider value={props.formulaScope ?? null}>
          <div className="panel-body">
            {props.editor ?? props.children}
            {props.footer}
          </div>
        </FormulaContext.Provider>
      </FieldValidation.Provider>
    </div>
  )
}

export function Section(props: { title: string; children: ReactNode; open?: boolean }) {
  const [open, setOpen] = useState(props.open ?? true)
  return (
    <div className="section">
      <button className="section-head" onClick={() => setOpen(!open)}>
        <span>{open ? '▾' : '▸'}</span> {props.title}
      </button>
      {open && <div className="section-body">{props.children}</div>}
    </div>
  )
}

function NumberInput(props: {
  label: string
  value: number
  onChange: (v: number) => void
  format: (v: number) => string
  parse: (t: string) => number | null
  min?: number
  hint?: string
  /** This field's parameter path in the feature, such as `end1.depth`. */
  path?: string
  kind: FieldKind
}) {
  const scope = useContext(FormulaContext)
  const formulas = props.path && scope ? scope : null
  const initial = formulas?.formulas[props.path!]
  const [text, setText] = useState(initial ? `=${initial}` : props.format(props.value))
  const [bad, setBad] = useState(false)
  const [note, setNote] = useState<string | null>(null)
  const fieldId = useId()
  const register = useContext(FieldValidation)
  useEffect(() => {
    register(fieldId, bad)
    return () => register(fieldId, false)
  }, [fieldId, register, bad])
  // Follow outside value changes, unless the field shows a formula.
  const [shown, setShown] = useState(props.value)
  if (!Object.is(props.value, shown)) {
    setShown(props.value)
    if (!text.trim().startsWith('=')) setText(props.format(props.value))
  }
  const commit = () => {
    const typed = text.trim()
    if (typed.startsWith('=') && formulas) {
      const expression = typed.slice(1).trim()
      void call<{ value: Quantity }>('equations.evaluate', { expression }).then(
        ({ value }) => {
          if (!fits(value, props.kind) || (props.min !== undefined && value.value < props.min)) {
            setBad(true)
            setNote(`= ${value.value}: wrong units or out of range for this field`)
            return
          }
          setBad(false)
          setNote(`= ${props.format(value.value)}`)
          setShown(value.value)
          formulas.set(props.path!, expression)
          props.onChange(value.value)
        },
        (e: Error) => {
          setBad(true)
          setNote(e.message)
        },
      )
      return
    }
    const v = props.parse(text)
    if (v === null || (props.min !== undefined && v < props.min)) {
      setBad(true)
      return
    }
    setBad(false)
    setNote(null)
    if (formulas?.formulas[props.path!]) formulas.set(props.path!, null)
    setShown(v)
    setText(props.format(v))
    props.onChange(v)
  }
  return (
    <label className="field">
      <span>{props.label}</span>
      <input
        className={bad ? 'bad' : ''}
        value={text}
        title={
          props.hint ??
          (formulas
            ? 'Type a value or expression, or start with = to link a formula, e.g. =width / 2'
            : 'Type a value or expression, e.g. 10, 3/8in, 2*5')
        }
        aria-invalid={bad}
        onChange={(e) => {
          setText(e.target.value)
          if (e.target.value.trim().startsWith('=') && formulas) {
            setBad(false)
            return
          }
          const value = props.parse(e.target.value)
          setBad(value === null || (props.min !== undefined && value < props.min))
        }}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === 'Enter') commit()
          if (e.key === 'Escape') (e.target as HTMLInputElement).blur()
          e.stopPropagation()
        }}
      />
      {note && <small className={bad ? 'panel-error' : undefined}>{note}</small>}
    </label>
  )
}

export function LengthField(props: {
  label: string
  value: number
  onChange: (mm: number) => void
  min?: number
  path?: string
}) {
  const u = useStore((s) => s.snapshot!.units)
  return (
    <NumberInput
      label={props.label}
      value={props.value}
      onChange={props.onChange}
      path={props.path}
      kind="length"
      format={(v) => formatLength(v, u)}
      parse={(t) => parseLength(t, u)}
      min={props.min ?? 0}
    />
  )
}

export function AngleField(props: {
  label: string
  value: number
  onChange: (rad: number) => void
  path?: string
}) {
  const u = useStore((s) => s.snapshot!.units)
  return (
    <NumberInput
      label={props.label}
      value={props.value}
      onChange={props.onChange}
      path={props.path}
      kind="angle"
      format={(v) => formatAngle(v, u)}
      parse={(t) => parseAngle(t, u)}
    />
  )
}

export function IntField(props: {
  label: string
  value: number
  onChange: (n: number) => void
  min?: number
  path?: string
}) {
  return (
    <NumberInput
      label={props.label}
      value={props.value}
      onChange={props.onChange}
      path={props.path}
      kind="integer"
      format={(v) => String(Math.round(v))}
      parse={(t) => {
        const n = Number(t)
        return Number.isInteger(n) ? n : null
      }}
      min={props.min ?? 1}
      hint="A whole number"
    />
  )
}

export function NumberField(props: {
  label: string
  value: number
  onChange: (n: number) => void
  min?: number
  unit?: string
  path?: string
}) {
  return (
    <NumberInput
      label={props.label}
      value={props.value}
      onChange={props.onChange}
      path={props.path}
      kind="scalar"
      format={(v) => (props.unit ? `${v} ${props.unit}` : String(v))}
      parse={(t) => {
        const n = Number(props.unit ? t.trim().replace(new RegExp(`\\s*${props.unit}$`), '') : t)
        return t.trim() !== '' && Number.isFinite(n) ? n : null
      }}
      min={props.min}
    />
  )
}

/** One authored expression and its evaluated quantity, shared by parameter editors. */
export function FormulaField(props: {
  label: string
  expression: string
  result?: EquationValue
  onChange: (expression: string) => void
  ariaLabel?: string
}) {
  const units = useStore((s) => s.snapshot!.units)
  const q = props.result?.value
  const evaluated = !q
    ? null
    : q.length === 1 && q.angle === 0
      ? formatLength(q.value, units)
      : q.angle === 1 && q.length === 0
        ? formatAngle(q.value, units)
        : `${Number(q.value.toPrecision(8))}${q.length ? ` mm^${q.length}` : ''}${q.angle ? ` rad^${q.angle}` : ''}`
  return (
    <div>
      <label className="field">
        <span>{props.label}</span>
        <input
          aria-label={props.ariaLabel ?? `Equation for ${props.label}`}
          value={props.expression}
          onChange={(e) => props.onChange(e.target.value)}
          title="Persistent formula; use explicit units such as width / 2 + 1mm"
        />
      </label>
      {props.result?.error ? (
        <small className="panel-error">{props.result.error}</small>
      ) : (
        <small aria-label={`Evaluated ${props.label}`}>{evaluated ?? 'Evaluating…'}</small>
      )}
    </div>
  )
}

export function ParameterField(props: {
  parameter: Parameter
  value: number
  expression?: string | null
  result?: EquationValue
  onChange: (value: number | string) => void
}) {
  const p = props.parameter
  const formula = typeof props.expression === 'string'
  const Field = p.integer
    ? IntField
    : p.value.angle
      ? AngleField
      : p.value.length
        ? LengthField
        : NumberField
  return (
    <div>
      <Check
        label={`${p.key} — formula`}
        value={formula}
        onChange={(enabled) =>
          props.onChange(
            enabled
              ? `${props.result?.value?.value ?? props.value}${p.value.angle ? 'rad' : p.value.length ? 'mm' : ''}`
              : (props.result?.value?.value ?? props.value),
          )
        }
      />
      {formula ? (
        <FormulaField
          label={p.key}
          expression={props.expression!}
          result={props.result}
          onChange={props.onChange}
        />
      ) : (
        <>
          <Field
            label={p.key}
            value={props.value}
            min={p.integer ? 1 : -Number.MAX_VALUE}
            onChange={props.onChange}
          />
          {props.result?.error && <small className="panel-error">{props.result.error}</small>}
        </>
      )}
    </div>
  )
}

export function Check(props: { label: string; value: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="check">
      <input
        type="checkbox"
        checked={props.value}
        onChange={(e) => props.onChange(e.target.checked)}
      />
      <span>{props.label}</span>
    </label>
  )
}

export function Choice<T extends string | number>(props: {
  label: string
  value: T
  options: { value: T; label: string }[]
  onChange: (v: T) => void
}) {
  return (
    <label className="field">
      <span>{props.label}</span>
      <select
        value={String(props.value)}
        onChange={(e) => {
          const o = props.options.find((x) => String(x.value) === e.target.value)
          if (o) props.onChange(o.value)
        }}
      >
        {props.options.map((o) => (
          <option key={String(o.value)} value={String(o.value)}>
            {o.label}
          </option>
        ))}
      </select>
    </label>
  )
}

/** Human label for a reference. */
export function refLabel(r: Ref, features: FeatureView[]): string {
  const fname = (id: number) => features.find((f) => f.id === id)?.name ?? `#${id}`
  switch (r.type) {
    case 'body':
      return `Body ${r.id}`
    case 'body_topology':
      return `${r.kind} · ${r.body}`
    case 'origin':
      return 'Origin'
    case 'front_plane':
      return 'Front Plane'
    case 'top_plane':
      return 'Top Plane'
    case 'right_plane':
      return 'Right Plane'
    case 'feature':
      return fname(r.id)
    case 'sketch_entity':
      return `${fname(r.sketch)} entity ${r.entity}`
    case 'curve_segment':
      return `Segment · ${fname(r.curve)}`
    case 'ordered_sketch_contour':
      return `${fname(r.sketch)} ordered contour · ${r.entities.length} curve(s)`
    case 'sketch_contour':
      return `${fname(r.sketch)} contour · ${r.entities.length} curve(s)`
    case 'face':
    case 'edge':
    case 'vertex': {
      // Names start with the creating feature's id, e.g. "7.s12" or "E(7.end|9.s3)".
      const m = /(\d+)\./.exec(r.name)
      const kind = r.type[0]!.toUpperCase() + r.type.slice(1)
      return m ? `${kind} · ${fname(Number(m[1]))}` : kind
    }
  }
}

/**
 * A selection box. While active, viewport picks of the allowed kinds are
 * added to it (or toggled off); its references are highlighted.
 */
export function RefBox(props: {
  label: string
  refs: Ref[]
  onChange: (refs: Ref[]) => void
  kinds: PickKind[]
  single?: boolean
  /** Offer the part origin as a choice. */
  origin?: boolean
  active: boolean
  onActivate: () => void
}) {
  const features = useStore((s) => s.snapshot?.features ?? [])
  const setPickTarget = useStore((s) => s.setPickTarget)
  const setHighlights = useStore((s) => s.setHighlights)
  const refsRef = useRef(props.refs)
  useEffect(() => {
    refsRef.current = props.refs
  })
  const { active, kinds, single, onChange } = props

  useEffect(() => {
    if (!active) return
    setPickTarget({
      kinds,
      onPick: (_pick, ref) => {
        const key = JSON.stringify(ref)
        const cur = refsRef.current
        if (cur.some((r) => JSON.stringify(r) === key))
          onChange(cur.filter((r) => JSON.stringify(r) !== key))
        else onChange(single ? [ref] : [...cur, ref])
      },
    })
    return () => setPickTarget(null)
  }, [active, kinds, single, onChange, setPickTarget])

  useEffect(() => {
    if (active) void setHighlights(props.refs)
  }, [active, props.refs, setHighlights])

  return (
    <div className={`refbox ${props.active ? 'active' : ''}`} onClick={props.onActivate}>
      <div className="refbox-label">
        {props.label}
        {props.origin && !props.refs.some((r) => r.type === 'origin') && (
          <button
            className="link"
            onClick={(e) => {
              e.stopPropagation()
              props.onChange(single ? [{ type: 'origin' }] : [...props.refs, { type: 'origin' }])
            }}
          >
            Use origin
          </button>
        )}
      </div>
      {props.active && kinds.includes('std_plane') && (
        <div className="refbox-planes">
          {(['front_plane', 'top_plane', 'right_plane'] as const).map((type) => (
            <button
              key={type}
              className="link"
              onClick={(e) => {
                e.stopPropagation()
                const ref: Ref = { type }
                const rest = refsRef.current.filter((r) => r.type !== type)
                onChange(
                  refsRef.current.some((r) => r.type === type)
                    ? rest
                    : single
                      ? [ref]
                      : [...rest, ref],
                )
              }}
            >
              {refLabel({ type }, features)}
            </button>
          ))}
        </div>
      )}
      {props.refs.length === 0 && (
        <div className="refbox-empty">
          {props.active ? 'Select in the graphics area' : 'Click to select'}
        </div>
      )}
      {props.refs.map((r) => (
        <div key={JSON.stringify(r)} className="chip">
          <span>{refLabel(r, features)}</span>
          <button
            onClick={(e) => {
              e.stopPropagation()
              props.onChange(props.refs.filter((x) => x !== r))
            }}
          >
            ×
          </button>
        </div>
      ))}
    </div>
  )
}

/** Debounced live preview of the parameters being edited. */
export function usePreview(params: FeatureParams | null, editing?: number, enabled = true) {
  const preview = useStore((s) => s.previewScene)
  const error = useStore((s) => s.previewError)
  const revision = useStore((s) => s.snapshot?.revision)
  const json = JSON.stringify(params)
  useEffect(() => {
    if (!enabled) {
      // Cancel a pending feature preview before an independent parameter preview
      // starts; a delayed null response must not erase the formula scene.
      void useStore.getState().preview(null)
      return
    }
    const t = setTimeout(() => {
      void useStore.getState().preview(json === 'null' ? null : JSON.parse(json), editing)
    }, 200)
    return () => clearTimeout(t)
  }, [json, editing, revision, enabled])
  return { preview, error }
}

/** Sketches in the tree, newest first, for sketch dropdowns. */
export function useSketches(): FeatureView[] {
  const features = useStore((s) => s.snapshot?.features ?? [])
  return features.filter((f) => f.type === 'sketch').reverse()
}

/** The sketch a new feature should use: the selected one, else the newest unused one. */
export function defaultSketch(
  features: FeatureView[],
  treeSelection: number | null,
): number | null {
  const sel = features.find((f) => f.id === treeSelection && f.type === 'sketch')
  if (sel) return sel.id
  const used = new Set<number>()
  for (const f of features) {
    for (const k of ['sketch'] as const) {
      const v = f.params[k]
      if (typeof v === 'number') used.add(v)
    }
  }
  const unused = features.filter((f) => f.type === 'sketch' && !used.has(f.id))
  return unused[unused.length - 1]?.id ?? null
}

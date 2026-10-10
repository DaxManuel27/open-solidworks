import { type ReactNode, useEffect, useRef, useState } from 'react'
import type { FeatureParams, FeatureView, Ref } from '../protocol'
import { type PickKind, useStore } from '../store'
import { formatAngle, formatLength, parseAngle, parseLength } from '../units'

export function PanelShell(props: {
  title: string
  onOk?: () => void
  onCancel: () => void
  okDisabled?: boolean
  error?: string | null
  children: ReactNode
  footer?: ReactNode
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement).tagName === 'INPUT') return
      if (e.key === 'Escape') props.onCancel()
      if (e.key === 'Enter' && props.onOk && !props.okDisabled) props.onOk()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })
  return (
    <div className="panel">
      <div className="panel-head">
        <span className="panel-title">{props.title}</span>
        {props.onOk && (
          <button
            className="ok"
            disabled={props.okDisabled}
            onClick={props.onOk}
            title="OK (Enter)"
          >
            ✓
          </button>
        )}
        <button className="cancel" onClick={props.onCancel} title="Cancel (Esc)">
          ✕
        </button>
      </div>
      {props.error && <div className="panel-error">{props.error}</div>}
      <div className="panel-body">
        {props.children}
        {props.footer}
      </div>
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
}) {
  const [text, setText] = useState(props.format(props.value))
  const [bad, setBad] = useState(false)
  const last = useRef(props.value)
  useEffect(() => {
    if (props.value !== last.current) {
      last.current = props.value
      setText(props.format(props.value))
    }
  }, [props, props.value])
  const commit = () => {
    const v = props.parse(text)
    if (v === null || (props.min !== undefined && v < props.min)) {
      setBad(true)
      return
    }
    setBad(false)
    last.current = v
    setText(props.format(v))
    props.onChange(v)
  }
  return (
    <label className="field">
      <span>{props.label}</span>
      <input
        className={bad ? 'bad' : ''}
        value={text}
        title={props.hint ?? 'Type a value or expression, e.g. 10, 3/8in, 2*5'}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === 'Enter') commit()
          if (e.key === 'Escape') (e.target as HTMLInputElement).blur()
          e.stopPropagation()
        }}
      />
    </label>
  )
}

export function LengthField(props: {
  label: string
  value: number
  onChange: (mm: number) => void
  min?: number
}) {
  const u = useStore((s) => s.snapshot!.units)
  return (
    <NumberInput
      label={props.label}
      value={props.value}
      onChange={props.onChange}
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
}) {
  const u = useStore((s) => s.snapshot!.units)
  return (
    <NumberInput
      label={props.label}
      value={props.value}
      onChange={props.onChange}
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
}) {
  return (
    <NumberInput
      label={props.label}
      value={props.value}
      onChange={(v) => props.onChange(Math.round(v))}
      format={(v) => String(Math.round(v))}
      parse={(t) => {
        const n = Number(t)
        return Number.isFinite(n) ? n : null
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
}) {
  return (
    <NumberInput
      label={props.label}
      value={props.value}
      onChange={props.onChange}
      format={(v) => (props.unit ? `${v} ${props.unit}` : String(v))}
      parse={(t) => {
        const n = parseFloat(t)
        return Number.isFinite(n) ? n : null
      }}
      min={props.min}
    />
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
export function usePreview(params: FeatureParams | null, editing?: number) {
  const preview = useStore((s) => s.previewScene)
  const error = useStore((s) => s.previewError)
  const revision = useStore((s) => s.snapshot?.revision)
  const json = JSON.stringify(params)
  useEffect(() => {
    const t = setTimeout(() => {
      void useStore.getState().preview(json === 'null' ? null : JSON.parse(json), editing)
    }, 200)
    return () => clearTimeout(t)
  }, [json, editing, revision])
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

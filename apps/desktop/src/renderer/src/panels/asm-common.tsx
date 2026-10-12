import { useEffect, useRef } from 'react'
import type { AsmComponent, AsmGeometry, AsmSnapshot, CompRef } from '../protocol'
import { type AsmSelected, useStore } from '../store'

/** A selected assembly reference with what it is. */
export interface Picked {
  ref: CompRef
  geometry: AsmGeometry | null
}

/** Every component in the tree, flattened, by joined key. */
export function componentIndex(a: AsmSnapshot | null): Map<string, AsmComponent> {
  const out = new Map<string, AsmComponent>()
  const walk = (cs: AsmComponent[]) => {
    for (const c of cs) {
      out.set(c.key.join('/'), c)
      walk(c.children ?? [])
    }
  }
  walk(a?.components ?? [])
  return out
}

export function componentName(a: AsmSnapshot | null, key: string[]): string {
  if (!key.length) return 'Assembly'
  return componentIndex(a).get(key.join('/'))?.name ?? key.join('/')
}

const ENTITY: Record<string, string> = {
  front_plane: 'Front Plane',
  top_plane: 'Top Plane',
  right_plane: 'Right Plane',
  origin: 'Origin',
}

export function pickedLabel(a: AsmSnapshot | null, p: Picked): string {
  const where = componentName(a, p.ref.component)
  const what =
    p.geometry?.type ??
    ENTITY[p.ref.entity.type] ??
    (p.ref.entity.type === 'feature' ? `feature ${p.ref.entity.id}` : p.ref.entity.type)
  return `${what} · ${where}`
}

/** The geometry kind mate filtering uses; standard planes are planes. */
export function geometryKind(p: Picked): string {
  if (p.geometry) return p.geometry.type
  if (['front_plane', 'top_plane', 'right_plane'].includes(p.ref.entity.type)) return 'plane'
  if (p.ref.entity.type === 'origin') return 'point'
  return 'plane'
}

/**
 * A selection box for assembly geometry. While active, picks in the
 * viewport of the allowed kinds are added (or removed if picked again).
 */
export function AsmRefBox(props: {
  label: string
  refs: Picked[]
  onChange: (refs: Picked[]) => void
  kinds: ('face' | 'edge' | 'vertex' | 'component')[]
  single?: boolean
  active: boolean
  onActivate: () => void
  /** Called after a pick fills a single box, to move to the next one. */
  onFilled?: () => void
}) {
  const assembly = useStore((s) => s.assembly)
  const setTarget = useStore((s) => s.setAsmTarget)
  const refs = useRef(props.refs)
  useEffect(() => {
    refs.current = props.refs
  })
  const { active, kinds, single, onChange, onFilled } = props
  useEffect(() => {
    if (!active) return
    setTarget({
      kinds,
      onPick: (s: AsmSelected) => {
        const picked: Picked = kinds.includes('component')
          ? {
              ref: { component: s.ref.component.slice(0, 1), entity: { type: 'origin' } },
              geometry: null,
            }
          : { ref: s.ref, geometry: s.geometry }
        const key = JSON.stringify(picked.ref)
        const cur = refs.current
        if (cur.some((r) => JSON.stringify(r.ref) === key))
          onChange(cur.filter((r) => JSON.stringify(r.ref) !== key))
        else {
          onChange(single ? [picked] : [...cur, picked])
          if (single) onFilled?.()
        }
      },
    })
    return () => setTarget(null)
  }, [active, kinds, single, onChange, onFilled, setTarget])
  useEffect(() => {
    if (!active) return
    useStore.setState({
      asmSelection: props.refs.map((r) => ({ pick: null, ref: r.ref, geometry: r.geometry })),
    })
  }, [active, props.refs])
  return (
    <div className={`refbox ${props.active ? 'active' : ''}`} onClick={props.onActivate}>
      <div className="refbox-label">{props.label}</div>
      {props.active && !kinds.includes('component') && (
        <div className="refbox-planes">
          {(['front_plane', 'top_plane', 'right_plane'] as const).map((type) => (
            <button
              key={type}
              className="link"
              onClick={(e) => {
                e.stopPropagation()
                const picked: Picked = { ref: { component: [], entity: { type } }, geometry: null }
                const rest = refs.current.filter(
                  (r) => JSON.stringify(r.ref) !== JSON.stringify(picked.ref),
                )
                onChange(single ? [picked] : [...rest, picked])
                if (single) onFilled?.()
              }}
            >
              {ENTITY[type]}
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
        <div key={JSON.stringify(r.ref)} className="chip">
          <span>
            {kinds.includes('component')
              ? componentName(assembly, r.ref.component)
              : pickedLabel(assembly, r)}
          </span>
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

/** Close the open panel and clear assembly picking state. */
export function closeAsmPanel(): void {
  useStore.setState({
    panel: null,
    asmTarget: null,
    asmPreview: null,
    asmSelection: [],
    asmMarks: { faces: [], meshes: [] },
  })
}

/** Run an assembly method from a panel; close it on success. */
export async function commitAsm(
  method: string,
  params: unknown,
  keepOpen = false,
): Promise<boolean> {
  const r = await useStore.getState().run(method, params)
  if (r && !keepOpen) closeAsmPanel()
  return !!r
}

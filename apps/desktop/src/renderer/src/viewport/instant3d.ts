import type { FeatureParams, Ref } from '../protocol'
import { useStore } from '../store'

/** The feature edit a drag of `delta` mm on a face asks for. */
export interface InstantEdit {
  params: FeatureParams
  /** The feature being changed, when the drag edits an existing one. */
  editing?: number
}

function faceName(ref: Ref): string | null {
  if (ref.type === 'face') return ref.name
  if (ref.type === 'body_topology' && ref.kind === 'face') return ref.name
  return null
}

/**
 * An extrude's end face changes its depth; any other face moves by an
 * offset, as Move Face does.
 */
export function instantEdit(ref: Ref, delta: number): InstantEdit | null {
  if (!Number.isFinite(delta) || Math.abs(delta) < 1e-6) return null
  const name = faceName(ref)
  if (!name) return null
  const m = /^(\d+)\.(d2\.)?end$/.exec(name)
  const features = useStore.getState().snapshot?.features ?? []
  if (m) {
    const f = features.find((x) => x.id === Number(m[1]))
    const key = m[2] ? 'end2' : 'end1'
    const end = f?.params[key] as { type?: string; depth?: number } | null | undefined
    if (f && f.type === 'extrude' && end?.type === 'blind' && typeof end.depth === 'number') {
      // A cut's end face moves the other way into the material.
      const sign = f.params['cut'] ? -1 : 1
      const depth = end.depth + sign * delta
      if (depth <= 1e-6) return null
      const params = structuredClone(f.params) as FeatureParams & Record<string, unknown>
      params[key] = { ...end, depth }
      return { params, editing: f.id }
    }
  }
  return {
    params: {
      type: 'move_face',
      faces: [ref],
      motion: { type: 'offset', distance: delta },
    } as unknown as FeatureParams,
  }
}

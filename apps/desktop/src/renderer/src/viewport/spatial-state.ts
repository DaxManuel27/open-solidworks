import { create } from 'zustand'
import type { Sketch3D, Entity3 } from '../../../shared/tools.gen'
import type { Vec3 } from '../protocol'

export type SpatialSketch = Required<Sketch3D>
export interface SpatialReport {
  status: 'solved' | 'failed'
  dof: number
  conflicting: number[]
  redundant: number[]
  entities: Record<string, 'under' | 'fully' | 'over'>
}
export type SpatialDrag = { point: number; to: Vec3 }
interface SpatialState {
  sketch: SpatialSketch | null
  report: SpatialReport | null
  selection: number[]
  tool: 'select' | 'point' | 'line' | 'bezier'
  plane: 'xy' | 'xz' | 'yz'
  depth: number
  grid: number
  lock: 'plane' | 'x' | 'y' | 'z'
  change: (sketch: SpatialSketch, drag?: SpatialDrag) => Promise<void>
}
export const useSpatial = create<SpatialState>(() => ({
  sketch: null,
  report: null,
  selection: [],
  tool: 'select',
  plane: 'xy',
  depth: 0,
  grid: 1,
  lock: 'plane',
  change: async () => {},
}))
export function spatialPoints(sk: SpatialSketch, e: Entity3): Vec3[] {
  const point = (id: number): Vec3 => {
    const p = sk.entities.find((p) => p.id === id)
    return p?.kind === 'point' ? p.at : [0, 0, 0]
  }
  return e.kind === 'point'
    ? [e.at]
    : e.kind === 'line'
      ? [point(e.a), point(e.b)]
      : e.points.map(point)
}
export function spatialSamples(sk: SpatialSketch, e: Entity3): Vec3[] {
  const p = spatialPoints(sk, e)
  if (e.kind !== 'bezier') return p
  return Array.from({ length: 49 }, (_, i) => {
    const t = i / 48,
      u = 1 - t
    return [0, 1, 2].map(
      (axis) =>
        p[0]![axis]! * u ** 3 +
        p[1]![axis]! * 3 * t * u ** 2 +
        p[2]![axis]! * 3 * t ** 2 * u +
        p[3]![axis]! * t ** 3,
    ) as Vec3
  })
}

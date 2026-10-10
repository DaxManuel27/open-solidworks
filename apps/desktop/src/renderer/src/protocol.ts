// Types for the core's JSON-RPC payloads. Mirrors crates/openforge/src/views.rs
// and the serde shapes in crates/of-model. Lengths are mm, angles radians.

export type Vec3 = [number, number, number]
export type Vec2 = [number, number]
export type Kind = 'face' | 'edge' | 'vertex'

export type Ref =
  | { type: 'origin' }
  | { type: 'front_plane' }
  | { type: 'top_plane' }
  | { type: 'right_plane' }
  | { type: 'face'; name: string }
  | { type: 'edge'; name: string }
  | { type: 'vertex'; name: string }
  | { type: 'feature'; id: number }
  | { type: 'sketch_entity'; sketch: number; entity: number }

export interface PlaneData {
  origin: Vec3
  x_dir: Vec3
  normal: Vec3
}

export interface MeshPayload {
  positions: string
  normals: string
  indices: string
  face_ranges: string
  edge_points: string
  edge_ranges: string
  vertices: string
  face_count: number
  edge_count: number
}

export type DatumGeometry =
  | { type: 'plane'; plane: PlaneData }
  | { type: 'axis'; origin: Vec3; dir: Vec3 }
  | { type: 'point'; point: Vec3 }
  | { type: 'coord_sys'; origin: Vec3; x: Vec3; y: Vec3; z: Vec3 }

export interface Scene {
  body: MeshPayload | null
  bbox: { min: Vec3; max: Vec3 } | null
  datums: { id: number; name: string; geometry: DatumGeometry }[]
  sketches: { id: number; name: string; plane: PlaneData; polylines: number[][] }[]
}

export type EndCondition =
  | { type: 'blind'; depth: number }
  | { type: 'through_all' }
  | { type: 'up_to_next' }
  | { type: 'up_to_vertex'; target: Ref }
  | { type: 'up_to_surface'; target: Ref }
  | { type: 'offset_from_surface'; target: Ref; offset: number; reverse_offset?: boolean }
  | { type: 'mid_plane'; depth: number }

export interface Thin {
  kind: 'one_direction' | 'mid_plane' | 'two_direction'
  thickness: number
  thickness2?: number
  reverse?: boolean
}

/** Feature parameters, tagged by `type` (see of-model features.rs). */
export type FeatureParams = { type: string } & Record<string, unknown>

export interface FeatureView {
  id: number
  name: string
  type: string
  suppressed: boolean
  error: string | null
  rolled_back: boolean
  sketch_status: string | null
  params: FeatureParams
}

export interface Units {
  length: 'mm' | 'cm' | 'm' | 'in' | 'ft'
  angle: 'deg' | 'rad'
  mass: 'g' | 'kg' | 'lb'
  decimals: number
}

export interface Snapshot {
  features: FeatureView[]
  rollback: number | null
  units: Units
  material: { name: string; density: number } | null
  path: string | null
  dirty: boolean
  can_undo: boolean
  can_redo: boolean
  editing: number | null
  scene?: Scene
}

export type Definition = 'under' | 'fully' | 'over'

export interface SketchPoint {
  id: number
  x: number
  y: number
  status: Definition
  construction: boolean
  external: boolean
  standalone: boolean
}

export interface SketchCurve {
  id: number
  type: 'line' | 'circle' | 'arc' | 'spline'
  points: number[]
  status: Definition
  construction: boolean
  external: boolean
  samples: Vec2[]
  center: Vec2 | null
  radius: number | null
}

export interface SketchConstraint {
  id: number
  type: string
  refs: number[]
  value?: number
  driving: boolean
  status: 'ok' | 'redundant' | 'conflicting'
  measured: number | null
  [key: string]: unknown
}

export interface SketchView {
  id: number
  plane: PlaneData
  points: SketchPoint[]
  curves: SketchCurve[]
  constraints: SketchConstraint[]
  dof: number
  status: string
  solved: boolean
  profile_error: string | null
  created?: number[]
}

export interface Measurement {
  values: { label: string; quantity: 'length' | 'angle' | 'area' | 'coordinate'; value: number }[]
  points: [Vec3, Vec3] | null
}

export interface MassProperties {
  material: string | null
  density: number
  mass: number
  volume: number
  area: number
  center: Vec3
  inertia: number[]
  principal_moments: Vec3
  principal_axes: [Vec3, Vec3, Vec3]
}

export function decodeF32(b64: string): Float32Array {
  const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0))
  return new Float32Array(bytes.buffer)
}

export function decodeU32(b64: string): Uint32Array {
  const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0))
  return new Uint32Array(bytes.buffer)
}

export function refKey(r: Ref): string {
  return JSON.stringify(r)
}

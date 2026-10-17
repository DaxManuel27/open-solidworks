// Types for the core's JSON-RPC payloads. Mirrors crates/openforge/src/views.rs
// and the serde shapes in crates/of-model. Lengths are mm, angles radians.

export type Vec3 = [number, number, number]
export interface CoreInfo {
  protocol_version: number
  scene_version: number
  part_file_version: number
  assembly_file_version?: number
  drawing_file_version?: number
  capabilities: string[]
}

export interface PreviewResult {
  revision: number
  scene: Scene | null
  error: string | null
}
export type Vec2 = [number, number]
export type Kind = 'face' | 'edge' | 'vertex'

export type Ref =
  | { type: 'body'; id: string }
  | { type: 'body_topology'; body: string; kind: Kind; name: string }
  | { type: 'origin' }
  | { type: 'front_plane' }
  | { type: 'top_plane' }
  | { type: 'right_plane' }
  | { type: 'face'; name: string }
  | { type: 'edge'; name: string }
  | { type: 'vertex'; name: string }
  | { type: 'feature'; id: number }
  | { type: 'sketch_entity'; sketch: number; entity: number }
  | { type: 'sketch_contour'; sketch: number; entities: number[] }
  | {
      type: 'ordered_sketch_contour'
      sketch: number
      entities: number[]
      reverse?: boolean
      start?: number | null
      /** Seam inside the start entity, as a fraction along it. */
      seam?: number | null
    }
  | { type: 'curve_segment'; curve: number; segment: string }

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

export interface BodyView {
  id: string
  /** Solid, surface (zero thickness) or graphics mesh. */
  kind?: 'solid' | 'surface' | 'mesh'
  name: string
  hidden: boolean
  faces: number[]
  edges: number[]
  vertices: number[]
  /** Mesh bodies: their triangle count. */
  triangles?: number
}

/** A graphics mesh body: base64 positions, triangle indices and groups. */
export interface MeshBodyView {
  id: string
  name: string
  hidden: boolean
  positions: string
  indices: string
  groups: string
  /** 0xRRGGBB per vertex, when the file had colours. */
  colors: string | null
  triangle_count: number
}

/** A Surface Flatten's triangles in the model, with each one's stretch. */
export interface FlatteningView {
  id: number
  positions: string
  indices: string
  strain: string
}

/** Mould folders. */
export interface MoldView {
  parting_lines: { feature: number; body: string; edges: number; openings: number }[]
  shut_off: string[]
  parting_surfaces: string[]
  cores: string[]
  cavities: string[]
}

/** A bend line of a flattened sheet metal body: two model points. */
export interface BendLineView {
  polyline: number[]
  up: boolean
}

/** A sheet metal body in the part, for the tree. */
export interface SheetMetalView {
  feature: number
  body: string | null
  name: string | null
  parameters: import('../../shared/tools.gen').SheetMetalParams
  bends: number
  flattened: boolean
}

export interface Scene {
  bend_lines?: BendLineView[]
  weld_beads?: { feature: number; polyline: number[]; size: number }[]
  cosmetic_threads: {
    id: number
    name: string
    body: string | null
    hidden: boolean
    polylines: number[][]
    error: string | null
    parameters: Record<string, unknown>
  }[]
  sketches3d: {
    id: number
    name: string
    polylines: number[][]
    sketch: import('./viewport/spatial-state').SpatialSketch
    report: import('./viewport/spatial-state').SpatialReport
  }[]
  curves: {
    id: number
    name: string
    polylines: number[][]
    segments: { reference: Extract<Ref, { type: 'curve_segment' }>; polylines: number[][] }[]
  }[]
  bodies: BodyView[]
  meshes?: MeshBodyView[]
  flattenings?: FlatteningView[]
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
  /** Features this one uses. */
  depends_on?: number[]
}

export interface Units {
  length: 'mm' | 'cm' | 'm' | 'in' | 'ft'
  angle: 'deg' | 'rad'
  mass: 'g' | 'kg' | 'lb'
  decimals: number
}

export interface Snapshot {
  generation: string
  configurations: { active: number; next_id: number; items: Configuration[] }
  variables: Record<string, string>
  equations: Record<string, string>
  equation_values: Record<string, EquationValue>
  parameters: Parameter[]
  revision: number
  features: FeatureView[]
  rollback: number | null
  units: Units
  material: { name: string; density: number } | null
  effective_material: { name: string; density: number } | null
  path: string | null
  dirty: boolean
  can_undo: boolean
  can_redo: boolean
  editing: number | null
  sheet_metal?: SheetMetalView[]
  mold?: MoldView
  scene?: Scene
}

export interface Quantity {
  value: number
  length: number
  angle: number
}
export interface Parameter {
  key: string
  feature: number
  path: string
  value: Quantity
  integer: boolean
}
export interface EquationValue {
  value: Quantity | null
  error: string | null
}
export interface Configuration {
  id: number
  name: string
  parameters: Record<string, number>
  equations: Record<string, string | null>
  variables: Record<string, string>
  suppressed: Record<string, boolean>
  material:
    { mode: 'inherit' | 'clear' } | { mode: 'set'; value: { name: string; density: number } }
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
  contours: {
    reference: Extract<Ref, { type: 'sketch_contour' }>
    closed: boolean
    ordered_entities: number[]
  }[]
  revision: number
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

// ---- Assemblies (crates/openforge/src/asm.rs, crates/of-assembly) ----

/** Geometry in an assembly: a component path (empty: the assembly) and a part reference. */
export interface CompRef {
  component: string[]
  entity: Ref
}

/** Translation (mm) and unit quaternion [x, y, z, w]. */
export interface Transform {
  translation: Vec3
  rotation: [number, number, number, number]
}

export interface AsmInstance {
  key: string[]
  name: string
  /** Key into `AsmScene.parts`; null for subassembly nodes and missing files. */
  part: string | null
  /** Column-major 4x4, assembly frame (exploded when an explode is shown). */
  transform: number[]
  hidden: boolean
  derived: number | null
  kind: 'part' | 'assembly' | 'missing'
}

export interface AsmScene {
  kind: 'assembly'
  parts: Record<string, MeshPayload | null>
  instances: AsmInstance[]
  explode_lines: [Vec3, Vec3][]
  exploded: boolean
  datums: Scene['datums']
  bbox: { min: Vec3; max: Vec3 } | null
}

export interface AsmComponent {
  key: string[]
  name: string
  file: string
  kind: 'part' | 'assembly' | 'missing' | 'suppressed'
  error?: string | null
  fixed: boolean
  dof?: number | null
  under_defined?: boolean
  hidden?: boolean
  derived?: number | null
  configuration?: number | null
  configurations?: { id: number; name: string }[]
  active_configuration?: number | null
  suppressed?: boolean
  flexible?: boolean
  exclude_from_bom?: boolean
  children?: AsmComponent[]
}

export type MateKind = { type: string } & Record<string, unknown>

export interface MateView {
  id: number
  name: string
  type: string
  definition: MateKind
  suppressed: boolean
  error: string | null
  conflicting: boolean
  redundant: boolean
  components: string[]
}

export interface ExplodeStepView {
  id: number
  name: string
  components: string[][]
  type: 'translate' | 'rotate' | 'radial'
  [key: string]: unknown
}

/** Empty maps and lists are left out of the JSON. */
export interface AsmConfiguration {
  id: number
  name: string
  suppressed?: Record<string, boolean>
  part_configurations?: Record<string, number>
  mates_suppressed?: Record<string, boolean>
  explode?: ExplodeStepView[]
}

export interface AsmPattern {
  id: number
  name: string
  type: string
  seeds: string[]
  skip?: number[]
  suppressed: boolean
  [key: string]: unknown
}

export interface AsmMirror {
  id: number
  name: string
  plane: CompRef
  components: { component: string; opposite_hand?: string | null }[]
  position: 'bounding_box' | 'center_of_mass' | 'origin'
  suppressed: boolean
}

export interface AsmSnapshot {
  kind: 'assembly'
  generation: string
  revision: number
  path: string | null
  dirty: boolean
  can_undo: boolean
  can_redo: boolean
  units: Units
  components: AsmComponent[]
  mates: MateView[]
  patterns: AsmPattern[]
  mirrors: AsmMirror[]
  reference: { id: number; name: string; type: string }[]
  configurations: { active: number; items: AsmConfiguration[] }
  explode: { steps: ExplodeStepView[]; shown: number | null }
  ignored_interferences: [string, string][]
  diagnostics: { solved: boolean; free: number; component_errors: Record<string, string> }
  scene?: AsmScene
  /** Set by some methods. */
  key?: string
  id?: number
  path_written?: string
}

/** World geometry of a picked assembly reference, from `asm.pick`. */
export type AsmGeometry =
  | { type: 'point'; point: Vec3 }
  | { type: 'line'; origin: Vec3; dir: Vec3; ends: [Vec3, Vec3] | null }
  | { type: 'plane'; origin: Vec3; normal: Vec3; center: Vec3 }
  | { type: 'cylinder'; origin: Vec3; dir: Vec3; radius: number }
  | { type: 'cone'; origin: Vec3; dir: Vec3 }
  | { type: 'sphere'; center: Vec3; radius: number }
  | { type: 'circle'; center: Vec3; normal: Vec3; radius: number }
  | { type: 'curve'; points: number; closed: boolean }

export interface HardwareEntry {
  kind: string
  standard: string
  name: string
  sizes: string[]
  lengths: number[]
}

export interface BomRow {
  item: string
  level: number
  quantity: number
  part_number: string
  description: string
  configuration: string
  file: string
  assembly: boolean
  material: string
  mass: number | null
  properties: Record<string, string>
}

export function compRefKey(r: CompRef): string {
  return JSON.stringify(r)
}

// Generated from the core tool catalog (crates/openforge/src/tools). Do not edit:
// run `cargo run -p openforge -- generate` instead.
// Lengths are millimetres and angles radians.

/** Boolean operations. */
export type BoolOp = 'fuse' | 'cut' | 'common'

export type ChamferMode =
  | {
    type: 'equal'
    distance: number
  }
  | {
    type: 'distance_distance'
    d1: number
    d2: number
  }
  | {
    type: 'angle_distance'
    distance: number
    angle: number
  }

export type Configuration = {
  /** Existing configuration ID from `configurations.items`. */
  id: number
  name: string
  /** Parameter key to formula; null stops inheriting the base equation. */
  equations?: Record<string, string | null>
  material?: MaterialOverride
  /** Parameter key (`f2.end1.depth`) to value in core units (mm, radians). */
  parameters?: Record<string, number>
  /** Feature ID to suppression state in this configuration. */
  suppressed?: Record<string, boolean>
  /** Variable name to formula, overriding base variables. */
  variables?: Record<string, string>
}

/**
 * Lengths are in millimetres and angles in radians. Where a dimension
 * takes a point, a circle or arc may be given and its center is used.
 */
export type ConstraintKind =
  | {
    type: 'coincident'
    a: number
    b: number
  }
  | {
    type: 'point_on_curve'
    point: number
    curve: number
  }
  | {
    type: 'midpoint'
    point: number
    line: number
  }
  | {
    type: 'horizontal'
    line: number
  }
  | {
    type: 'vertical'
    line: number
  }
  | {
    type: 'horizontal_points'
    a: number
    b: number
  }
  | {
    type: 'vertical_points'
    a: number
    b: number
  }
  | {
    type: 'parallel'
    a: number
    b: number
  }
  | {
    type: 'perpendicular'
    a: number
    b: number
  }
  | {
    type: 'collinear'
    a: number
    b: number
  }
  | {
    type: 'equal'
    a: number
    b: number
  }
  | {
    type: 'tangent'
    a: number
    b: number
  }
  | {
    type: 'concentric'
    a: number
    b: number
  }
  | {
    type: 'symmetric'
    a: number
    b: number
    about: number
  }
  | {
    type: 'fix'
    entity: number
  }
  | {
    type: 'distance'
    a: number
    b: number
    value: number
  }
  | {
    type: 'horizontal_distance'
    a: number
    b: number
    value: number
  }
  | {
    type: 'vertical_distance'
    a: number
    b: number
    value: number
  }
  | {
    type: 'length'
    line: number
    value: number
  }
  | {
    type: 'radius'
    curve: number
    value: number
  }
  | {
    type: 'diameter'
    curve: number
    value: number
  }
  | {
    type: 'angle'
    a: number
    b: number
    value: number
  }

export type CurveSelection = {
  reference: Ref
  reverse?: boolean
}

/** How far an extrude goes in one direction. */
export type EndCondition =
  | {
    type: 'blind'
    /** mm, positive. */
    depth: number
  }
  | {
    type: 'through_all'
  }
  | {
    type: 'up_to_next'
  }
  | {
    type: 'up_to_vertex'
    target: Ref
  }
  | {
    type: 'up_to_surface'
    target: Ref
  }
  | {
    type: 'offset_from_surface'
    target: Ref
    /** mm from the target surface. */
    offset: number
    reverse_offset?: boolean
  }
  | {
    type: 'mid_plane'
    /** mm, positive. */
    depth: number
  }

export type FaceKind = 'face'

export type Feature =
  | {
    type: 'split_line'
    /** A sketch or curve feature. */
    source: Ref
    /** A face; required to be a face for a split line. */
    target: Ref
    /** Projection direction, XYZ. */
    direction: [number, number, number]
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'draft'
    faces: Ref[]
    neutral_plane: Ref
    /** Signed draft angle, radians. */
    angle: number
    direction?: Ref | null
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'variable_fillet'
    edge: Ref
    /** Ordered stations; include positions 0 and 1. */
    stations: RadiusStation[]
    reverse?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'helix'
    /** Base plane; the helix axis is its normal through its origin. */
    plane: Ref
    /** mm. */
    radius: number
    /** mm per turn. */
    pitch: number
    turns: number
    left_handed?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /** Radians. */
    start_angle?: number
  }
  | {
    type: 'composite_curve'
    /** Connected curves in order. */
    segments: CurveSelection[]
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'projected_curve'
    /** A sketch or curve feature. */
    source: Ref
    /** A face; required to be a face for a split line. */
    target: Ref
    /** Projection direction, XYZ. */
    direction: [number, number, number]
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'sweep'
    /** Sketch with one closed profile and no holes. */
    profile: number
    /** One connected sketch, construction curve or edge. */
    path: Ref
    cut?: boolean
    merge?: boolean
    orientation?: SweepOrientation
    reverse_path?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'loft'
    /** Ordered profile sketch IDs, each one closed profile without holes. */
    profiles: number[]
    cut?: boolean
    merge?: boolean
    /** Profiles whose direction is reversed. */
    reversed?: number[]
    ruled?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'combine'
    /** The first selected body is the target for subtraction. */
    bodies: string[]
    operation: BoolOp
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'move_body'
    bodies: string[]
    copy?: boolean
    /** XYZ, mm. */
    origin?: [number, number, number]
    /** Euler XYZ angles in radians, applied X then Y then Z about `origin`. */
    rotation?: [number, number, number]
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /** XYZ, mm. */
    translation?: [number, number, number]
  }
  | {
    type: 'split_body'
    bodies: string[]
    /** A plane or planar face. */
    plane: Ref
    keep?: KeepSide
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'plane'
    mode: 'offset'
    reference: Ref
    /** mm along the reference normal. */
    distance: number
    flip?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'plane'
    mode: 'angle'
    reference: Ref
    axis: Ref
    /** Radians. */
    angle: number
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'plane'
    mode: 'three_points'
    points: [Ref, Ref, Ref]
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'plane'
    mode: 'mid_plane'
    a: Ref
    b: Ref
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'plane'
    mode: 'parallel_at_point'
    reference: Ref
    point: Ref
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'plane'
    mode: 'normal_to_curve'
    edge: Ref
    at_end?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'axis'
    mode: 'two_points'
    a: Ref
    b: Ref
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'axis'
    mode: 'two_planes'
    a: Ref
    b: Ref
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'axis'
    mode: 'cylinder'
    face: Ref
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'axis'
    mode: 'edge'
    edge: Ref
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'axis'
    mode: 'point_and_face'
    point: Ref
    face: Ref
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'point'
    mode: 'coordinates'
    x: number
    y: number
    z: number
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'point'
    mode: 'at'
    target: Ref
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'coord_sys'
    origin: Ref
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    x_axis?: Ref | null
    z_axis?: Ref | null
  }
  | {
    type: 'extrude'
    /** Sketch feature ID with closed profiles. */
    sketch: number
    end1: EndCondition
    /** Remove material instead of adding it. */
    cut?: boolean
    /** Also extrude the other way. */
    end2?: EndCondition | null
    /** Cut away everything outside the profile instead. */
    flip_side_to_cut?: boolean
    /** Merge with existing bodies (bosses only). */
    merge?: boolean
    /**
     * Extrude against the sketch normal. A cut on a face sketch needs
     * `true` to go into the material.
     */
    reverse?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /** Start this far from the sketch plane, mm. */
    start_offset?: number
    thin?: Thin | null
  }
  | {
    type: 'revolve'
    sketch: number
    /** A sketch line, reference axis or straight edge. */
    axis: Ref
    angle: RevolveAngle
    cut?: boolean
    /** Optional independent end condition in the opposite direction. */
    end2?: RevolveAngle | null
    merge?: boolean
    reverse?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    thin?: Thin | null
  }
  | {
    type: 'fillet'
    /** mm. */
    radius: number
    edges?: Ref[]
    /** All edges of these faces are filleted. */
    faces?: Ref[]
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'chamfer'
    mode: ChamferMode
    edges?: Ref[]
    faces?: Ref[]
    /** Swap which face each distance is measured on. */
    flip?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'shell'
    /** Wall thickness, mm. */
    thickness: number
    /** Faces to remove. None hollows the part completely. */
    faces?: Ref[]
    outward?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'hole'
    /** Sketch whose standalone points locate the holes. */
    sketch: number
    kind: HoleKind
    standard: HoleStandard
    /** Fastener size, like "M6" or "1/4-20". */
    size: string
    end: HoleEnd
    /** mm. */
    cbore_depth_override?: number | null
    /** mm. */
    cbore_diameter_override?: number | null
    /** Included angle, radians. */
    csink_angle_override?: number | null
    /** mm. */
    csink_diameter_override?: number | null
    /** mm. */
    diameter_override?: number | null
    fit?: HoleFit
    reverse?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'linear_pattern'
    features: number[]
    direction1: PatternDirection
    direction2?: PatternDirection | null
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'circular_pattern'
    features: number[]
    /** An axis, cylindrical face, circular edge or straight edge. */
    axis: Ref
    /** Total instances including the original. */
    count: number
    /** Total angle with equal spacing, or the angle between instances; radians. */
    angle: number
    equal_spacing?: boolean
    reverse?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'mirror'
    /** A plane or planar face. */
    plane: Ref
    /** Features to mirror. Empty mirrors the whole body. */
    features?: number[]
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }

export type HoleEnd =
  | {
    type: 'blind'
    /** mm. */
    depth: number
  }
  | {
    type: 'through_all'
  }

export type HoleFit = 'close' | 'normal' | 'loose'

export type HoleKind = 'simple' | 'counterbore' | 'countersink' | 'tapped'

export type HoleStandard = 'iso' | 'ansi'

export type KeepSide = 'both' | 'positive' | 'negative'

/** Kinds of sub-shape. */
export type Kind = 'face' | 'edge' | 'vertex' | 'solid'

export type Material = {
  name: string
  /** kg/m³. */
  density: number
}

export type MaterialOverride =
  | {
    mode: 'inherit'
  }
  | {
    mode: 'clear'
  }
  | {
    mode: 'set'
    value: Material
  }

export type PatternDirection = {
  /** A straight edge, sketch line, axis, or plane/face (its normal). */
  reference: Ref
  /** Distance between instances, mm. */
  spacing: number
  /** Total instances including the original. */
  count: number
  reverse?: boolean
}

/** A point given by sketch entity ID, or a new point at a position. */
export type PointArg =
  | {
    id: number
  }
  | {
    at: [number, number]
  }

export type RadiusStation = {
  /** Fraction along the edge, 0 to 1. */
  position: number
  /** mm. */
  radius: number
}

/** A reference to geometry, by stable name rather than kernel index. */
export type Ref =
  | {
    type: 'body'
    id: string
  }
  | {
    type: 'body_topology'
    body: string
    kind: Kind
    /** Stable topology name, such as `2.end` or `E(2.end|2.s5)`. */
    name: string
  }
  | {
    type: 'origin'
  }
  | {
    type: 'front_plane'
  }
  | {
    type: 'top_plane'
  }
  | {
    type: 'right_plane'
  }
  | {
    type: 'face'
    name: string
  }
  | {
    type: 'edge'
    name: string
  }
  | {
    type: 'vertex'
    name: string
  }
  | {
    type: 'feature'
    id: number
  }
  | {
    type: 'sketch_entity'
    sketch: number
    entity: number
  }

export type RevolveAngle =
  | {
    type: 'blind'
    /** Radians; 2π (6.2832) is a full turn. */
    angle: number
  }
  | {
    type: 'mid_plane'
    /** Radians. */
    angle: number
  }
  | {
    type: 'two_direction'
    /** Radians. */
    angle1: number
    /** Radians. */
    angle2: number
  }
  | {
    type: 'up_to_vertex'
    target: Ref
  }
  | {
    type: 'up_to_surface'
    target: Ref
  }
  | {
    type: 'offset_from_surface'
    target: Ref
    /** mm from the target surface. */
    offset: number
    reverse_offset?: boolean
  }

/**
 * One sketch edit, tagged by `op`. Coordinates are sketch-plane millimetres.
 * Results list new entity IDs in `created`.
 */
export type SketchOp =
  | {
    op: 'add_point'
    at: [number, number]
  }
  | {
    op: 'add_line'
    a: PointArg
    b: PointArg
  }
  | {
    op: 'add_circle'
    center: PointArg
    radius: number
  }
  | {
    op: 'add_arc3'
    start: PointArg
    end: PointArg
    via: [number, number]
  }
  | {
    op: 'add_center_arc'
    center: PointArg
    start: PointArg
    end: [number, number]
    clockwise?: boolean
  }
  | {
    op: 'add_tangent_arc'
    start: number
    tangent: [number, number]
    end: PointArg
  }
  | {
    op: 'add_spline'
    points: PointArg[]
    closed?: boolean
  }
  | {
    op: 'rectangle'
    a: [number, number]
    b: [number, number]
    center?: boolean
  }
  | {
    op: 'slot'
    a: [number, number]
    b: [number, number]
    width: number
  }
  | {
    op: 'polygon'
    center: [number, number]
    vertex: [number, number]
    sides: number
  }
  | {
    op: 'constraint'
    constraint: ConstraintKind
  }
  | {
    op: 'dimension'
    constraint: ConstraintKind
    value?: number | null
  }
  | {
    op: 'set_value'
    id: number
    value: number
  }
  | {
    op: 'set_driving'
    id: number
    driving: boolean
  }
  | {
    op: 'delete'
    ids: number[]
  }
  | {
    op: 'move'
    ids: number[]
    delta: [number, number]
  }
  | {
    op: 'trim'
    entity: number
    pick: [number, number]
  }
  | {
    op: 'extend'
    entity: number
    pick: [number, number]
  }
  | {
    op: 'offset'
    ids: number[]
    distance: number
  }
  | {
    op: 'mirror'
    ids: number[]
    about: number
  }
  | {
    op: 'fillet'
    point: number
    radius: number
  }
  | {
    op: 'construction'
    ids: number[]
    construction: boolean
  }
  | {
    op: 'convert'
    refs: Ref[]
  }

/**
 * Where a sketch lies. Sketch X/Y map to model X/Y on the front plane, X/Z on
 * the top plane and Y/Z on the right plane.
 */
export type SketchPlane =
  | {
    type: 'front_plane'
  }
  | {
    type: 'top_plane'
  }
  | {
    type: 'right_plane'
  }
  | {
    type: 'feature'
    id: number
  }
  | {
    type: 'face'
    name: string
  }
  | {
    type: 'body_topology'
    body: string
    kind: FaceKind
    name: string
  }

export type SweepOrientation = 'minimum_twist' | 'frenet' | 'fixed'

/** A thin feature: the sketch contour thickened into a wall. */
export type Thin = {
  kind: ThinKind
  /** Wall thickness, mm. */
  thickness: number
  reverse?: boolean
  /** Second-side thickness for `two_direction`, mm. */
  thickness2?: number
}

export type ThinKind = 'one_direction' | 'mid_plane' | 'two_direction'

/** Arguments of each catalog tool, by tool name. */
export interface ToolArgs {
  /** Protocol, scene and part-file versions and the capabilities this core implements. Check it before relying on a feature type. */
  core_info: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** The feature tree with each feature's error, units, material, bodies, variables, equations, registered parameters (stable keys such as f2.end1.depth) and configurations. Read it before editing. */
  part_info: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** The complete authored definition of one feature, including sketch geometry and relations. Read it before update_feature, which replaces the whole definition. */
  get_feature: {
    /** Feature ID from `part_info` `features`. */
    id: number
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Faces and edges of the visible model with stable refs, surface or curve type, size, centroid, normal and endpoints (mm). Use the refs in fillet, chamfer, shell, draft, sketch-on-face and up-to-surface features. */
  topology: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Points, curves, relations and definition status (under, fully or over defined) of one sketch. Use it to find entity IDs before adding relations. */
  get_sketch: {
    /** Feature ID from `part_info` `features`. */
    id: number
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Lengths, areas, radii, distances and angles of face, edge or vertex refs in the visible model. Results are mm and radians. */
  measure: {
    /** One to four face, edge or vertex refs. */
    refs: Ref[]
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Volume (mm³), surface area (mm²), mass (kg) from the part's material, center of mass and inertia of the active configuration. Use it to check the requested size. */
  mass_properties: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Create an empty sketch on a standard plane, a reference plane or a planar face, and start editing it. Returns the sketch ID as `id`. Add geometry with edit_sketch, then close_sketch. */
  new_sketch: {
    plane: SketchPlane
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Apply one edit to a sketch: add geometry, relations or dimensions, change a value, or delete. Returns the solved sketch, its definition status and the new entity IDs in `created`. Coordinates are sketch-plane mm. */
  edit_sketch: {
    /** Sketch feature ID. */
    id: number
    op: SketchOp
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Finish editing the open sketch. Close it before adding the feature that uses it. This does not commit the assistant's draft. */
  close_sketch: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Add a feature after the last one and rebuild. Returns its ID as `id`. Check `features[].error` in the result: a geometry failure is stored on the feature, not raised. A cut sketched on a face needs `reverse: true` to go into the material. */
  add_feature: {
    feature: Feature
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /** Tree label; omitted uses a default such as `Boss-Extrude2`. */
    name?: string | null
  }
  /** Replace one feature's complete definition (same shape as add_feature) and rebuild from it. Read get_feature first and keep fields you are not changing. For one dimension, set_parameter is simpler. */
  update_feature: {
    id: number
    /** The complete replacement definition, same shape as `add_feature`. */
    feature: Feature
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Delete features and every feature that depends on them. Returns the deleted IDs as `deleted`. Do not delete existing work to fix an error unless the user asked. */
  delete_features: {
    ids: number[]
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Set one base parameter by its stable key from part_info `parameters`, in mm or radians. Removes that parameter's base equation; configuration overrides still apply. To change one configuration only, use update_configuration. */
  set_parameter: {
    /** Stable key from `part_info` `parameters`, such as `f2.end1.depth`. */
    key: string
    /** Core units: mm for lengths, radians for angles. */
    value: number
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Replace all base variables and equations. Formulas use + - * /, min, max, abs, sqrt, sin, cos, tan, pi and units mm, cm, m, in, ft, deg, rad; lengths need units. Check part_info `equation_values` and feature errors afterwards. */
  set_equations: {
    /** Every global variable: name to formula, such as `"8mm"` or `"width / 2"`. */
    variables: Record<string, string>
    /** Every equation: parameter key to formula, such as `"thickness * 2"`. */
    equations: Record<string, string>
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Create a configuration and make it active. It inherits base values unless copy_from names a configuration to duplicate. Read its ID from part_info `configurations.items`. */
  create_configuration: {
    name: string
    /** Configuration to duplicate; omitted inherits base values. */
    copy_from?: number | null
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Replace one configuration's overrides, keeping its ID: parameter values (mm, radians), equations, variables, suppressed features and material. Features themselves are shared by every configuration. */
  update_configuration: {
    /** The complete configuration. Omitted maps and `material` are reset. */
    configuration: Configuration
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Make a configuration active, end any feature edit and rebuild the part with its overrides. */
  activate_configuration: {
    /** Configuration ID from `part_info` `configurations.items`. */
    id: number
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Delete a configuration. At least one must remain. */
  delete_configuration: {
    /** Configuration ID from `part_info` `configurations.items`. */
    id: number
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Rename, hide or show a body by its stable ID from part_info `bodies`. Hidden bodies still count for mass and export. */
  body_display: {
    /** Body ID from `part_info` `bodies`. */
    id: string
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    hidden?: boolean | null
    name?: string | null
  }
  /** Set the base material by library name, or a custom name with density in kg/m³. Omit both to clear it. */
  set_material: {
    /** kg/m³, for a custom material. */
    density?: number | null
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /**
     * Library name such as `Aluminum 6061`, or a custom name with `density`.
     * Omitted clears the material.
     */
    name?: string | null
  }
  /** Replace the open part with an empty one. Lengths are mm and angles radians. */
  new_part: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Open an .ofp part file, replacing the open part. */
  open_part: {
    path: string
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Save the part as an .ofp file. */
  save_part: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /** Omitted saves to the path the part was opened from. */
    path?: string | null
  }
  /** Export a configuration (default: active) to STEP, IGES or STL, chosen by extension. Fails while active features have errors. Hidden bodies are included. */
  export: {
    /** `.step`, `.stp`, `.iges`, `.igs` or `.stl`; the extension picks the format. */
    path: string
    /** Configuration ID; omitted exports the active configuration. */
    configuration?: number | null
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Import a .step or .iges file as a new body feature. */
  import: {
    path: string
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Undo the last change. */
  undo: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
}

export type ToolName = keyof ToolArgs

/** Core JSON-RPC method behind each tool. */
export const TOOL_METHODS = {
  core_info: 'core.info',
  part_info: 'doc.info',
  get_feature: 'doc.feature',
  topology: 'doc.topology',
  get_sketch: 'sketch.get',
  measure: 'measure',
  mass_properties: 'mass_properties',
  new_sketch: 'sketch.new',
  edit_sketch: 'sketch.edit',
  close_sketch: 'sketch.close',
  add_feature: 'feature.add',
  update_feature: 'feature.update',
  delete_features: 'feature.delete',
  set_parameter: 'parameter.set',
  set_equations: 'equations.set',
  create_configuration: 'configuration.create',
  update_configuration: 'configuration.update',
  activate_configuration: 'configuration.activate',
  delete_configuration: 'configuration.delete',
  body_display: 'body.display',
  set_material: 'doc.set_material',
  new_part: 'doc.new',
  open_part: 'doc.open',
  save_part: 'doc.save',
  export: 'export',
  import: 'import',
  undo: 'doc.undo',
} as const satisfies Record<ToolName, string>

/** Schema examples, validated and replayed on the core in CI. */
export const TOOL_EXAMPLES: { [K in ToolName]: ToolArgs[K][] } = {
  core_info: [{}],
  part_info: [{}],
  get_feature: [{"id":2}],
  topology: [{}],
  get_sketch: [{"id":3}],
  measure: [{"refs":[{"name":"2.end","type":"face"}]}, {"refs":[{"name":"2.start","type":"face"},{"name":"2.end","type":"face"}]}],
  mass_properties: [{}],
  new_sketch: [{"plane":{"type":"front_plane"}}, {"plane":{"name":"2.end","type":"face"}}],
  edit_sketch: [{"id":3,"op":{"a":[5,5],"b":[20,15],"op":"rectangle"}}, {"id":3,"op":{"center":{"at":[45,20]},"op":"add_circle","radius":4}}, {"id":3,"op":{"a":{"id":3},"b":{"at":[10,30]},"op":"add_line"}}, {"id":3,"op":{"at":[50,30],"op":"add_point"}}, {"id":3,"op":{"a":[10,30],"b":[25,30],"op":"slot","width":4}}, {"id":3,"op":{"center":[45,30],"op":"polygon","sides":6,"vertex":[48,30]}}, {"id":3,"op":{"constraint":{"a":1,"b":3,"type":"horizontal_points"},"op":"constraint"}}, {"id":3,"op":{"constraint":{"a":0,"b":1,"type":"horizontal_distance","value":0},"op":"dimension","value":30}}, {"id":3,"op":{"id":4,"op":"set_value","value":12}}, {"id":3,"op":{"ids":[3],"op":"delete"}}],
  close_sketch: [{}],
  add_feature: [{"feature":{"end1":{"depth":10,"type":"blind"},"sketch":4,"type":"extrude"}}, {"feature":{"cut":true,"end1":{"type":"through_all"},"sketch":3,"type":"extrude"}}, {"feature":{"angle":{"angle":6.283185307179586,"type":"blind"},"axis":{"entity":8,"sketch":4,"type":"sketch_entity"},"sketch":4,"type":"revolve"}}, {"feature":{"edges":[{"name":"E(2.end|2.s5)","type":"edge"}],"radius":1,"type":"fillet"}}, {"feature":{"edges":[{"name":"E(2.end|2.s5)","type":"edge"}],"mode":{"distance":1,"type":"equal"},"type":"chamfer"}}, {"feature":{"faces":[{"name":"2.end","type":"face"}],"thickness":1,"type":"shell"}}, {"feature":{"end":{"type":"through_all"},"kind":"counterbore","reverse":true,"size":"M3","sketch":3,"standard":"iso","type":"hole"}}, {"feature":{"distance":20,"mode":"offset","reference":{"type":"front_plane"},"type":"plane"}}, {"feature":{"direction1":{"count":2,"reference":{"type":"right_plane"},"spacing":70},"features":[2],"type":"linear_pattern"}}, {"feature":{"features":[2],"plane":{"type":"right_plane"},"type":"mirror"}}, {"feature":{"bodies":["b2:17a163b7910805aa"],"copy":true,"translation":[0,0,10],"type":"move_body"}}, {"feature":{"pitch":2,"plane":{"type":"top_plane"},"radius":5,"turns":3,"type":"helix"},"name":"Spring path"}],
  update_feature: [{"feature":{"end1":{"depth":8,"type":"blind"},"sketch":1,"type":"extrude"},"id":2}],
  delete_features: [{"ids":[3]}],
  set_parameter: [{"key":"f2.end1.depth","value":8}],
  set_equations: [{"equations":{"f2.end1.depth":"thickness"},"variables":{"thickness":"8mm"}}],
  create_configuration: [{"name":"Thin"}, {"copy_from":2,"name":"Long copy"}],
  update_configuration: [{"configuration":{"id":1,"name":"Default","parameters":{"f2.end1.depth":8}}}, {"configuration":{"id":2,"material":{"mode":"set","value":{"density":1240,"name":"PLA"}},"name":"Long","suppressed":{"3":true}}}],
  activate_configuration: [{"id":2}],
  delete_configuration: [{"id":2}],
  body_display: [{"id":"b2:17a163b7910805aa","name":"Plate"}, {"hidden":true,"id":"b2:17a163b7910805aa"}],
  set_material: [{"name":"Aluminum 6061"}, {"density":1150,"name":"Resin"}],
  new_part: [{}],
  open_part: [{"path":"part.ofp"}],
  save_part: [{"path":"part.ofp"}],
  export: [{"path":"part.step"}, {"configuration":2,"path":"part.stl"}],
  import: [{"path":"part.step"}],
  undo: [{}],
}

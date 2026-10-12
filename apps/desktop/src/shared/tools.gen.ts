// Generated from the core tool catalog (crates/openforge/src/tools). Do not edit:
// run `cargo run -p openforge -- generate` instead.
// Lengths are millimetres and angles radians.

/** Direction sense for mates between oriented entities. */
export type Alignment = 'aligned' | 'anti_aligned' | 'closest'

export type Axis3 = 'x' | 'y' | 'z'

export type BomType = 'top_level' | 'parts_only' | 'indented'

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

export type Change = 'added' | 'modified' | 'removed'

export type Changed = {
  id: number
  name: string
  /** Feature `type`, such as `extrude` or `sketch`. */
  kind: string
  change: Change
}

export type Collision = {
  /** Check only the dragged component. */
  dragged_only?: boolean
}

/**
 * A reference to geometry in the assembly: a component path (empty for the
 * assembly's own planes, axes and sketches) and a part reference inside it.
 */
export type CompRef = {
  /** Face, edge, vertex, plane, axis or point inside that component. */
  entity: Ref
  /**
   * Instance keys from the top: `["2"]`, `["4", "1"]` inside subassembly
   * 4, or `["2@p7.1"]` for a pattern instance. Empty means the assembly.
   */
  component?: string[]
}

/** One direction of a linear component pattern. */
export type ComponentPatternDirection = {
  /** Edge, axis or plane (normal) for the direction. */
  reference: CompRef
  spacing: number
  count: number
  reverse?: boolean
}

export type Configuration = {
  /** Existing configuration ID from `configurations.items`. */
  id: number
  name: string
  additionalProperties?: unknown
  /** Parameter key to formula; null stops inheriting the base equation. */
  equations?: Record<string, string | null>
  material?: MaterialOverride
  /** Parameter key (`f2.end1.depth`) to value in core units (mm, radians). */
  parameters?: Record<string, number>
  /** Configuration-specific custom properties, overriding the part's. */
  properties?: Record<string, Property>
  /** Feature ID to suppression state in this configuration. */
  suppressed?: Record<string, boolean>
  /** Variable name to formula, overriding base variables. */
  variables?: Record<string, string>
}

export type Constraint3 =
  | {
    kind: 'coincident'
    id: number
    a: number
    b: number
    /**
     * A model reference, as JSON, that sets this relation's location on every
     * rebuild: a vertex or reference point for `fixed`, a plane or planar face
     * for `point_on_plane`. The sketch then follows that geometry.
     */
    external?: string | null
  }
  | {
    kind: 'fixed'
    id: number
    point: number
    at: [number, number, number]
    /**
     * A model reference, as JSON, that sets this relation's location on every
     * rebuild: a vertex or reference point for `fixed`, a plane or planar face
     * for `point_on_plane`. The sketch then follows that geometry.
     */
    external?: string | null
  }
  | {
    kind: 'distance'
    id: number
    a: number
    b: number
    value: number
    /**
     * A model reference, as JSON, that sets this relation's location on every
     * rebuild: a vertex or reference point for `fixed`, a plane or planar face
     * for `point_on_plane`. The sketch then follows that geometry.
     */
    external?: string | null
  }
  | {
    kind: 'axis'
    id: number
    line: number
    axis: Axis3
    /**
     * A model reference, as JSON, that sets this relation's location on every
     * rebuild: a vertex or reference point for `fixed`, a plane or planar face
     * for `point_on_plane`. The sketch then follows that geometry.
     */
    external?: string | null
  }
  | {
    kind: 'parallel'
    id: number
    a: number
    b: number
    /**
     * A model reference, as JSON, that sets this relation's location on every
     * rebuild: a vertex or reference point for `fixed`, a plane or planar face
     * for `point_on_plane`. The sketch then follows that geometry.
     */
    external?: string | null
  }
  | {
    kind: 'perpendicular'
    id: number
    a: number
    b: number
    /**
     * A model reference, as JSON, that sets this relation's location on every
     * rebuild: a vertex or reference point for `fixed`, a plane or planar face
     * for `point_on_plane`. The sketch then follows that geometry.
     */
    external?: string | null
  }
  | {
    kind: 'point_on_plane'
    id: number
    point: number
    origin: [number, number, number]
    normal: [number, number, number]
    /**
     * A model reference, as JSON, that sets this relation's location on every
     * rebuild: a vertex or reference point for `fixed`, a plane or planar face
     * for `point_on_plane`. The sketch then follows that geometry.
     */
    external?: string | null
  }
  | {
    kind: 'tangent'
    id: number
    a: number
    b: number
    end_a?: boolean
    end_b?: boolean
    /**
     * A model reference, as JSON, that sets this relation's location on every
     * rebuild: a vertex or reference point for `fixed`, a plane or planar face
     * for `point_on_plane`. The sketch then follows that geometry.
     */
    external?: string | null
  }
  | {
    kind: 'point_on_curve'
    id: number
    point: number
    curve: number
    /**
     * A model reference, as JSON, that sets this relation's location on every
     * rebuild: a vertex or reference point for `fixed`, a plane or planar face
     * for `point_on_plane`. The sketch then follows that geometry.
     */
    external?: string | null
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

/** An explicit complete closed contour. The first traversed entity is its seam. */
export type ContourSelection = {
  sketch: number
  entities: number[]
  reverse?: boolean
  /**
   * Put the seam inside the start entity, this fraction along it in the
   * traversal direction (0 < seam < 1): by length for lines, arcs and
   * circles, by curve parameter for Bezier curves. The entity is split
   * there, except a circle, which just starts there. Not for splines.
   */
  seam?: OrderedFloat | null
  /** Entity at the start/seam; must be a member of the selected contour. */
  start?: number | null
}

/** IDs created by the call, in the form the next call accepts. */
export type Created = {
  /** New sketch entity and relation IDs, from `edit_sketch`. */
  entities: number[]
  configuration?: number | null
  feature?: number | null
}

export type CurveSelection = {
  reference: Ref
  reverse?: boolean
}

export type DragPhase = 'start' | 'move' | 'end'

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

export type Entity3 =
  | {
    kind: 'point'
    id: number
    at: [number, number, number]
    construction?: boolean
  }
  | {
    kind: 'line'
    id: number
    a: number
    b: number
    construction?: boolean
  }
  | {
    kind: 'bezier'
    id: number
    points: [number, number, number, number]
    construction?: boolean
  }

/** Stable error codes reported to tool clients. Messages may change; codes do not. */
export type ErrorCode =
  | 'E_INVALID_ARGS'
  | 'E_UNKNOWN_FEATURE'
  | 'E_NOT_A_SKETCH'
  | 'E_UNKNOWN_ENTITY'
  | 'E_UNKNOWN_REF'
  | 'E_UNKNOWN_PARAMETER'
  | 'E_UNKNOWN_CONFIGURATION'
  | 'E_UNKNOWN_BODY'
  | 'E_INVALID_FORMULA'
  | 'E_REBUILD_FAILED'
  | 'E_NO_EFFECT'
  | 'E_STALE_REVISION'
  | 'E_EDIT_IN_PROGRESS'
  | 'E_DRAFT_MISSING'
  | 'E_NOT_ALLOWED'
  | 'E_FAILED'

export type ExplodeMotion =
  | {
    type: 'translate'
    direction: [number, number, number]
    distance: number
    /** Space components evenly: each moves `distance` further than the last. */
    auto_space?: boolean
  }
  | {
    type: 'rotate'
    origin: [number, number, number]
    axis: [number, number, number]
    angle: number
  }
  | {
    type: 'radial'
    origin: [number, number, number]
    axis: [number, number, number]
    distance: number
  }

export type FaceKind = 'face'

export type Feature =
  | {
    type: 'dome'
    face: Ref
    /**
     * Height in millimetres, positive; a spherical cap's height is at most the
     * disk radius.
     */
    height: number
    /**
     * Meet the faces beside the boundary tangentially: half an ellipsoid of
     * revolution on a disk (its height may exceed the radius), or a surface
     * tangent to the side faces on a face whose boundary has no corners.
     */
    elliptical?: boolean
    /** Remove an inward cap rather than adding an outward cap. */
    inward?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'rib'
    /** Sketch feature ID with one or more open chains of lines and arcs. */
    sketch: number
    /** Web thickness, mm. */
    thickness: number
    direction?: RibDirection
    /**
     * Taper the rib's thickness faces by this angle (radians) so the web
     * thickens away from its sketch.
     */
    draft?: number | null
    /**
     * Grow away from the part instead of toward it. By default material grows
     * toward the center of the existing part.
     */
    flip?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    side?: RibSide
  }
  | {
    type: 'wrap'
    /** Sketch feature ID with closed profiles (see above for the plane). */
    sketch: number
    /** The curved face to wrap onto. */
    face: Ref
    /**
     * Depth in mm for emboss and deboss, along the face normal (away from the
     * axis for cylinders and cones); ignored for scribe.
     */
    depth?: number
    mode?: WrapMode
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'cosmetic_thread'
    /**
     * Persistent cylindrical face reference. The face must be bounded by two
     * complete coaxial circles; trimmed or interrupted cylinders are rejected.
     */
    face: Ref
    /** Cylinder diameter, mm; must match the selected cylindrical surface. */
    diameter: number
    /** Axial distance per turn, mm. */
    pitch: number
    /** Axial annotated length, mm; contained within the selected face. */
    length: number
    /** True selects an inward-facing hole wall; false selects an external boss. */
    internal?: boolean
    left_handed?: boolean
    /** Start at the opposite rim and advance toward the first rim. */
    reverse?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /** Start phase in the cylindrical face's radial frame, radians. */
    start_angle?: number
    /** Distance from the selected end rim along the cylinder axis, mm. */
    start_offset?: number
  }
  | {
    type: 'thread'
    /** Persistent cylindrical face reference, bounded by two complete coaxial circles. */
    face: Ref
    /** Cylinder diameter, mm; must match the selected cylindrical surface. */
    diameter: number
    /** Axial distance per turn, mm. */
    pitch: number
    /** Axial threaded length, mm; contained within the selected face. */
    length: number
    /** True cuts into an inward-facing hole wall; false into an external boss. */
    internal?: boolean
    left_handed?: boolean
    /** Start at the opposite rim and advance toward the first rim. */
    reverse?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /** Start phase in the cylindrical face's radial frame, radians. */
    start_angle?: number
    /** Distance from the selected end rim along the cylinder axis, mm. */
    start_offset?: number
  }
  | {
    type: 'sketch3d'
    /** Monotonically increasing; deleted entity IDs must never be reused. */
    next_id: number
    constraints?: Constraint3[]
    entities?: Entity3[]
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'split_line'
    /**
     * A face or body; the first sketch for sketch on sketch. A split line splits
     * this face.
     */
    target: Ref
    /** Projection direction, XYZ; the pull direction for silhouettes. */
    direction: [number, number, number]
    mode?: ProjectionMode
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /**
     * A sketch or curve feature to project; the second sketch for sketch on
     * sketch; the plane, face or body that cuts the target for an intersection
     * split line. Unused for silhouettes.
     */
    source?: Ref
  }
  | {
    type: 'draft'
    /**
     * The neutral plane; with parting-line edges, only its normal (the pull
     * direction) is used.
     */
    neutral_plane: Ref
    /** Signed draft angle, radians. */
    angle: number
    direction?: Ref | null
    /** Faces to taper about the neutral plane. Leave empty for a parting-line draft. */
    faces?: Ref[]
    /** Draft the faces on the side opposite the pull direction instead. */
    other_side?: boolean
    /**
     * Parting-line edges (straight or circular). Each face next to an edge on the
     * pull side tapers about a plane through that edge.
     */
    parting_line?: Ref[]
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /**
     * Step draft: each planar face tilts about a line normal to the pull
     * direction through the parting line's nearest point, and perpendicular step
     * faces join it to inclined parts of the parting line.
     */
    step?: boolean
  }
  | {
    type: 'variable_fillet'
    /** An edge of the chain to fillet; tangent-continuous neighbours join the chain. */
    edge: Ref
    /**
     * Ordered stations; include positions 0 and 1. Positions are fractions of
     * the chain's length, measured in the selected edge's direction.
     */
    stations: RadiusStation[]
    /** Measure positions from the other end of the chain. */
    reverse?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /** Change the radius linearly between stations instead of smoothly. */
    straight?: boolean
  }
  | {
    type: 'face_fillet'
    /** Faces in one plane on the first side of the blend. */
    faces1: Ref[]
    /** Faces in one plane on the other side; not parallel to the first. */
    faces2: Ref[]
    /** Blend radius, mm. */
    radius: number
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
    /** Which two of pitch, turns and height define the helix. */
    defined_by?: HelixInput
    /** Total height, mm, for the height-based definitions. */
    height?: number
    left_handed?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /** Radians. */
    start_angle?: number
    /**
     * Pitch rows for a variable-pitch helix: the first at turn 0, the last at
     * the total turns. Pitch changes linearly in turn count between rows. When
     * present it replaces pitch, turns and height.
     */
    variable_pitch?: HelixPitch[]
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
    /**
     * A face or body; the first sketch for sketch on sketch. A split line splits
     * this face.
     */
    target: Ref
    /** Projection direction, XYZ; the pull direction for silhouettes. */
    direction: [number, number, number]
    mode?: ProjectionMode
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /**
     * A sketch or curve feature to project; the second sketch for sketch on
     * sketch; the plane, face or body that cuts the target for an intersection
     * split line. Unused for silhouettes.
     */
    source?: Ref
  }
  | {
    type: 'sweep'
    /** One connected sketch, construction curve or edge. */
    path: Ref
    /**
     * Sweep a circle of this diameter (mm), centred on the path start and normal
     * to it, instead of a sketch profile.
     */
    circle_diameter?: number | null
    cut?: boolean
    /**
     * A curve beside the path that sets the section's rotation (follow path and
     * guide). Replaces `orientation`. It scales the section only with
     * `guide_scale`.
     */
    guide?: Ref | null
    /**
     * A second guide curve. The section turns with the first guide and
     * stretches so both guides keep their places on it. Needs `guide`.
     */
    guide2?: Ref | null
    /**
     * With `guide`: also scale the section uniformly so the point where the
     * guide crosses each plane normal to the path keeps its place on the
     * section. Needs an open, tangent-continuous path.
     */
    guide_scale?: boolean
    /** Explicit closed holes inside the selected outer contour, from the same sketch. */
    inner_profiles?: ContourSelection[]
    merge?: boolean
    orientation?: SweepOrientation
    /**
     * A whole sketch with one closed profile, or one explicitly selected closed
     * contour. Omit it to sweep a circle of `circle_diameter`.
     */
    profile?: ProfileSelection | null
    reverse_path?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /**
     * Thicken the profile sketch's single open or closed chain instead of
     * sweeping a closed region. Requires a whole-sketch profile.
     */
    thin?: Thin | null
    /**
     * Turn the section by this angle (radians) from the path start to its end,
     * spread by arc length. Replaces `orientation`.
     */
    twist?: number
  }
  | {
    type: 'loft'
    /** Ordered closed sections; legacy sketch IDs or explicit contour/seam selections. */
    profiles: ProfileSelection[]
    /**
     * A curve crossing every section's plane in order; the sections blend
     * along it. Not combined with guides, end conditions or `closed`.
     */
    centerline?: Ref | null
    /**
     * Join the last section back to the first, making a ring without end caps.
     * Needs at least three sections and no guides.
     */
    closed?: boolean
    cut?: boolean
    /** How the loft arrives at the last section. */
    end?: LoftEnd
    /**
     * Guide curves crossing every section in order; the loft surface passes
     * through each guide. Use two or more; `ruled` does not apply with guides.
     */
    guides?: Ref[]
    merge?: boolean
    /** Profiles whose direction is reversed. */
    reversed?: number[]
    ruled?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /** How the loft leaves the first section. */
    start?: LoftEnd
  }
  | {
    type: 'boundary'
    /** Ordered closed sections. The first and last must be planar; they cap the solid. */
    sections: ProfileSelection[]
    /** Curves that cross every section in order (sketch, 3D sketch, edge or curve paths). */
    curves: Ref[]
    cut?: boolean
    merge?: boolean
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
    keep?: KeepSide
    /** A plane or planar face. */
    plane?: Ref
    /**
     * Stable piece IDs from an all-pieces preview. None keeps every piece.
     * Missing or ambiguous IDs fail instead of selecting another piece.
     */
    retained_pieces?: string[] | null
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /**
     * General split mode: planes, bounded faces or whole tool bodies. Empty
     * preserves the legacy one-piece-per-plane-side behavior above.
     */
    tools?: Ref[]
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
    /**
     * Show a cosmetic thread in each tapped hole: nominal diameter and coarse
     * pitch from the size table, through the material the hole drills.
     */
    cosmetic_thread?: boolean
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
    /**
     * Whole-body mirrors only: false replaces the body with its mirror
     * image (opposite-hand parts) instead of adding it.
     */
    keep_original?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }

/** Kinds of standard hardware. */
export type HardwareKind =
  | 'socket_head_cap_screw'
  | 'socket_head_cap_screw_inch'
  | 'hex_bolt'
  | 'hex_screw'
  | 'hex_nut'
  | 'washer'
  | 'dowel_pin'

/**
 * What to generate: a kind, a size from its table and, for screws and
 * pins, a length in mm.
 */
export type HardwareSpec = {
  kind: HardwareKind
  /** Size designation from the catalog, such as `M6` or `1/4-20`. */
  size: string
  /** Length under the head (screws) or overall (pins), mm. */
  length?: number | null
}

export type HelixInput = 'pitch_and_turns' | 'height_and_turns' | 'height_and_pitch'

export type HelixPitch = {
  /** Turn count at which this pitch applies. */
  turn: number
  /** mm per turn. */
  pitch: number
}

export type HingeLimit = {
  faces: [CompRef, CompRef]
  min: number
  max: number
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

/** A range for limit mates, in mm or radians. */
export type Limit = {
  min: number
  max: number
}

/**
 * A loft end condition. The surface leaves (or meets) the end section along
 * the given tangent; `tangent_length` scales it relative to the spacing of the
 * sections (1 is a natural length; larger values bulge further).
 */
export type LoftEnd =
  | {
    type: 'none'
  }
  | {
    type: 'normal'
    tangent_length?: number
  }
  | {
    type: 'direction'
    direction: Ref
    flip?: boolean
    tangent_length?: number
  }

/** The mate types: standard, advanced and mechanical. */
export type MateKind =
  | {
    type: 'coincident'
    a: CompRef
    b: CompRef
    alignment?: Alignment
  }
  | {
    type: 'parallel'
    a: CompRef
    b: CompRef
    alignment?: Alignment
  }
  | {
    type: 'perpendicular'
    a: CompRef
    b: CompRef
  }
  | {
    type: 'tangent'
    a: CompRef
    b: CompRef
    alignment?: Alignment
  }
  | {
    type: 'concentric'
    a: CompRef
    b: CompRef
    alignment?: Alignment
    /** Also stop rotation about the shared axis. */
    lock_rotation?: boolean
  }
  | {
    type: 'lock'
    a: string[]
    b: string[]
  }
  | {
    type: 'distance'
    a: CompRef
    b: CompRef
    distance: number
    flip?: boolean
    limit?: Limit | null
  }
  | {
    type: 'angle'
    a: CompRef
    b: CompRef
    angle: number
    flip?: boolean
    limit?: Limit | null
  }
  | {
    type: 'profile_center'
    a: CompRef
    b: CompRef
    flip?: boolean
    lock_rotation?: boolean
    offset?: number
  }
  | {
    type: 'symmetric'
    a: CompRef
    b: CompRef
    plane: CompRef
  }
  | {
    type: 'width'
    width: [CompRef, CompRef]
    tab: [CompRef, CompRef]
    constraint: WidthConstraint
  }
  | {
    type: 'path'
    point: CompRef
    path: CompRef[]
    constraint: TravelConstraint
    /** Keep this axis of the moving component tangent to the path. */
    follow?: CompRef | null
    /** Roll control: keep this axis of the moving component toward `up`. */
    roll?: Roll | null
  }
  | {
    type: 'linear_coupler'
    a: CompRef
    b: CompRef
    /** When `a` moves `ratio_a`, `b` moves `ratio_b`. */
    ratio_a: number
    ratio_b: number
    reverse?: boolean
  }
  | {
    type: 'cam'
    cam: CompRef
    follower: CompRef
  }
  | {
    type: 'slot'
    slot: CompRef
    pin: CompRef
    constraint: TravelConstraint
  }
  | {
    type: 'hinge'
    axis: [CompRef, CompRef]
    faces: [CompRef, CompRef]
    limit?: HingeLimit | null
  }
  | {
    type: 'gear'
    a: CompRef
    b: CompRef
    ratio_a: number
    ratio_b: number
    reverse?: boolean
  }
  | {
    type: 'rack_pinion'
    rack: CompRef
    pinion: CompRef
    distance_per_revolution: number
    reverse?: boolean
  }
  | {
    type: 'screw'
    a: CompRef
    b: CompRef
    distance_per_revolution: number
    reverse?: boolean
  }
  | {
    type: 'universal_joint'
    a: CompRef
    b: CompRef
    joint_point?: CompRef | null
  }

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

export type Measurements = {
  volume_mm3: number
  body_count: number
  /** Bounding box `[min, max]`, mm. */
  bbox_mm?: [[number, number, number], [number, number, number]] | null
}

export type MirrorItem = {
  component: string
  /**
   * Write an opposite-hand part here (relative to the assembly) instead
   * of placing a reoriented copy of the same part.
   */
  opposite_hand?: string | null
}

export type MirrorPosition = 'center_of_mass' | 'bounding_box' | 'origin'

/** How to move a component. */
export type Motion =
  | {
    mode: 'delta'
    translation: [number, number, number]
  }
  | {
    mode: 'to'
    point: [number, number, number]
    local?: [number, number, number]
  }
  | {
    mode: 'along'
    reference: CompRef
    distance: number
  }
  | {
    mode: 'rotate'
    angles: [number, number, number]
  }
  | {
    mode: 'about'
    reference: CompRef
    angle: number
  }
  | {
    mode: 'pose'
    transform: Transform
  }
  | {
    mode: 'drag'
    local: [number, number, number]
    point: [number, number, number]
  }

/** A finite number that compares and hashes by value. */
export type OrderedFloat = number

export type PatternDirection = {
  /** A straight edge, sketch line, axis, or plane/face (its normal). */
  reference: Ref
  /** Distance between instances, mm. */
  spacing: number
  /** Total instances including the original. */
  count: number
  reverse?: boolean
}

export type PatternKind =
  | {
    type: 'linear'
    direction1: ComponentPatternDirection
    direction2?: ComponentPatternDirection | null
  }
  | {
    type: 'circular'
    axis: CompRef
    /** Angle between instances, or the total angle with `equal_spacing`. */
    angle: number
    count: number
    equal_spacing?: boolean
    reverse?: boolean
  }
  | {
    type: 'pattern_driven'
    /** The pattern feature: a component path and `{type: feature, id}`. */
    feature: CompRef
  }
  | {
    type: 'sketch_driven'
    sketch: CompRef
    /** The seed point the instances are placed by; default the seed's origin. */
    reference?: CompRef | null
  }
  | {
    type: 'curve_driven'
    curve: CompRef[]
    spacing: number
    count: number
    reverse?: boolean
    /** Rotate instances to follow the curve tangent. */
    tangent?: boolean
  }

/** A point given by sketch entity ID, or a new point at a position. */
export type PointArg =
  | {
    id: number
  }
  | {
    at: [number, number]
  }

/** Legacy sketch IDs retain their original whole-sketch behavior. */
export type ProfileSelection = number | ContourSelection

export type ProjectionMode = 'onto_face' | 'sketch_on_sketch' | 'intersection' | 'silhouette'

export type Property = {
  value: string
  kind?: PropertyKind
}

export type PropertyKind = 'text' | 'number' | 'date' | 'yes_no'

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
  | {
    type: 'curve_segment'
    curve: number
    segment: string
  }
  | {
    type: 'ordered_sketch_contour'
    sketch: number
    entities: number[]
    reverse?: boolean
    /**
     * Put the seam inside the start entity, this fraction along it in the
     * traversal direction (0 < seam < 1): by length for lines, arcs and
     * circles, by curve parameter for Bezier curves. The entity is split
     * there, except a circle, which just starts there. Not for splines.
     */
    seam?: OrderedFloat | null
    /** Entity at the start/seam; must be a member of the selected contour. */
    start?: number | null
  }
  | {
    type: 'sketch_contour'
    sketch: number
    entities: number[]
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

/** How a rib's material grows from its sketch to the surrounding material. */
export type RibDirection = 'parallel_to_sketch' | 'normal_to_sketch'

/** Which side of the sketch a rib's thickness grows to. */
export type RibSide = 'both' | 'first' | 'second'

export type Roll = {
  axis: CompRef
  up: CompRef
}

export type Sketch3D = {
  /** Monotonically increasing; deleted entity IDs must never be reused. */
  next_id: number
  constraints?: Constraint3[]
  entities?: Entity3[]
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

export type SpatialDrag = {
  point: number
  to: [number, number, number]
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

export type ToolError = {
  code: ErrorCode
  message: string
  repair: string
  /** Values allowed at `path`. */
  allowed?: string[] | null
  /** Feature the error is about. */
  feature?: number | null
  /** JSON path of the offending argument. */
  path?: string | null
}

/** A rigid placement: translation in mm and a unit quaternion `[x, y, z, w]`. */
export type Transform = {
  translation: [number, number, number]
  rotation: [number, number, number, number]
}

export type TravelConstraint =
  | {
    type: 'free'
  }
  | {
    type: 'center'
  }
  | {
    type: 'distance'
    distance: number
    flip?: boolean
  }
  | {
    type: 'percent'
    percent: number
    flip?: boolean
  }

export type Warning = {
  code: WarningCode
  message: string
  feature: number
}

export type WarningCode = 'W_SKETCH_UNDER_DEFINED' | 'W_SKETCH_OVER_DEFINED'

export type WidthConstraint =
  | {
    type: 'centered'
  }
  | {
    type: 'free'
  }
  | {
    type: 'dimension'
    distance: number
    flip?: boolean
  }
  | {
    type: 'percent'
    percent: number
  }

/** What a wrapped sketch does to the face it is wrapped onto. */
export type WrapMode = 'emboss' | 'deboss' | 'scribe'

/** The result of every tool call (MCP `structuredContent`, `draft.apply`). */
export type Envelope = {
  /**
   * True only if the call succeeded, introduced no feature error and every
   * added or changed material feature changed the part.
   */
  ok: boolean
  /** Document revision after the call. */
  revision: number
  /** Document identity; it changes on new/open and core restart. */
  generation: string
  /** Features this call added, changed or removed. */
  changed: Changed[]
  created: Created
  /** Feature IDs removed by this call. */
  deleted: number[]
  /** Feature errors this call introduced or left on a feature it changed. */
  feature_errors: Record<string, string>
  /** Errors the part already had before the call, unchanged by it. */
  existing_errors: Record<string, string>
  warnings: Warning[]
  /** Suggested next steps. */
  next: string[]
  /** The tool's own result, trimmed unless `detail` is `full`. */
  result: unknown
  /** Assistant draft the call ran in. */
  draft?: string | null
  error?: ToolError | null
  /** The active configuration after a change. */
  measurements?: Measurements | null
}

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
  /** Solve or drag a supplied spatial sketch without changing the document. Returns sketch and solver DOF/conflict report. Commit the returned sketch with add_feature/update_feature type sketch3d. Failed solves preserve the supplied geometry. */
  solve_spatial_sketch: {
    sketch: Sketch3D
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    drag?: SpatialDrag | null
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
    /**
     * Parameter path within this feature (such as `end1.depth`) to a formula,
     * stored as an equation on the new feature; null removes one.
     */
    formulas?: Record<string, string | null>
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
    /**
     * Parameter path within this feature to a formula; null removes that
     * parameter's equation. Paths left out keep their equations.
     */
    formulas?: Record<string, string | null>
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
  /** Replace shared base variables and equations, or the selected configuration's overrides. Optional parameters atomically set numeric values; omitted configuration edits the base, and removing variant entries resets them to base. Formulas use + - * /, min, max, abs, sqrt, sin, cos, tan, pi and units mm, cm, m, in, ft, deg, rad; lengths need units. Check part_info `equation_values` and feature errors afterwards. */
  set_equations: {
    /** Every global variable: name to formula, such as `"8mm"` or `"width / 2"`. */
    variables: Record<string, string>
    /** Parameter key to formula; null disables the inherited equation in a variant. */
    equations: Record<string, string | null>
    /**
     * Omitted edits shared base values; otherwise replaces this configuration's
     * variable/equation/numeric overrides. Removing an override restores the base.
     */
    configuration?: number | null
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /**
     * Numeric parameter values in mm/radians. Base edits update only these keys;
     * configuration edits replace that configuration's numeric override map.
     */
    parameters?: Record<string, number>
    /** Reject a stale preview or commit if the document changed while editing. */
    revision?: number | null
  }
  /** Where a reference is now: `point` for a vertex or reference point, `plane` (origin and normal) for a plane or planar face. Values are mm. */
  locate_reference: {
    /** A vertex, reference point, plane or planar face. */
    reference: Ref
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Evaluate a formula with the part's variables and parameters (active configuration) without changing anything. Returns `value` with its value and unit powers. */
  evaluate_formula: {
    /** A formula such as `width / 2 + 1mm`. */
    expression: string
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
    /** Reject an edit based on an older document revision. */
    revision?: number | null
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
  /** Replace the open document with an empty assembly. Insert parts with insert_component; the first one is fixed at the origin. */
  new_assembly: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** The open assembly: components (keys, files, fixed, remaining DOF, errors, children), mates (IDs, types, errors), patterns, mirrors, configurations, explode steps and solve diagnostics. */
  assembly_info: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Faces and edges of one component in assembly coordinates, each with the component `ref` that add_mate and measure take. */
  component_topology: {
    /** Component key path from `assembly_info` `components`, such as `["2"]`. */
    component: string[]
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Insert an .ofp part or .ofa subassembly. Returns its key as `key`. Transform rotation is a unit quaternion [x, y, z, w]. */
  insert_component: {
    /** `.ofp` part or `.ofa` assembly file. */
    path: string
    /** Part configuration ID; omitted uses the file's active configuration. */
    configuration?: number | null
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /** Placement; omitted puts the component at the origin. */
    transform?: Transform | null
  }
  /** Standard hardware kinds with their sizes and preferred lengths (mm). */
  hardware_catalog: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Generate (or reuse) a standard screw, bolt, nut, washer or pin from hardware_catalog and insert it. Its axis is Z, its bearing face on Z = 0, the head toward +Z. */
  insert_hardware: {
    hardware: HardwareSpec
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /**
     * Library folder; omitted uses `OPENFORGE_HARDWARE_DIR` or
     * `~/OpenForge/Hardware`.
     */
    folder?: string | null
    transform?: Transform | null
  }
  /** Rename, fix or float, hide, suppress, exclude from BOM, make a subassembly flexible, pick a part configuration or set the transform of a top-level component. */
  update_component: {
    /** Top-level component key. */
    key: string
    /**
     * Part configuration ID in the active assembly configuration; null
     * returns to the file's active configuration.
     */
    configuration?: number | null
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    exclude_from_bom?: boolean | null
    fixed?: boolean | null
    /** Subassemblies: solve their components with this assembly's mates. */
    flexible?: boolean | null
    hidden?: boolean | null
    name?: string | null
    /** Suppression in the active assembly configuration. */
    suppressed?: boolean | null
    transform?: Transform | null
  }
  /** Delete components and every mate that uses them. Returns `deleted_mates`. */
  delete_components: {
    /** Top-level component keys from `assembly_info` `components`. */
    keys: string[]
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Add a mate between component references from component_topology and solve. Fails without changing anything if the mate can't be satisfied, naming the mates it conflicts with. Lengths mm, angles radians. */
  add_mate: {
    mate: MateKind
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    name?: string | null
  }
  /** Change a mate's definition, name or suppression by ID. */
  update_mate: {
    /** Mate ID from `assembly_info` `mates`. */
    id: number
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    mate?: MateKind | null
    name?: string | null
    suppressed?: boolean | null
  }
  /** Delete mates by ID. */
  delete_mates: {
    ids: number[]
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Move or rotate a floating component within its mates: by a delta, to a point, along or about a reference. With `collision` it stops before parts overlap. */
  move_component: {
    /** Component key path: `["2"]`, or a flexible subassembly's child `["4", "1"]`. */
    component: string[]
    motion: Motion
    /** Stop before parts overlap. */
    collision?: Collision | null
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /**
     * Interactive drags: `start` snapshots for undo, `move` previews and
     * returns only positions, `end` (the default) records the change.
     */
    phase?: DragPhase | null
  }
  /** Pattern components: linear, circular, following a part's pattern feature, at sketch points or along a curve. Instances get keys like `2@p5.1`. */
  add_component_pattern: {
    /** Linear, circular, pattern-driven, sketch-driven or curve-driven. */
    pattern: PatternKind
    /** Seed component keys. */
    seeds: string[]
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    name?: string | null
    skip?: number[]
  }
  /** Mirror components about a plane: reoriented copies, or opposite-hand part files written next to the assembly. */
  mirror_components: {
    /** Plane or flat face to mirror about. */
    plane: CompRef
    components: MirrorItem[]
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    name?: string | null
    position?: MirrorPosition
  }
  /** Add an explode step to the active configuration and show the exploded view. Explode steps move only the display, never mates. */
  add_explode_step: {
    /** Component key paths to move. */
    components: string[][]
    /** Translate along a direction, rotate about an axis, or spread radially. */
    motion: ExplodeMotion
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    name?: string | null
  }
  /** Find components whose solids overlap, with each overlap's volume (mm³). */
  interference: {
    /** Check only among these component key paths; empty checks everything. */
    components?: string[][]
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    include_hidden?: boolean
    subassemblies_as_components?: boolean
    treat_coincidence?: boolean
  }
  /** List gaps between components smaller than `minimum` mm, smallest first. */
  clearance: {
    /** Report gaps smaller than this, mm. */
    minimum: number
    components?: string[][]
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** The bill of materials: top-level, parts-only or indented rows with quantities and part properties. `csv` also writes it to a file. */
  bill_of_materials: {
    /** Also write the BOM as CSV here. */
    csv?: string | null
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    type?: BomType
  }
  /** Read a STEP file with several parts into a new assembly: one .ofp per distinct part in `folder`, each instance placed and fixed where the file had it. */
  import_step_assembly: {
    /** `.step` or `.stp` file with one or more parts. */
    path: string
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /**
     * Folder for the generated part files; omitted uses a folder named
     * after the STEP file beside it.
     */
    folder?: string | null
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
  solve_spatial_sketch: 'sketch3d.solve',
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
  locate_reference: 'ref.locate',
  evaluate_formula: 'equations.evaluate',
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
  new_assembly: 'asm.new',
  assembly_info: 'asm.info',
  component_topology: 'asm.topology',
  insert_component: 'asm.insert',
  hardware_catalog: 'asm.hardware_catalog',
  insert_hardware: 'asm.insert_hardware',
  update_component: 'asm.component.update',
  delete_components: 'asm.component.delete',
  add_mate: 'asm.mate.add',
  update_mate: 'asm.mate.update',
  delete_mates: 'asm.mate.delete',
  move_component: 'asm.move',
  add_component_pattern: 'asm.pattern.add',
  mirror_components: 'asm.mirror.add',
  add_explode_step: 'asm.explode.add',
  interference: 'asm.interference',
  clearance: 'asm.clearance',
  bill_of_materials: 'asm.bom',
  import_step_assembly: 'asm.import_step',
} as const satisfies Record<ToolName, string>

/** Schema examples, validated and replayed on the core in CI. */
export const TOOL_EXAMPLES: { [K in ToolName]: ToolArgs[K][] } = {
  core_info: [{}],
  part_info: [{}],
  get_feature: [{"id":2}],
  topology: [{}],
  get_sketch: [{"id":3}],
  solve_spatial_sketch: [{"sketch":{"constraints":[],"entities":[],"next_id":1}}],
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
  locate_reference: [{"reference":{"type":"front_plane"}}],
  evaluate_formula: [{"expression":"f2.end1.depth * 2"}],
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
  new_assembly: [{}],
  assembly_info: [{}],
  component_topology: [{"component":["2"]}],
  insert_component: [{"path":"part.ofp","transform":{"rotation":[0,0,0,1],"translation":[0,0,40]}}],
  hardware_catalog: [{}],
  insert_hardware: [{"hardware":{"kind":"hex_nut","size":"M6"}}, {"hardware":{"kind":"socket_head_cap_screw","length":16,"size":"M5"},"transform":{"rotation":[0,0,0,1],"translation":[30,20,5]}}],
  update_component: [{"key":"2","name":"Lid"}, {"fixed":true,"key":"2"}],
  delete_components: [{"keys":["2"]}],
  add_mate: [{"mate":{"a":{"component":["1"],"entity":{"name":"2.end","type":"face"}},"b":{"component":["2"],"entity":{"name":"2.start","type":"face"}},"type":"coincident"}}, {"mate":{"a":{"component":[],"entity":{"type":"right_plane"}},"b":{"component":["2"],"entity":{"type":"right_plane"}},"distance":10,"type":"distance"},"name":"Offset"}],
  update_mate: [{"id":3,"name":"Seat"}, {"id":3,"suppressed":true}],
  delete_mates: [{"ids":[3]}],
  move_component: [{"component":["2"],"motion":{"mode":"delta","translation":[10,0,0]}}, {"collision":{},"component":["2"],"motion":{"angles":[0,0,0.5],"mode":"rotate"}}],
  add_component_pattern: [{"pattern":{"direction1":{"count":3,"reference":{"component":[],"entity":{"type":"right_plane"}},"spacing":70},"type":"linear"},"seeds":["2"]}],
  mirror_components: [{"components":[{"component":"2"}],"plane":{"component":[],"entity":{"type":"right_plane"}}}],
  add_explode_step: [{"components":[["2"]],"motion":{"direction":[0,0,1],"distance":30,"type":"translate"}}],
  interference: [{}, {"treat_coincidence":true}],
  clearance: [{"minimum":50}],
  bill_of_materials: [{"type":"parts_only"}],
  import_step_assembly: [{"path":"assembly.step"}],
}

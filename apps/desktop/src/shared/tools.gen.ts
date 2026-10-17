// Generated from the core tool catalog (crates/openforge/src/tools). Do not edit:
// run `cargo run -p openforge -- generate` instead.
// Lengths are millimetres and angles radians.

/** Direction sense for mates between oriented entities. */
export type Alignment = 'aligned' | 'anti_aligned' | 'closest'

/** How the flat length of a bend is found. */
export type Allowance =
  | {
    type: 'k_factor'
    k: number
  }
  | {
    type: 'bend_allowance'
    value: number
  }
  | {
    type: 'bend_deduction'
    value: number
  }
  | {
    type: 'bend_table'
    table: string
  }
  | {
    type: 'bend_calculation'
    ranges: CalculationRange[]
  }

/** Where an annotation is attached. */
export type Anchor =
  | {
    type: 'sheet'
    at: [number, number]
  }
  | {
    type: 'model'
    target: ViewRef
    at: [number, number]
  }

/** A face that sets an edge flange's angle. */
export type AngleFrom = {
  /** A planar face. */
  face: Ref
  /** Run parallel to the face; square to it otherwise. */
  parallel?: boolean
}

export type AnnotationKind =
  | {
    type: 'note'
    /** Where the text's bottom-left corner sits. */
    at: [number, number]
    text: string
    /** Radians, counter-clockwise. */
    angle?: number
    border?: Border
    height?: number | null
    /** Leader jog (a horizontal shoulder at the text). */
    jog?: boolean
    leader?: Anchor | null
  }
  | {
    type: 'dimension'
    dim: 'linear'
    /**
     * Where the value text goes, sheet mm: relative to the annotation's
     * view when it has one; for imported model dimensions, relative to the
     * middle of what they measure (so they follow model changes). The
     * dimension line passes through it.
     */
    at: [number, number]
    a: Pick
    arrows?: Arrows
    b?: Pick | null
    /** A second line of text under the value. */
    below?: string
    /** Also show the value in inches; the standard decides when unset. */
    dual?: boolean | null
    /** A radius whose center is off the sheet: jogged dimension line. */
    foreshortened?: boolean
    /** An inspection dimension: value in a rounded frame. */
    inspection?: boolean
    orientation?: Orient
    /** A reference dimension: value in parentheses. */
    parentheses?: boolean
    /** Decimal places of the value; the standard's when unset. */
    precision?: number | null
    prefix?: string
    suffix?: string
    /** Replaces the value; `<DIM>` stands for it. */
    text?: string | null
    tolerance?: Tolerance
    tolerance_precision?: number | null
  }
  | {
    type: 'dimension'
    dim: 'angular'
    /**
     * Where the value text goes, sheet mm: relative to the annotation's
     * view when it has one; for imported model dimensions, relative to the
     * middle of what they measure (so they follow model changes). The
     * dimension line passes through it.
     */
    at: [number, number]
    a: Pick
    b: Pick
    arrows?: Arrows
    /** A second line of text under the value. */
    below?: string
    /** Also show the value in inches; the standard decides when unset. */
    dual?: boolean | null
    /** A radius whose center is off the sheet: jogged dimension line. */
    foreshortened?: boolean
    /** An inspection dimension: value in a rounded frame. */
    inspection?: boolean
    /** A reference dimension: value in parentheses. */
    parentheses?: boolean
    /** Decimal places of the value; the standard's when unset. */
    precision?: number | null
    prefix?: string
    suffix?: string
    /** Replaces the value; `<DIM>` stands for it. */
    text?: string | null
    tolerance?: Tolerance
    tolerance_precision?: number | null
  }
  | {
    type: 'dimension'
    dim: 'radius'
    /**
     * Where the value text goes, sheet mm: relative to the annotation's
     * view when it has one; for imported model dimensions, relative to the
     * middle of what they measure (so they follow model changes). The
     * dimension line passes through it.
     */
    at: [number, number]
    target: Pick
    arrows?: Arrows
    /** A second line of text under the value. */
    below?: string
    /** Also show the value in inches; the standard decides when unset. */
    dual?: boolean | null
    /** A radius whose center is off the sheet: jogged dimension line. */
    foreshortened?: boolean
    /** An inspection dimension: value in a rounded frame. */
    inspection?: boolean
    /** A reference dimension: value in parentheses. */
    parentheses?: boolean
    /** Decimal places of the value; the standard's when unset. */
    precision?: number | null
    prefix?: string
    suffix?: string
    /** Replaces the value; `<DIM>` stands for it. */
    text?: string | null
    tolerance?: Tolerance
    tolerance_precision?: number | null
  }
  | {
    type: 'dimension'
    dim: 'diameter'
    /**
     * Where the value text goes, sheet mm: relative to the annotation's
     * view when it has one; for imported model dimensions, relative to the
     * middle of what they measure (so they follow model changes). The
     * dimension line passes through it.
     */
    at: [number, number]
    target: Pick
    arrows?: Arrows
    /** A second line of text under the value. */
    below?: string
    /** Also show the value in inches; the standard decides when unset. */
    dual?: boolean | null
    /** A radius whose center is off the sheet: jogged dimension line. */
    foreshortened?: boolean
    /** An inspection dimension: value in a rounded frame. */
    inspection?: boolean
    /** A reference dimension: value in parentheses. */
    parentheses?: boolean
    /** Decimal places of the value; the standard's when unset. */
    precision?: number | null
    prefix?: string
    suffix?: string
    /** Replaces the value; `<DIM>` stands for it. */
    text?: string | null
    tolerance?: Tolerance
    tolerance_precision?: number | null
  }
  | {
    type: 'dimension'
    dim: 'arc_length'
    /**
     * Where the value text goes, sheet mm: relative to the annotation's
     * view when it has one; for imported model dimensions, relative to the
     * middle of what they measure (so they follow model changes). The
     * dimension line passes through it.
     */
    at: [number, number]
    target: Pick
    arrows?: Arrows
    /** A second line of text under the value. */
    below?: string
    /** Also show the value in inches; the standard decides when unset. */
    dual?: boolean | null
    /** A radius whose center is off the sheet: jogged dimension line. */
    foreshortened?: boolean
    /** An inspection dimension: value in a rounded frame. */
    inspection?: boolean
    /** A reference dimension: value in parentheses. */
    parentheses?: boolean
    /** Decimal places of the value; the standard's when unset. */
    precision?: number | null
    prefix?: string
    suffix?: string
    /** Replaces the value; `<DIM>` stands for it. */
    text?: string | null
    tolerance?: Tolerance
    tolerance_precision?: number | null
  }
  | {
    type: 'dimension'
    dim: 'chamfer'
    /**
     * Where the value text goes, sheet mm: relative to the annotation's
     * view when it has one; for imported model dimensions, relative to the
     * middle of what they measure (so they follow model changes). The
     * dimension line passes through it.
     */
    at: [number, number]
    edge: Pick
    adjacent: Pick
    arrows?: Arrows
    /** A second line of text under the value. */
    below?: string
    /** Also show the value in inches; the standard decides when unset. */
    dual?: boolean | null
    /** A radius whose center is off the sheet: jogged dimension line. */
    foreshortened?: boolean
    /** An inspection dimension: value in a rounded frame. */
    inspection?: boolean
    /** A reference dimension: value in parentheses. */
    parentheses?: boolean
    /** Decimal places of the value; the standard's when unset. */
    precision?: number | null
    prefix?: string
    suffix?: string
    /** Replaces the value; `<DIM>` stands for it. */
    text?: string | null
    tolerance?: Tolerance
    tolerance_precision?: number | null
  }
  | {
    type: 'dimension'
    dim: 'ordinate'
    /**
     * Where the value text goes, sheet mm: relative to the annotation's
     * view when it has one; for imported model dimensions, relative to the
     * middle of what they measure (so they follow model changes). The
     * dimension line passes through it.
     */
    at: [number, number]
    origin: Pick
    points: Pick[]
    arrows?: Arrows
    /** A second line of text under the value. */
    below?: string
    /** Also show the value in inches; the standard decides when unset. */
    dual?: boolean | null
    /** A radius whose center is off the sheet: jogged dimension line. */
    foreshortened?: boolean
    /** An inspection dimension: value in a rounded frame. */
    inspection?: boolean
    /** A reference dimension: value in parentheses. */
    parentheses?: boolean
    /** Decimal places of the value; the standard's when unset. */
    precision?: number | null
    prefix?: string
    suffix?: string
    /** Replaces the value; `<DIM>` stands for it. */
    text?: string | null
    tolerance?: Tolerance
    tolerance_precision?: number | null
    vertical?: boolean
  }
  | {
    type: 'dimension'
    dim: 'baseline'
    /**
     * Where the value text goes, sheet mm: relative to the annotation's
     * view when it has one; for imported model dimensions, relative to the
     * middle of what they measure (so they follow model changes). The
     * dimension line passes through it.
     */
    at: [number, number]
    base: Pick
    points: Pick[]
    arrows?: Arrows
    /** A second line of text under the value. */
    below?: string
    /** Also show the value in inches; the standard decides when unset. */
    dual?: boolean | null
    /** A radius whose center is off the sheet: jogged dimension line. */
    foreshortened?: boolean
    /** An inspection dimension: value in a rounded frame. */
    inspection?: boolean
    orientation?: Orient
    /** A reference dimension: value in parentheses. */
    parentheses?: boolean
    /** Decimal places of the value; the standard's when unset. */
    precision?: number | null
    prefix?: string
    spacing?: number
    suffix?: string
    /** Replaces the value; `<DIM>` stands for it. */
    text?: string | null
    tolerance?: Tolerance
    tolerance_precision?: number | null
  }
  | {
    type: 'dimension'
    dim: 'chain'
    /**
     * Where the value text goes, sheet mm: relative to the annotation's
     * view when it has one; for imported model dimensions, relative to the
     * middle of what they measure (so they follow model changes). The
     * dimension line passes through it.
     */
    at: [number, number]
    points: Pick[]
    arrows?: Arrows
    /** A second line of text under the value. */
    below?: string
    /** Also show the value in inches; the standard decides when unset. */
    dual?: boolean | null
    /** A radius whose center is off the sheet: jogged dimension line. */
    foreshortened?: boolean
    /** An inspection dimension: value in a rounded frame. */
    inspection?: boolean
    orientation?: Orient
    /** A reference dimension: value in parentheses. */
    parentheses?: boolean
    /** Decimal places of the value; the standard's when unset. */
    precision?: number | null
    prefix?: string
    suffix?: string
    /** Replaces the value; `<DIM>` stands for it. */
    text?: string | null
    tolerance?: Tolerance
    tolerance_precision?: number | null
  }
  | {
    type: 'dimension'
    dim: 'angular_running'
    /**
     * Where the value text goes, sheet mm: relative to the annotation's
     * view when it has one; for imported model dimensions, relative to the
     * middle of what they measure (so they follow model changes). The
     * dimension line passes through it.
     */
    at: [number, number]
    base: Pick
    lines: Pick[]
    arrows?: Arrows
    /** A second line of text under the value. */
    below?: string
    /** Also show the value in inches; the standard decides when unset. */
    dual?: boolean | null
    /** A radius whose center is off the sheet: jogged dimension line. */
    foreshortened?: boolean
    /** An inspection dimension: value in a rounded frame. */
    inspection?: boolean
    /** A reference dimension: value in parentheses. */
    parentheses?: boolean
    /** Decimal places of the value; the standard's when unset. */
    precision?: number | null
    prefix?: string
    suffix?: string
    /** Replaces the value; `<DIM>` stands for it. */
    text?: string | null
    tolerance?: Tolerance
    tolerance_precision?: number | null
  }
  | {
    type: 'dimension'
    dim: 'model'
    /**
     * Where the value text goes, sheet mm: relative to the annotation's
     * view when it has one; for imported model dimensions, relative to the
     * middle of what they measure (so they follow model changes). The
     * dimension line passes through it.
     */
    at: [number, number]
    view: number
    key: string
    arrows?: Arrows
    /** A second line of text under the value. */
    below?: string
    /** Also show the value in inches; the standard decides when unset. */
    dual?: boolean | null
    /** A radius whose center is off the sheet: jogged dimension line. */
    foreshortened?: boolean
    /** An inspection dimension: value in a rounded frame. */
    inspection?: boolean
    /** A reference dimension: value in parentheses. */
    parentheses?: boolean
    /** Decimal places of the value; the standard's when unset. */
    precision?: number | null
    prefix?: string
    suffix?: string
    /** Replaces the value; `<DIM>` stands for it. */
    text?: string | null
    tolerance?: Tolerance
    tolerance_precision?: number | null
  }
  | {
    type: 'balloon'
    at: [number, number]
    leader: Anchor
    content: BalloonText
    /** Second line of a split circle (quantity by default). */
    lower?: BalloonText | null
    shape?: BalloonShape
    /** Diameter (or side), mm; default from the standard. */
    size?: number | null
    /** Further balloons stacked beside this one, one leader for all. */
    stacked?: BalloonText[]
    /**
     * BOM table the item numbers come from; the first on the sheet when
     * unset.
     */
    table?: number | null
  }
  | {
    type: 'hole_callout'
    at: [number, number]
    /** A circular edge of the hole. */
    target: ViewRef
    /** Replaces the generated text. */
    text?: string | null
  }
  | {
    type: 'center_mark'
    targets: ViewRef[]
    /** Extend the lines past the circle. */
    extended?: boolean
    pattern?: MarkPattern
    /** Mark size, mm of paper; default from the standard. */
    size?: number | null
  }
  | {
    type: 'centerline'
    a: ViewRef
    b?: ViewRef | null
    /** Overshoot past the geometry, mm of paper. */
    extend?: number
  }
  | {
    type: 'surface_finish'
    at: [number, number]
    /** The same requirement all around the part outline. */
    all_around?: boolean
    angle?: number
    /** Lay symbol: `=`, `⊥`, `X`, `M`, `C`, `R`, `P`. */
    lay?: string
    leader?: Anchor | null
    /** Production method (above the long leg). */
    method?: string
    process?: FinishProcess
    /** Roughness requirement text, e.g. `Ra 1.6`. */
    roughness?: string
  }
  | {
    type: 'datum_feature'
    at: [number, number]
    label: string
    target: Anchor
  }
  | {
    type: 'datum_target'
    at: [number, number]
    label: string
    target: Anchor
    /** Point (X), line or area target drawn at the anchor. */
    kind?: TargetKind
    size?: string
  }
  | {
    type: 'gtol'
    at: [number, number]
    rows: GtolRow[]
    /** Text above the frame, e.g. `6X`. */
    above?: string
    leader?: Anchor | null
  }
  | {
    type: 'weld'
    at: [number, number]
    leader: Anchor
    all_around?: boolean
    arrow?: WeldSide | null
    field?: boolean
    other?: WeldSide | null
    /** Tail text (process, specification). */
    tail?: string
  }
  | {
    type: 'area_hatch'
    boundary: ([number, number])[]
    angle?: number
    pattern?: Pattern
    scale?: number
  }
  | {
    type: 'revision_cloud'
    shape: CloudShape
    /** Arc size, mm of paper. */
    arc?: number
    /** Revision letter shown in a triangle beside it. */
    revision?: string | null
  }
  | {
    type: 'revision_symbol'
    at: [number, number]
    revision: string
    leader?: Anchor | null
  }
  | {
    type: 'table'
    table: 'bom'
    /** Sheet position of the `anchor` corner, mm. */
    at: [number, number]
    view: number
    anchor?: Corner
    bom?: BomType
    columns?: Column[]
    continued?: Piece[]
    increment?: number
    /** Rows per piece; the rest continue at `continued`. */
    max_rows?: number | null
    numbering?: Numbering
    start?: number
    /** Capital height of cell text; the standard's when unset. */
    text_height?: number | null
  }
  | {
    type: 'table'
    table: 'hole'
    /** Sheet position of the `anchor` corner, mm. */
    at: [number, number]
    origin: Pick
    holes: ViewRef[]
    anchor?: Corner
    /** Combine holes of the same size into one row each. */
    combine?: boolean
    continued?: Piece[]
    /** Rows per piece; the rest continue at `continued`. */
    max_rows?: number | null
    /** Draw each hole's tag beside it. */
    show_tags?: boolean
    tags?: Tags
    /** Capital height of cell text; the standard's when unset. */
    text_height?: number | null
  }
  | {
    type: 'table'
    table: 'revision'
    /** Sheet position of the `anchor` corner, mm. */
    at: [number, number]
    rows: RevisionRow[]
    anchor?: Corner
    continued?: Piece[]
    /** Rows per piece; the rest continue at `continued`. */
    max_rows?: number | null
    /** Number revisions 1, 2, 3 instead of A, B, C. */
    numeric?: boolean
    /** Capital height of cell text; the standard's when unset. */
    text_height?: number | null
  }
  | {
    type: 'table'
    table: 'general'
    /** Sheet position of the `anchor` corner, mm. */
    at: [number, number]
    cells: string[][]
    anchor?: Corner
    continued?: Piece[]
    /** The first row is a header. */
    header?: boolean
    /** Rows per piece; the rest continue at `continued`. */
    max_rows?: number | null
    /** Capital height of cell text; the standard's when unset. */
    text_height?: number | null
  }
  | {
    type: 'table'
    table: 'general_tolerance'
    /** Sheet position of the `anchor` corner, mm. */
    at: [number, number]
    class: ToleranceClass
    anchor?: Corner
    continued?: Piece[]
    /** Rows per piece; the rest continue at `continued`. */
    max_rows?: number | null
    /** Capital height of cell text; the standard's when unset. */
    text_height?: number | null
  }
  | {
    type: 'table'
    table: 'bend'
    /** Sheet position of the `anchor` corner, mm. */
    at: [number, number]
    view: number
    anchor?: Corner
    continued?: Piece[]
    /** Rows per piece; the rest continue at `continued`. */
    max_rows?: number | null
    /** Draw each bend's tag at its bend line. */
    show_tags?: boolean
    /** Capital height of cell text; the standard's when unset. */
    text_height?: number | null
  }
  | {
    type: 'table'
    table: 'punch'
    /** Sheet position of the `anchor` corner, mm. */
    at: [number, number]
    view: number
    anchor?: Corner
    continued?: Piece[]
    /** Rows per piece; the rest continue at `continued`. */
    max_rows?: number | null
    origin?: Pick | null
    /** Capital height of cell text; the standard's when unset. */
    text_height?: number | null
  }

export type Arrows = 'auto' | 'inside' | 'outside'

export type Axis3 = 'x' | 'y' | 'z'

export type BalloonLayout = 'circular' | 'top' | 'bottom' | 'left' | 'right' | 'square'

export type BalloonShape =
  | 'circle' | 'triangle' | 'hexagon' | 'box' | 'diamond' | 'flag'
  | 'split_circle'
  | 'none'

/** What a balloon shows. */
export type BalloonText =
  | {
    type: 'item'
    component: string[]
  }
  | {
    type: 'quantity'
    component: string[]
  }
  | {
    type: 'property'
    component: string[]
    name: string
  }
  | {
    type: 'text'
    text: string
  }

export type BeadKind = 'cosmetic' | 'fillet'

/** What a bead follows. */
export type BeadPath =
  | {
    type: 'edges'
    edges: Ref[]
  }
  | {
    type: 'faces'
    from: Ref[]
    to: Ref[]
  }

export type BeadSides = 'one' | 'both'

/** Per-feature changes to the body's bend defaults. */
export type BendOverride = {
  allowance?: Allowance | null
  /** Inside radius, mm. */
  radius?: number | null
  relief?: Relief | null
}

/** Where a sketched bend or jog sits relative to its sketch line. */
export type BendPosition = 'centerline' | 'material_inside' | 'material_outside' | 'bend_outside'

export type BomType = 'top_level' | 'parts_only' | 'indented'

/** Boolean operations. */
export type BoolOp = 'fuse' | 'cut' | 'common'

export type Border = 'none' | 'box' | 'circle'

/**
 * One gap of a break view, along the view's x (vertical break lines) or
 * y axis (horizontal), in view-plane mm relative to the model origin.
 */
export type Break = {
  vertical: boolean
  from: number
  to: number
}

export type BreakStyle = 'straight' | 'curved' | 'zig_zag' | 'small_zig_zag' | 'jagged'

export type BrokenOut = {
  profile: Profile
  depth: number
}

/** One angle range of a bend calculation table. */
export type CalculationRange = {
  /** Degrees, inclusive. */
  from: number
  /** Degrees, inclusive. */
  to: number
  /** The allowance as a length, such as `angle * (radius + 0.44 * thickness)`. */
  formula: string
}

/** Which way an end cap's thickness goes. */
export type CapDirection =
  | {
    type: 'outward'
  }
  | {
    type: 'inward'
  }
  | {
    type: 'inset'
    depth: number
  }

/** How far a cap's edge stays in from the profile's edge. */
export type CapOffset =
  | {
    type: 'ratio'
    ratio: number
  }
  | {
    type: 'distance'
    distance: number
  }

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

/** Geometric characteristics. */
export type Characteristic = 'straightness' | 'flatness' | 'circularity' | 'cylindricity' | 'line_profile' | 'surface_profile' | 'angularity' | 'perpendicularity' | 'parallelism' | 'position' | 'concentricity' | 'symmetry' | 'circular_runout' | 'total_runout'

/** How a closed corner closes. */
export type ClosedCornerKind = 'butt' | 'overlap' | 'underlap'

export type CloudShape =
  | {
    type: 'rectangle'
    a: [number, number]
    b: [number, number]
  }
  | {
    type: 'ellipse'
    center: [number, number]
    rx: number
    ry: number
  }
  | {
    type: 'polygon'
    points: ([number, number])[]
  }
  | {
    type: 'freehand'
    points: ([number, number])[]
  }

export type Collision = {
  /** Check only the dragged component. */
  dragged_only?: boolean
}

export type Column =
  | {
    type: 'item'
  }
  | {
    type: 'part_number'
  }
  | {
    type: 'description'
  }
  | {
    type: 'quantity'
  }
  | {
    type: 'material'
  }
  | {
    type: 'mass'
  }
  | {
    type: 'property'
    name: string
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

export type Contour = 'none' | 'flat' | 'convex' | 'concave'

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

/** Which corner of the table `at` names. */
export type Corner = 'top_left' | 'top_right' | 'bottom_left' | 'bottom_right'

/** Rounds or bevels corners of the sheet's outline. */
export type CornerBreak =
  | {
    type: 'chamfer'
    distance: number
  }
  | {
    type: 'fillet'
    radius: number
  }

/** The shape of a corner relief, in the flat. */
export type CornerReliefKind =
  | 'rectangular'
  | 'circular'
  | 'tear'
  | 'obround'
  | 'constant_width'
  | 'full_round'
  | 'suitcase'

/** How the members of a group meet at their shared corners. */
export type CornerTreatment = 'end_miter' | 'end_butt1' | 'end_butt2'

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

/** The cut list's settings, saved with the part. */
export type CutListSettings = {
  /** Bodies left out of the cut list. */
  excluded?: string[]
  /** Bodies listed together as one item whatever their shape. */
  groups?: string[][]
  /** Item properties set by hand, keyed by the item's first body. */
  overrides?: Record<string, Record<string, string>>
  /** List each body as its own item instead of collecting identical ones. */
  separate?: boolean
  sort?: CutListSort
  /** Folders of bodies fabricated apart, listed on their own. */
  sub_weldments?: SubWeldment[]
}

export type CutListSort = 'default' | 'profile'

/**
 * The cutting line of a section view, in the parent view's model plane:
 * points are model coordinates projected into the parent (view-plane mm,
 * relative to the model origin, unscaled). Two points make a straight cut;
 * more make an offset (jogged) or aligned cut.
 */
export type CuttingLine = {
  points: ([number, number])[]
}

export type DatumRef = {
  label: string
  modifier?: Modifier | null
}

export type DetailStyle = 'standard' | 'broken' | 'leader' | 'no_leader' | 'connected'

/** Line display of a view. */
export type DisplayStyle = 'hidden_removed' | 'hidden_visible' | 'wireframe' | 'shaded_edges' | 'shaded'

export type DragPhase = 'start' | 'move' | 'end'

/** DXF file version. */
export type DxfVersion = 'r12' | 'r2018'

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

/** A profile family. */
export type Family =
  | 'ipe'
  | 'hea'
  | 'upn'
  | 'angle'
  | 'rhs'
  | 'chs'
  | 'w'
  | 'c'
  | 'l'
  | 'hss_rect'
  | 'hss_round'
  | 'pipe'

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
  | {
    type: 'base_flange'
    sketch: number
    /** The new body's sheet metal defaults. */
    sheet: SheetMetalParams
    /** For an open profile: how far it is extruded. */
    end1?: EndCondition | null
    /** For an open profile: a second direction. */
    end2?: EndCondition | null
    /** Extrude an open profile the other way. */
    reverse?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    side?: ThicknessSide
  }
  | {
    type: 'edge_flange'
    /**
     * Edges between a face of the sheet and its thickness. Each flange rises
     * from the face its edge lies on, unless `reverse`.
     */
    edges: Ref[]
    length: FlangeLength
    /** Bend angle in radians, between 0 and π (default 90°). */
    angle?: number
    /**
     * Set the angle from a face instead: the flanges run parallel or square
     * to it.
     */
    angle_from?: AngleFrom | null
    bend?: BendOverride
    end_offset?: number
    /** Gap between flanges that meet at a corner, mm. */
    gap?: number
    measure?: LengthMeasure
    position?: FlangePosition
    /** Move the flange away from the edge into the face, mm. */
    position_offset?: number
    /** Outlines replacing the rectangular wall of some flanges. */
    profiles?: FlangeProfile[]
    /** Rise from the other face of the sheet. */
    reverse?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /** Shorten each flange at the start and end of its edge, mm. */
    start_offset?: number
  }
  | {
    type: 'flat_pattern'
    /** Round or bevel the flat's outside corners. */
    break_corners?: CornerBreak | null
    /**
     * Round the inside corners of the flat's outline (relief corners) by
     * half the thickness.
     */
    corner_treatment?: boolean
    /** Faces whose regions are left out of the flat. */
    exclude?: Ref[]
    /** The face that stays in place; each body's default otherwise. */
    fixed_face?: Ref | null
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /** Straighten curved edges in bend regions (cuts across bends). */
    simplify_bends?: boolean
    /**
     * Keep the bend regions as faces of their own, split from the walls
     * along their boundaries, instead of one merged face per side.
     */
    split_bend_regions?: boolean
  }
  | {
    type: 'hem'
    edges: Ref[]
    kind: HemKind
    allowance?: Allowance | null
    /** Gap where hems on adjacent edges meet, mm. */
    gap?: number
    /** Length of the folded-back part for closed, open and double hems, mm. */
    length?: number
    position?: HemPosition
    /** Fold toward the other face of the sheet. */
    reverse?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'miter_flange'
    sketch: number
    /** Edges of one face of the sheet, the first one under the profile. */
    edges: Ref[]
    bend?: BendOverride
    end_offset?: number
    gap?: number
    position?: FlangePosition
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /** Shorten the chain at its start and end, mm. */
    start_offset?: number
  }
  | {
    type: 'sketched_bend'
    /** A sketch on a flat face of the sheet holding one line across it. */
    sketch: number
    /** Radians, between 0 and 180°. */
    angle?: number
    bend?: BendOverride
    /** A face on the side that stays put; the body's fixed face otherwise. */
    fixed?: Ref | null
    position?: BendPosition
    /** Bend toward the other face of the sheet. */
    reverse?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'jog'
    sketch: number
    /** mm. */
    offset: number
    /** Radians, between 0 and 180° (default 90°). */
    angle?: number
    bend?: BendOverride
    dimension?: JogDimension
    /**
     * Keep the moved side where it was in plan, shortening the flat; when
     * off, the flat keeps its length and the moved side shifts back.
     */
    fix_projected_length?: boolean
    fixed?: Ref | null
    position?: BendPosition
    reverse?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'cross_break'
    face: Ref
    /** Radians (default 3°). */
    angle?: number
    /** mm. */
    radius?: number
    /** Break toward the other side. */
    reverse?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'swept_flange'
    /**
     * A sketch with one open chain of lines and arcs, square to the path at
     * its start.
     */
    profile: number
    path: Ref
    bend?: BendOverride
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /** The new body's defaults when it does not join a sheet metal body. */
    sheet?: SheetMetalParams | null
    side?: ThicknessSide
  }
  | {
    type: 'lofted_bend'
    profile1: number
    profile2: number
    sheet: SheetMetalParams
    /** Formed: the most the facets may stray from the smooth surface, mm. */
    deviation?: number
    /** Bent: facets per profile segment. */
    facets?: number
    method?: LoftMethod
    /** Thickness toward the other side of the profiles. */
    reverse?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'rip'
    edges: RipEdge[]
    /** mm. */
    gap?: number
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'insert_bends'
    /**
     * A flat face of the wall that stays put; its opposite face sets the
     * thickness.
     */
    fixed_face: Ref
    /** Bend radius, allowance and relief; the thickness is measured. */
    sheet: SheetMetalParams
    rip_gap?: number
    rips?: RipEdge[]
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'convert_to_sheet_metal'
    fixed_face: Ref
    sheet: SheetMetalParams
    bend_edges: Ref[]
    /** Keep the original solid as a separate body. */
    keep_body?: boolean
    rip_gap?: number
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'unfold'
    /** The face that stays put; it also chooses the body. */
    fixed_face?: Ref | null
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'fold'
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'normal_cut'
    sketch: number
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'corner_relief'
    /** Bend faces of the corners to relieve; every corner when empty. */
    bends?: Ref[]
    /**
     * Center the relief on the bend lines rather than on the inside tangent
     * lines.
     */
    centered?: boolean
    kind?: CornerReliefKind
    /**
     * How far the relief reaches past the bend regions, as a multiple of
     * thickness.
     */
    ratio?: number
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /**
     * Add a narrow slit from each corner outward, so the bends close up
     * without tearing.
     */
    slit?: boolean
  }
  | {
    type: 'closed_corner'
    /** End faces of flanges to extend toward the flange beside them. */
    faces: Ref[]
    /** Also close the corners of end faces coplanar with the selected ones. */
    coplanar?: boolean
    /** mm. */
    gap?: number
    kind?: ClosedCornerKind
    /**
     * For overlap and underlap: how much of the covered flange's thickness
     * the covering one spans, 0 to 1.
     */
    overlap_ratio?: number
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'welded_corner'
    /** An edge of the seam. */
    edge: Ref
    /**
     * Fillet radius of the bead's outside, mm (shown with the bead; the
     * solid is not changed).
     */
    fillet?: number | null
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /** Bead leg size, mm. */
    size?: number
    /** A point where the bead stops along the seam; the whole seam otherwise. */
    stop?: Ref | null
    /** Weld symbol text shown with the bead. */
    symbol?: string | null
  }
  | {
    type: 'break_corner'
    corner: CornerBreak
    /**
     * Edges through the thickness; every outside corner of the body when
     * empty.
     */
    edges?: Ref[]
    /** Only inside (concave) corners. */
    internal_only?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'bend_notch'
    /** Bend faces; every bend when empty. */
    bends?: Ref[]
    /** Notch both ends of each bend line, not just the start. */
    both_ends?: boolean
    /** mm. */
    depth?: number
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    shape?: NotchShape
    /** mm. */
    width?: number
  }
  | {
    type: 'tab_and_slot'
    /** An edge of the tab body that rests on the slot body's face. */
    edge: Ref
    /** The face of the slot body the tabs go through. */
    face: Ref
    /** Tab length along the edge, mm. */
    length: number
    /** Extra slot length and width, mm. */
    clearance?: number
    /** Corners of the tabs' ends and of the slots. */
    corners?: TabCorner
    /** Number of tabs, spread evenly along the edge. */
    count?: number
    end?: Ref | null
    /** How far tabs stick out past the slot body; through it when omitted. */
    height?: number | null
    /** Extra slot length only, in place of `clearance`, mm. */
    length_clearance?: number | null
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /** Spacing between tab centers instead of `count`, mm. */
    spacing?: number | null
    /**
     * Points along the edge where the tabs start and end; the whole edge
     * otherwise. With only one, the tabs run to the edge's farther end.
     */
    start?: Ref | null
    /** Cut the slots through the whole slot body, not just its sheet. */
    through_all?: boolean
    /** Extra slot width only, in place of `clearance`, mm. */
    width_clearance?: number | null
  }
  | {
    type: 'sheet_metal_gusset'
    /** A face of the bend to stiffen. */
    bend: Ref
    /** Leg along each side from the inside of the bend, mm. */
    depth: number
    /** Width of the rib across the bend, mm. */
    width: number
    /**
     * Instead of `depth2`: the rib's back leaves the first wall at this
     * angle, radians.
     */
    angle?: number | null
    /** Leg along the second wall, mm; `depth` on both when omitted. */
    depth2?: number | null
    /** Where along the bend, as a fraction of its length. */
    position?: number
    punch_id?: string | null
    /** Round the rib's back across its width instead of leaving it flat. */
    rounded_back?: boolean
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'form_tool'
    shape: FormShape
    /** The flat face of the sheet the tools press into. */
    face: Ref
    /** Press from the other side. */
    flip?: boolean
    /** Shown in punch tables. */
    punch_id?: string | null
    /** Turn about the face normal, radians. */
    rotation?: number
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    /**
     * A sketch on that face whose points place one tool each; the face's
     * center when omitted.
     */
    sketch?: number | null
  }
  | {
    type: 'forming_tool'
    stopping_face: Ref
    /** The point placed on the sheet; the stopping face's centroid otherwise. */
    insertion_point?: Ref | null
    remove_faces?: Ref[]
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'weldment'
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'structural_member'
    profile: ProfileSource
    groups: MemberGroup[]
    /** Join each mitered group's members into one body. */
    merge_miter_trimmed?: boolean
    mirror?: ProfileMirror
    /** Move the profile in its own plane (x across, y up), mm. */
    offset?: [number, number]
    pierce?: PiercePoint
    /** Turn the profile about its pierce point, radians. */
    rotation?: number
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'trim_extend'
    /** Member bodies to trim or extend. */
    bodies: string[]
    boundary: TrimBoundary
    /** Lengthen members that stop short of the boundary. */
    allow_extension?: boolean
    /** Weld gap left at the boundary, mm. */
    gap?: number
    kind?: TrimKind
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'end_cap'
    /** Square end faces of structural members. */
    faces: Ref[]
    /** mm. */
    thickness: number
    /** Chamfer or round the cap's corners. */
    corners?: CornerBreak | null
    direction?: CapDirection
    offset?: CapOffset
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
  }
  | {
    type: 'weld_gusset'
    /** Two planar faces meeting at an inside corner. */
    faces: [Ref, Ref]
    shape: GussetShape
    /** mm. */
    thickness: number
    /** Corner cut off where the faces meet, for the weld, mm. */
    chamfer?: number
    location?: GussetLocation
    /** Along the shared edge from the location, mm. */
    offset?: number
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    side?: GussetSide
  }
  | {
    type: 'weld_bead'
    path: BeadPath
    /** Leg size, mm. */
    size: number
    /** Weld only from `start` to `end` along each seam, mm from its start. */
    from_to?: [number, number] | null
    intermittent?: Intermittent | null
    kind?: BeadKind
    /** For cosmetic beads, kg per metre of bead. */
    mass_per_length?: number | null
    /** Weld folder properties. */
    material?: string | null
    process?: string | null
    /** Body IDs this feature acts on; omitted acts on the whole part. */
    scope?: string[] | null
    sides?: BeadSides
    /** Weld symbol text. */
    symbol?: string | null
    /** Carry on along edges tangent to the selected ones. */
    tangent?: boolean
  }

export type FinishProcess = 'any' | 'removal' | 'no_removal'

export type FitDisplay = 'classes' | 'classes_with_tolerance' | 'tolerance_only'

/** How long a flange is. */
export type FlangeLength =
  | {
    type: 'blind'
    length: number
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
    offset: number
  }
  | {
    type: 'up_to_edge_and_merge'
    target: Ref
  }

/** Where a flange's bend sits relative to the selected edge. */
export type FlangePosition =
  | 'material_inside'
  | 'material_outside'
  | 'bend_outside'
  | 'bend_from_virtual_sharp'
  | 'tangent_to_bend'

/**
 * The wall outline of one flange of an edge flange, in the flange's plane:
 * `u` runs along the edge from its start and `v` up the wall from the end of
 * the bend, mm. The outline must rest on `v = 0`, which sets the bend's
 * extent; its highest point sets the flange length.
 */
export type FlangeProfile = {
  /** Index into `edges`. */
  edge: number
  points: ([number, number])[]
}

/** What a flat pattern view draws beside the outline. */
export type FlatView = {
  /** Bend lines, up and down on their own layers. */
  bend_lines?: boolean
  /** Bend notes (`UP 90° R1`) beside the bend lines. */
  bend_notes?: boolean
}

/** A built-in forming tool shape. Sizes in mm. */
export type FormShape =
  | {
    type: 'louver'
    length: number
    width: number
    height: number
  }
  | {
    type: 'lance'
    length: number
    width: number
    height: number
  }
  | {
    type: 'emboss'
    diameter: number
    height: number
  }
  | {
    type: 'dimple'
    diameter: number
    height: number
  }
  | {
    type: 'rib'
    length: number
    width: number
    height: number
  }
  | {
    type: 'file'
    path: string
    configuration?: number | null
  }

/** A gauge chosen from a gauge table. */
export type GaugeChoice = {
  /** A built-in table name (`steel`, `stainless`, `aluminum`) or a CSV path. */
  table: string
  /** Gauge name from the table, such as `16`. */
  gauge: string
  /** Inside radius from the table's allowed radii; the smallest when omitted. */
  radius?: number | null
}

/** One row of a feature control frame. */
export type GtolRow = {
  characteristic: Characteristic
  /** Tolerance value text, e.g. `0.05`. */
  tolerance: string
  datums?: DatumRef[]
  /** Cylindrical zone: `⌀` before the value. */
  diameter?: boolean
  modifiers?: Modifier[]
}

/** Where along the faces' shared edge a gusset goes. */
export type GussetLocation = 'start' | 'middle' | 'end'

/** A gusset's outline in its plane. */
export type GussetShape =
  | {
    type: 'triangle'
    d1: number
    d2: number
  }
  | {
    type: 'polygon'
    d1: number
    d2: number
    d3: number
    d4: number
  }

/** Where a gusset's thickness lies from its location. */
export type GussetSide = 'both' | 'inner' | 'outer'

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

export type HatchOptions = {
  /** Radians added to the pattern's own angle. */
  angle?: number
  /** `None` picks a pattern from each body's material. */
  pattern?: Pattern | null
  scale?: number
}

export type HelixInput = 'pitch_and_turns' | 'height_and_turns' | 'height_and_pitch'

export type HelixPitch = {
  /** Turn count at which this pitch applies. */
  turn: number
  /** mm per turn. */
  pitch: number
}

/** The shape of a hem. */
export type HemKind =
  | {
    type: 'closed'
  }
  | {
    type: 'open'
    gap: number
  }
  | {
    type: 'tear_drop'
    /** Radians, between 180° and 270°. */
    angle: number
    /** Inside radius, mm. */
    radius: number
  }
  | {
    type: 'rolled'
    /** Radians, less than 360°. */
    angle: number
    radius: number
  }
  | {
    type: 'double'
    gap: number
  }

/** Where a hem sits relative to its edge. */
export type HemPosition = 'material_inside' | 'bend_outside'

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

/** Beads in pieces along the path. */
export type Intermittent = {
  /** Each piece, mm. */
  length: number
  /** Between pieces, mm. */
  gap: number
  /** With both sides: the other side's pieces fall in this side's gaps. */
  staggered?: boolean
}

/** How a jog's offset is measured. */
export type JogDimension = 'outside' | 'inside' | 'overall'

export type KeepSide = 'both' | 'positive' | 'negative'

/** Kinds of sub-shape. */
export type Kind = 'face' | 'edge' | 'vertex' | 'solid'

/** What a flange's length is measured from. */
export type LengthMeasure = 'outer_virtual_sharp' | 'inner_virtual_sharp' | 'tangent_to_bend'

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

/** How a lofted bend is made. */
export type LoftMethod = 'bent' | 'formed'

export type MarkPattern = 'single' | 'linear' | 'circular'

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

/** Segments that share a profile placement and corner treatment. */
export type MemberGroup = {
  /** Straight lines of 2D or 3D sketches. */
  segments: Ref[]
  corner?: CornerTreatment
  /** Gap left between members at their corners, mm. */
  gap?: number
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

/** Which model items to import. */
export type ModelItems = {
  /** Centerlines and center marks of cylinders. */
  axes?: boolean
  dimensions?: boolean
  hole_callouts?: boolean
  threads?: boolean
}

/** A model file shown in a view. */
export type ModelSource = {
  /**
   * `.ofp` or `.ofa` file, relative to the drawing (forward slashes) or
   * absolute while the drawing is unsaved.
   */
  path: string
  /** Part or assembly configuration ID; `None` uses the file's active one. */
  configuration?: number | null
  /** Show a sheet metal part's flat pattern, looking at the flat's front. */
  flat?: FlatView | null
  /** Content hash of the file when it was last loaded. */
  hash?: string | null
}

/** Material condition and other modifiers. */
export type Modifier =
  | 'mmc'
  | 'lmc'
  | 'projected'
  | 'free_state'
  | 'tangent_plane'
  | 'rfs'
  | 'unequal'

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

/** The shape of a bend notch. */
export type NotchShape = 'v' | 'rectangular'

export type Numbering = 'detailed' | 'flat' | 'none'

/** A finite number that compares and hashes by value. */
export type OrderedFloat = number

/** Direction a linear dimension measures in. */
export type Orient = 'horizontal' | 'vertical' | 'auto' | 'aligned'

/** A view direction. */
export type Orientation =
  | {
    type: 'front'
  }
  | {
    type: 'back'
  }
  | {
    type: 'left'
  }
  | {
    type: 'right'
  }
  | {
    type: 'top'
  }
  | {
    type: 'bottom'
  }
  | {
    type: 'isometric'
  }
  | {
    type: 'dimetric'
  }
  | {
    type: 'trimetric'
  }
  | {
    type: 'custom'
    toward: [number, number, number]
    up: [number, number, number]
  }
  | {
    type: 'relative'
    front: CompRef
    top: CompRef
  }

/**
 * Values a document may change from its base standard. Lengths are mm of
 * paper.
 */
export type Overrides = {
  /** Decimal places of angles, degrees. */
  angle_precision?: number | null
  arrow_length?: number | null
  /** Use a decimal comma. */
  decimal_comma?: boolean | null
  /** Show a second value in inches after every linear dimension. */
  dual_units?: boolean | null
  /** Gap between the part and an extension line, mm. */
  extension_gap?: number | null
  /** Extension line past the dimension line, mm. */
  extension_overshoot?: number | null
  label_height?: number | null
  /** Decimal places of linear dimensions. */
  precision?: number | null
  text_height?: number | null
  thick_line?: number | null
  thin_line?: number | null
  /** Tolerance text height as a fraction of the value's. */
  tolerance_scale?: number | null
  /** Show trailing zeros up to the precision. */
  trailing_zeros?: boolean | null
  /** Dimension text always horizontal (ANSI) instead of along the line (ISO). */
  unidirectional?: boolean | null
}

/** A named hatch pattern from the library. */
export type Pattern =
  | 'general'
  | 'steel'
  | 'bronze'
  | 'aluminium'
  | 'plastic'
  | 'insulation'
  | 'solid'

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

/** Model geometry picked in a view. */
export type Pick = {
  view: number
  entity: CompRef
  /**
   * Where the user picked, in view-plane mm (model mm relative to the
   * model origin): with `point` auto, the edge's end, middle or center
   * nearest this point is used.
   */
  near?: [number, number] | null
  point?: Which
}

/** Where the rows past `max_rows` continue. */
export type Piece = {
  at: [number, number]
  /** Sheet ID; the table's own sheet when unset. */
  sheet?: number | null
}

/** The point of the profile that lies on the sketch segment. */
export type PiercePoint =
  | 'center' | 'top_left' | 'top' | 'top_right' | 'left' | 'right' | 'bottom_left' | 'bottom' | 'bottom_right'
  | 'centroid'

/** Where a projected view sits relative to its parent on the sheet. */
export type Placement = 'left' | 'right' | 'above' | 'below' | 'above_left' | 'above_right' | 'below_left' | 'below_right'

/** A point given by sketch entity ID, or a new point at a position. */
export type PointArg =
  | {
    id: number
  }
  | {
    at: [number, number]
  }

/**
 * A closed profile on a parent view (view-plane mm relative to the model
 * origin, unscaled), used by detail, broken-out section and crop views.
 */
export type Profile =
  | {
    type: 'circle'
    center: [number, number]
    radius: number
  }
  | {
    type: 'polygon'
    points: ([number, number])[]
  }

/** Mirror the profile across its own axes. */
export type ProfileMirror = 'none' | 'horizontal' | 'vertical'

/** Legacy sketch IDs retain their original whole-sketch behavior. */
export type ProfileSelection = number | ContourSelection

/** Where a member's profile comes from. */
export type ProfileSource =
  | {
    type: 'standard'
    family: Family
    size: string
  }
  | {
    type: 'custom'
    /** `.ofp` file. */
    path: string
    /** The sketch's feature ID in that part. */
    sketch: number
  }

/** How projected views are arranged around their parent. */
export type Projection = 'first' | 'third'

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

export type Relief = {
  /** How far the relief goes past the bend, mm; defaults to the width. */
  depth?: number | null
  kind?: ReliefKind
  /**
   * Relief width and its depth past the bend, as a ratio of thickness,
   * unless `width` is given.
   */
  ratio?: number
  /** mm; overrides `ratio`. */
  width?: number | null
}

/** Relief cut where a bend does not run the full length of its edge. */
export type ReliefKind = 'rectangular' | 'obround' | 'none' | 'tear'

export type RevisionRow = {
  approved?: string
  date?: string
  description?: string
  /** Letter or number; given automatically when empty. */
  revision?: string
  zone?: string
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

/** Which side of a ripped edge the gap goes. */
export type RipDirection = 'both' | 'first' | 'second'

export type RipEdge = {
  /** A sharp edge where two walls meet. */
  edge: Ref
  direction?: RipDirection
}

export type Roll = {
  axis: CompRef
  up: CompRef
}

/** A drawing scale `num:den`; 1:2 draws the model at half size. */
export type Scale = {
  num: number
  den: number
}

export type SectionKind = 'full' | 'half' | 'aligned'

/** The defaults of one sheet metal body. */
export type SheetMetalParams = {
  /** mm. */
  thickness: number
  /** Inside bend radius, mm. */
  radius: number
  allowance?: Allowance
  /** Take thickness, radius and K-factor from a gauge table instead. */
  gauge?: GaugeChoice | null
  relief?: Relief
  /** Take the K-factor and inside radius from the part's material. */
  use_material?: boolean
}

/** Paper sizes. Standard sizes are landscape unless the sheet says portrait. */
export type SheetSize =
  | {
    type: 'a0'
  }
  | {
    type: 'a1'
  }
  | {
    type: 'a2'
  }
  | {
    type: 'a3'
  }
  | {
    type: 'a4'
  }
  | {
    type: 'ansi_a'
  }
  | {
    type: 'ansi_b'
  }
  | {
    type: 'ansi_c'
  }
  | {
    type: 'ansi_d'
  }
  | {
    type: 'ansi_e'
  }
  | {
    type: 'custom'
    width: number
    height: number
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

export type Standard = {
  base?: StandardBase
  overrides?: Overrides
}

export type StandardBase = 'iso' | 'ansi'

export type SubWeldment = {
  name: string
  bodies: string[]
}

export type SweepOrientation = 'minimum_twist' | 'frenet' | 'fixed'

/** How tab and slot corners are finished. */
export type TabCorner =
  | {
    type: 'sharp'
  }
  | {
    type: 'fillet'
    radius: number
  }
  | {
    type: 'chamfer'
    distance: number
  }

export type Tags = 'alphabetic' | 'numeric'

/** How edges between tangent faces are drawn. */
export type TangentEdges = 'visible' | 'removed' | 'font'

export type TargetKind = 'point' | 'line' | 'area'

/** Which side of the sketch the sheet's thickness goes. */
export type ThicknessSide = 'reverse' | 'normal' | 'mid_plane'

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

/** Tolerance of a dimension; values in mm (or degrees for angles). */
export type Tolerance =
  | {
    type: 'none'
  }
  | {
    type: 'basic'
  }
  | {
    type: 'bilateral'
    plus: number
    minus: number
  }
  | {
    type: 'limit'
    plus: number
    minus: number
  }
  | {
    type: 'symmetric'
    value: number
  }
  | {
    type: 'min'
  }
  | {
    type: 'max'
  }
  | {
    type: 'fit'
    display?: FitDisplay
    hole?: string | null
    shaft?: string | null
  }
  | {
    type: 'general'
  }

export type ToleranceClass = 'f' | 'm' | 'c' | 'v'

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

/** What members are trimmed to. */
export type TrimBoundary =
  | {
    type: 'faces'
    faces: Ref[]
  }
  | {
    type: 'bodies'
    bodies: string[]
    /** Cope to the bodies' shape instead of a plane at their face. */
    coped?: boolean
  }

/** How trimmed members end. */
export type TrimKind = 'end_trim' | 'end_miter' | 'end_butt1' | 'end_butt2'

/** What a view shows. */
export type ViewKind =
  | {
    type: 'model'
    model: ModelSource
    orientation: Orientation
    /** Assemblies: show the active configuration's explode steps. */
    exploded?: boolean
  }
  | {
    type: 'projected'
    parent: number
    placement: Placement
  }
  | {
    type: 'auxiliary'
    parent: number
    edge: CompRef
    flip?: boolean
  }
  | {
    type: 'section'
    parent: number
    line: CuttingLine
    /**
     * Section depth in mm beyond the cut (partial section); `None`
     * shows everything behind it.
     */
    depth?: number | null
    /** Leave hardware library parts uncut. */
    exclude_fasteners?: boolean
    /** Assembly components not cut (drawn whole), by key path. */
    excluded?: string[][]
    /** Look the other way along the cut. */
    flip?: boolean
    kind?: SectionKind
    /** Show only the cut faces. */
    slice?: boolean
  }
  | {
    type: 'detail'
    parent: number
    profile: Profile
    full_outline?: boolean
    style?: DetailStyle
  }

/**
 * Model geometry seen in a view: an edge, face, vertex or reference entity
 * of the view's model (a part: empty component path).
 */
export type ViewRef = {
  view: number
  entity: CompRef
}

export type Warning = {
  code: WarningCode
  message: string
  feature: number
}

export type WarningCode = 'W_SKETCH_UNDER_DEFINED' | 'W_SKETCH_OVER_DEFINED'

export type WeldSide = {
  symbol: WeldSymbol
  contour?: Contour
  /** Length and pitch after it, e.g. `50-100`. */
  length?: string
  /** Size (or throat) before the symbol, e.g. `a5` or `6`. */
  size?: string
}

/** Weld symbols on one side of the reference line. */
export type WeldSymbol = 'fillet' | 'square' | 'v' | 'bevel' | 'u' | 'j' | 'flare_v' | 'flare_bevel' | 'plug' | 'spot' | 'seam' | 'backing' | 'surfacing' | 'edge'

/** Which point of an edge a dimension uses. */
export type Which = 'start' | 'end' | 'mid' | 'center' | 'auto'

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
  /** Export a configuration (default: active) to STEP, IGES or STL, chosen by extension. Fails while active features have errors. Hidden bodies are included. A part exported to .dxf writes its sheet metal flat patterns at true size (layers CUT, BEND_UP, BEND_DOWN, FORM, BEND_NOTES). An open drawing exports every sheet to .pdf or .dxf. */
  export: {
    /**
     * `.step`, `.stp`, `.iges`, `.igs` or `.stl`; the extension picks the
     * format. `.dxf` writes the flat patterns of the part's sheet metal bodies.
     */
    path: string
    /** Flat pattern DXF: write bend notes (`UP 90° R1`) beside the bend lines. */
    bend_notes?: boolean
    /**
     * Flat pattern DXF: the sheet metal bodies to write (IDs from part_info
     * `bodies`); omitted writes every sheet metal body.
     */
    bodies?: string[] | null
    /** Configuration ID; omitted exports the active configuration. */
    configuration?: number | null
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /** Flat pattern DXF version. */
    dxf_version?: DxfVersion | null
    /**
     * Flat pattern DXF: one file per body (`name-Body 1.dxf`, ...) instead of
     * all side by side in one.
     */
    per_body?: boolean
  }
  /** The weldment cut list: items of identical bodies (members by profile, length and end angles; sheet metal by blank; other bodies by shape) with quantity, folder and properties (DESCRIPTION, LENGTH, ANGLE1/2 in degrees, MASS kg, BLANK LENGTH/WIDTH, bounding sizes), the settings, and the weld folder (beads, total length mm, total mass kg). */
  cut_list: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Replace the cut list settings: manual groups (lists of body IDs), excluded bodies, property overrides keyed by an item's first body, sub-weldment folders, separate (no automatic grouping) and sort (default or profile). */
  set_cut_list: {
    /**
     * The whole settings object: manual groups, exclusions, overrides,
     * sub-weldments, separate and sort.
     */
    settings: CutListSettings
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Structural member profiles: families (ISO IPE, HEA, UPN, angles, SHS/RHS, CHS; ANSI W, C, L, HSS, pipe) with their sizes; with family and size, that section's outline (mm, centered on its bounding box), area, centroid and second moments. */
  weldment_profiles: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /** With `size`: that profile's outline and section properties. */
    family?: Family | null
    size?: string | null
  }
  /** Gauge tables for sheet metal parameters' `gauge`: the built-in table names, and with `table` its gauges (name, thickness mm, allowed inside radii mm, K-factor). */
  sheet_metal_gauges: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /**
     * A built-in table (`steel`, `stainless`, `aluminum`) or a CSV path;
     * omitted lists the built-in tables only.
     */
    table?: string | null
  }
  /** Sheet metal bodies: parameters, bends (radians, mm) and cut list properties (flat bounding box, cutting lengths, cut-outs, bend count, gauge). With outline, each flat pattern's loops, bend lines and form marks in flat mm. */
  sheet_metal_info: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /**
     * Include each flat pattern's outline, bend lines and form marks in
     * flat millimetres.
     */
    outline?: boolean
  }
  /** Set part custom properties used by BOMs and drawing title blocks ($PRP:"PartNo"); values may link to {mass}, {material}, {volume}, {param:KEY} or, for sheet metal, {flat_length}, {flat_width}, {thickness}, {bends}, {cutouts}, {outer_cut}, {inner_cut} and {gauge}. Null removes one. */
  set_part_properties: {
    /**
     * Part custom properties for BOMs and title blocks (`PartNo`,
     * `Description`, ...); values may link to `{mass}`, `{material}` or
     * `{param:f2.end1.depth}`. Null removes one.
     */
    properties: Record<string, Property | null>
    additionalProperties?: unknown
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Choose whether a part dimension (parameter key from part_info) is imported into drawings by insert_model_items, and the name shown under it. */
  set_dimension_display: {
    /**
     * Parameter key from part_info, such as `f2.end1.depth` or
     * `f1.dimension.4`.
     */
    key: string
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /** Name shown under the value in drawings; null removes it. */
    name?: string | null
    /** Import it into drawings with model items. */
    show?: boolean | null
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
  /** Replace the open document with a drawing of one sheet (ISO: A3, first angle; ANSI: B, third angle); with `model`, start it with a standard 3 view and isometric of that file. Add views of saved parts or assemblies with add_drawing_view; export writes every sheet to PDF or DXF. */
  new_drawing: {
    /** Part or assembly configuration ID for `model`. */
    configuration?: number | null
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /**
     * Make the drawing from this `.ofp` or `.ofa` file: a standard 3 view
     * and an isometric, scaled to fit.
     */
    model?: string | null
    /** Omitted uses first angle (ISO) or third angle (ANSI). */
    projection?: Projection | null
    /** First sheet's paper; omitted uses A3 (ISO) or ANSI B. */
    size?: SheetSize | null
    /** Drafting standard; omitted uses ISO. */
    standard?: StandardBase | null
  }
  /** Add a standard 3 view of a saved part or assembly to a sheet (front, top and side per the sheet's projection) and by default an isometric, laid out and scaled to fit. Returns the view IDs in `views`. */
  standard_views: {
    sheet: number
    /** `.ofp` or `.ofa` file. */
    model: string
    configuration?: number | null
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /** Also add an isometric view (default true). */
    isometric?: boolean
    /**
     * Omitted picks the largest preferred scale that fits and sets it as
     * the sheet scale.
     */
    scale?: Scale | null
  }
  /** Planar faces of a saved part or assembly (component path, stable face ref, outward normal, area; largest first), for relative-to-model view orientations. */
  model_planar_faces: {
    /** `.ofp` or `.ofa` file. */
    model: string
    configuration?: number | null
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Mark every view as up to date with its model. Views always draw the saved model; render_sheet reports `stale` views whose model changed since the last update or save. */
  update_drawing: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** The open drawing: standard, drawing properties, and each sheet's ID, size, scale, projection and format with its views (IDs, kinds, parents, model files, positions) and annotations. */
  drawing_info: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Draw a sheet. Returns each view's sheet box, scale, paper axes and error; without `summary` also the display list: lines, arcs, polylines, fills and text in sheet mm, each naming its view and the model edge it shows. */
  render_sheet: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /** Omitted renders the first sheet. */
    sheet?: number | null
    /** Only view boxes, scales and errors, without the display list. */
    summary?: boolean
  }
  /** Add a sheet after the last one; omitted fields copy the last sheet. Returns its ID as `id`. */
  add_sheet: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    name?: string | null
    portrait?: boolean | null
    projection?: Projection | null
    scale?: Scale | null
    /** Omitted fields copy the last sheet. */
    size?: SheetSize | null
    /** Sheet format: `iso`, `ansi` or an `.ofdt` file. */
    template?: string | null
  }
  /** Rename, resize, rescale or reorder a sheet, change its projection or format, or pick the view whose model fills the title block. */
  update_sheet: {
    id: number
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /** Show the border and title block. */
    format_visible?: boolean | null
    /** New position in the sheet order, from 0. */
    index?: number | null
    name?: string | null
    portrait?: boolean | null
    projection?: Projection | null
    /**
     * View whose model fills `$PRP` title block fields; null uses the first
     * view.
     */
    properties_view?: number | null
    scale?: Scale | null
    size?: SheetSize | null
    template?: string | null
  }
  /** Delete sheets with everything on them. A drawing keeps at least one sheet. */
  delete_sheets: {
    ids: number[]
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Add a view to a sheet: a model view of an .ofp or .ofa file in a named or custom orientation (model Y is up; front looks along -Z), or a view projected or folded from another view. `origin` is where the model origin lands on the sheet, mm. Returns its ID as `id`. */
  add_drawing_view: {
    sheet: number
    /** What the view shows. Model paths are `.ofp` or `.ofa` files. */
    view: ViewKind
    /**
     * Sheet position of the model origin, mm. Projected and auxiliary
     * views stay aligned with their parent along the fold.
     */
    origin: [number, number]
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    display?: DisplayStyle | null
    name?: string | null
    /** Omitted uses the sheet scale (model views) or the parent's. */
    scale?: Scale | null
    tangent?: TangentEdges | null
  }
  /** Move, rescale, rename or restyle a view, hide model edges or components in it, break it (gaps closed up, dimensions keep true lengths), crop it, add broken-out sections, set section hatching, or replace what it shows. */
  update_drawing_view: {
    id: number
    aligned?: boolean | null
    /** Gap between break lines, mm of paper. */
    break_gap?: number | null
    break_style?: BreakStyle | null
    /**
     * Break gaps in view-plane mm (replaces the list); empty removes the
     * break.
     */
    breaks?: Break[] | null
    /** Broken-out sections (replaces the list). */
    broken_out?: BrokenOut[] | null
    /** Crop the view to a closed profile (view-plane mm); null removes it. */
    crop?: Profile | null
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /** Null follows the parent view. */
    display?: DisplayStyle | null
    /** Section hatching: pattern (null: by material), scale and angle. */
    hatch?: HatchOptions | null
    /** Assembly components not drawn, by key path (replaces the list). */
    hidden_components?: string[][] | null
    /** Model edges not drawn (replaces the list). */
    hidden_edges?: CompRef[] | null
    label?: string | null
    name?: string | null
    origin?: [number, number] | null
    /** Null returns to the sheet or parent scale. */
    scale?: Scale | null
    tangent?: TangentEdges | null
    view?: ViewKind | null
  }
  /** Delete views and every view projected or folded from them. Returns all deleted IDs as `deleted`. */
  delete_drawing_views: {
    ids: number[]
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Add an annotation to a sheet: note, dimension (linear, angular, radius, diameter, arc length, chamfer, ordinate, baseline, chain, angular running or an imported model dimension), balloon, hole callout, center mark, centerline, surface finish, datum feature or target, geometric tolerance, weld symbol, area hatch, revision cloud or symbol, or a table (BOM, hole, revision, general, ISO 2768 general tolerance). With `view` its positions are relative to that view and it moves with it. Text may hold ⌀ ⌴ ⌵ ↧ ± ° and fields such as $PRP:"PartNo". Returns its ID as `id`. */
  add_annotation: {
    sheet: number
    annotation: AnnotationKind
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /**
     * Layer; omitted uses the kind's (DIMENSIONS, NOTES, CENTER, TABLES,
     * HATCH).
     */
    layer?: string | null
    /**
     * The view the annotation moves with: its positions are then sheet mm
     * relative to that view's origin. Omitted places it on the sheet.
     */
    view?: number | null
  }
  /** Replace an annotation's content, attach it to a view (it then moves with the view) or to the sheet (null) keeping where it is drawn, or change its layer. */
  update_annotation: {
    id: number
    /** Replaces the annotation's content (same or different kind). */
    annotation?: AnnotationKind | null
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    layer?: string | null
    /** Null moves it to the sheet, keeping where it is drawn. */
    view?: number | null
  }
  /** Move annotations by an offset in sheet mm; points on model geometry stay attached. */
  move_annotations: {
    ids: number[]
    /** Offset in sheet mm. */
    by: [number, number]
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Import model items into part views: dimensions marked for drawing (each once, in the view showing it truest; driving), Hole Wizard callouts, cosmetic thread callouts and optionally centerlines and center marks. Returns the new annotation IDs in `added`. */
  insert_model_items: {
    sheet: number
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    items?: ModelItems
    /**
     * Part views to import into; omitted uses every part view on the
     * sheet. Each dimension goes to the view that shows it truest.
     */
    views?: number[] | null
  }
  /** Change an imported model dimension on the drawing (mm or degrees): the part is edited, rebuilt and saved, and every view of it updates. Fails, changing nothing, when the part would not rebuild. Undo it in the part. */
  drive_dimension: {
    /** An imported model dimension. */
    id: number
    /** New value: mm, or degrees for angles. */
    value: number
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Balloon the components of an assembly view with their BOM item numbers, laid out around the view (square, circular, top, bottom, left or right). Returns the new IDs in `added`. */
  auto_balloons: {
    sheet: number
    /** An assembly view on the sheet. */
    view: number
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /** One balloon per BOM row instead of one per component. */
    ignore_multiple?: boolean
    layout?: BalloonLayout
  }
  /** The cells of a table annotation (BOM, hole, revision, general, tolerance) as CSV, returned as `csv` or written to `path`. */
  table_csv: {
    /** A table annotation. */
    id: number
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /** Write the CSV here; omitted returns it as `csv`. */
    path?: string | null
  }
  /** Set the drawing's drafting standard (ISO or ANSI base with overrides: text and arrow sizes, line weights, precision, zeros, decimal comma, dual units, extension lines), or load it from an .ofstd file; `save` writes it to an .ofstd file for other drawings. */
  set_drafting_standard: {
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /** Load it from an `.ofstd` file instead. */
    load?: string | null
    /** Also save the result to an `.ofstd` file for other drawings. */
    save?: string | null
    /** Replaces the drafting standard (base and overrides). */
    standard?: Standard | null
  }
  /** Write chosen sheets (default: all) to a vector PDF (embedded font, true paper size, layers; optional black and white) or a DXF (R12 or 2018; one file for all sheets side by side, or one per sheet) at true scale with layers. Returns the files in `paths`. */
  export_drawing: {
    /** `.pdf` or `.dxf` file. */
    path: string
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
    /** DXF version: `r12` (AC1009) or `r2018` (AC1032, default). */
    dxf_version?: DxfVersion | null
    /** PDF: black lines and text instead of each pen's colour. */
    monochrome?: boolean
    /**
     * DXF: one file per sheet (`name-Sheet1.dxf`, ...) instead of all
     * sheets side by side in one.
     */
    per_sheet?: boolean
    /** Sheet IDs to write; omitted writes every sheet. */
    sheets?: number[] | null
  }
  /** Delete notes and other annotations by ID. */
  delete_annotations: {
    ids: number[]
    /** `full` returns the untrimmed result; the default `summary` omits bulky snapshot fields. */
    detail?: 'summary' | 'full'
  }
  /** Set drawing properties shown by $PRPSHEET title block fields (DrawnBy, CheckedBy, Revision, Company, Tolerance, Date); null removes one. */
  set_drawing_properties: {
    /**
     * Drawing properties for `$PRPSHEET` fields (`DrawnBy`, `Revision`,
     * ...); null removes one.
     */
    properties: Record<string, string | null>
    additionalProperties?: unknown
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
  cut_list: 'doc.cut_list',
  set_cut_list: 'doc.cut_list.set',
  weldment_profiles: 'weldment.profiles',
  sheet_metal_gauges: 'sheet_metal.gauges',
  sheet_metal_info: 'doc.sheet_metal',
  set_part_properties: 'doc.properties.set',
  set_dimension_display: 'doc.dimension_display.set',
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
  new_drawing: 'drawing.new',
  standard_views: 'drawing.views.standard',
  model_planar_faces: 'drawing.model.faces',
  update_drawing: 'drawing.update',
  drawing_info: 'drawing.info',
  render_sheet: 'drawing.render',
  add_sheet: 'drawing.sheet.add',
  update_sheet: 'drawing.sheet.update',
  delete_sheets: 'drawing.sheet.delete',
  add_drawing_view: 'drawing.view.add',
  update_drawing_view: 'drawing.view.update',
  delete_drawing_views: 'drawing.view.delete',
  add_annotation: 'drawing.annotation.add',
  update_annotation: 'drawing.annotation.update',
  move_annotations: 'drawing.annotation.move',
  insert_model_items: 'drawing.model_items',
  drive_dimension: 'drawing.dimension.drive',
  auto_balloons: 'drawing.balloons.auto',
  table_csv: 'drawing.table.csv',
  set_drafting_standard: 'drawing.standard.set',
  export_drawing: 'drawing.export',
  delete_annotations: 'drawing.annotation.delete',
  set_drawing_properties: 'drawing.properties.set',
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
  cut_list: [{}],
  set_cut_list: [{"settings":{}}],
  weldment_profiles: [{}, {"family":"rhs","size":"SHS 40x40x3"}],
  sheet_metal_gauges: [{}, {"table":"steel"}],
  sheet_metal_info: [{}, {"outline":true}],
  set_part_properties: [{"properties":{"PartNo":{"value":"BRK-1"},"Weight":{"value":"{mass}"}}}],
  set_dimension_display: [{"key":"f2.end1.depth","show":false}, {"key":"f2.end1.depth","name":"THICKNESS"}],
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
  new_drawing: [{}, {"standard":"ansi"}, {"model":"part.ofp"}],
  standard_views: [{"model":"part.ofp","sheet":4}, {"isometric":false,"model":"part.ofp","scale":{"den":2,"num":1},"sheet":4}],
  model_planar_faces: [{"model":"part.ofp"}],
  update_drawing: [{}],
  drawing_info: [{}],
  render_sheet: [{"sheet":1,"summary":true}],
  add_sheet: [{}, {"scale":{"den":2,"num":1},"size":{"type":"a2"}}],
  update_sheet: [{"id":1,"scale":{"den":2,"num":1}}, {"id":1,"name":"Plate","properties_view":2}],
  delete_sheets: [{"ids":[4]}],
  add_drawing_view: [{"origin":[300,200],"sheet":1,"view":{"model":{"path":"part.ofp"},"orientation":{"type":"isometric"},"type":"model"}}, {"origin":[150,250],"sheet":1,"view":{"parent":2,"placement":"above","type":"projected"}}],
  update_drawing_view: [{"id":2,"origin":[120,160]}, {"display":"hidden_visible","id":3}, {"break_style":"zig_zag","breaks":[{"from":20,"to":40,"vertical":true}],"id":2}, {"crop":{"center":[30,20],"radius":15,"type":"circle"},"id":2}, {"broken_out":[{"depth":3,"profile":{"center":[30,20],"radius":8,"type":"circle"}}],"id":2}],
  delete_drawing_views: [{"ids":[3]}],
  add_annotation: [{"annotation":{"at":[40,60],"text":"BREAK ALL EDGES 0.5","type":"note"},"sheet":1}, {"annotation":{"at":[0,15],"dim":"model","key":"f2.end1.depth","tolerance":{"type":"symmetric","value":0.1},"type":"dimension","view":3},"sheet":1,"view":3}, {"annotation":{"at":[250,220],"rows":[{"characteristic":"flatness","tolerance":"0.05"}],"type":"gtol"},"sheet":1}, {"annotation":{"at":[20,120],"class":"m","table":"general_tolerance","type":"table"},"sheet":1}],
  update_annotation: [{"annotation":{"at":[100,250],"text":"SEE DETAIL A","type":"note"},"id":5}, {"id":5,"view":2}],
  move_annotations: [{"by":[10,-5],"ids":[5,8]}],
  insert_model_items: [{"sheet":1}, {"items":{"axes":true,"dimensions":true,"hole_callouts":false,"threads":false},"sheet":1,"views":[2]}],
  drive_dimension: [{"id":8,"value":7.5}],
  auto_balloons: [{"sheet":4,"view":6}, {"ignore_multiple":false,"layout":"circular","sheet":4,"view":6}],
  table_csv: [{"id":7}],
  set_drafting_standard: [{"standard":{"base":"ansi"}}, {"standard":{"base":"iso","overrides":{"precision":2,"text_height":5}}}],
  export_drawing: [{"monochrome":true,"path":"drawing.pdf","sheets":[1]}, {"dxf_version":"r12","path":"drawing.dxf","per_sheet":true}],
  delete_annotations: [{"ids":[5]}],
  set_drawing_properties: [{"properties":{"DrawnBy":"AK","Revision":"B"}}],
}

//! Sheet metal: parameters, the bends each feature records, and the flat
//! pattern read from them.
//!
//! A sheet metal body is a constant-thickness solid whose curved regions are
//! bends. Every feature that makes a bend records it ([`BendRecord`]) by the
//! stable names of its two cylindrical faces, so flattening never has to guess
//! which faces are bends or what allowance they use.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{EndCondition, FeatureId, Ref, State};

/// How the flat length of a bend is found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Allowance {
    /// The neutral axis lies `k × thickness` from the inside of the bend:
    /// allowance = angle × (radius + k × thickness).
    KFactor {
        #[schemars(range(min = 0.0, max = 1.0))]
        k: f64,
    },
    /// The flat length of every bend, mm.
    BendAllowance {
        #[schemars(range(min = 0.0))]
        value: f64,
    },
    /// Subtracted from the two outside (virtual sharp) lengths of every
    /// bend, mm.
    BendDeduction { value: f64 },
    /// Allowance or deduction interpolated from a bend table by thickness,
    /// inside radius and angle.
    BendTable { table: String },
    /// Allowance from equations by angle range, in the variables `angle`
    /// (radians), `degrees`, `radius` and `thickness` (lengths).
    BendCalculation { ranges: Vec<CalculationRange> },
}

impl Default for Allowance {
    fn default() -> Self {
        Allowance::KFactor { k: 0.5 }
    }
}

/// One angle range of a bend calculation table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CalculationRange {
    /// Degrees, inclusive.
    pub from: f64,
    /// Degrees, inclusive.
    pub to: f64,
    /// The allowance as a length, such as `angle * (radius + 0.44 * thickness)`.
    pub formula: String,
}

/// Relief cut where a bend does not run the full length of its edge.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReliefKind {
    #[default]
    Rectangular,
    Obround,
    /// A narrow slit instead of a cut-out.
    Tear,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Relief {
    #[serde(default)]
    pub kind: ReliefKind,
    /// Relief width and its depth past the bend, as a ratio of thickness,
    /// unless `width` is given.
    #[serde(default = "half")]
    #[schemars(range(min = 0.05, max = 2.0))]
    pub ratio: f64,
    /// mm; overrides `ratio`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// How far the relief goes past the bend, mm; defaults to the width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<f64>,
}

fn half() -> f64 {
    0.5
}

impl Default for Relief {
    fn default() -> Self {
        Relief {
            kind: ReliefKind::Rectangular,
            ratio: 0.5,
            width: None,
            depth: None,
        }
    }
}

impl Relief {
    pub fn width(&self, thickness: f64) -> f64 {
        self.width.unwrap_or(self.ratio * thickness)
    }
    pub fn depth(&self, thickness: f64) -> f64 {
        self.depth.unwrap_or(self.width(thickness))
    }
}

/// A gauge chosen from a gauge table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GaugeChoice {
    /// A built-in table name (`steel`, `stainless`, `aluminum`) or a CSV path.
    pub table: String,
    /// Gauge name from the table, such as `16`.
    pub gauge: String,
    /// Inside radius from the table's allowed radii; the smallest when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
}

/// The defaults of one sheet metal body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SheetMetalParams {
    /// mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub thickness: f64,
    /// Inside bend radius, mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub radius: f64,
    #[serde(default)]
    pub allowance: Allowance,
    #[serde(default)]
    pub relief: Relief,
    /// Take thickness, radius and K-factor from a gauge table instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gauge: Option<GaugeChoice>,
    /// Take the K-factor and inside radius from the part's material.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub use_material: bool,
}

impl Default for SheetMetalParams {
    fn default() -> Self {
        SheetMetalParams {
            thickness: 1.0,
            radius: 1.0,
            allowance: Allowance::default(),
            relief: Relief::default(),
            gauge: None,
            use_material: false,
        }
    }
}

/// Per-feature changes to the body's bend defaults.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BendOverride {
    /// Inside radius, mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowance: Option<Allowance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relief: Option<Relief>,
}

/// Which side of the sketch the sheet's thickness goes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ThicknessSide {
    /// Along the sketch normal for closed profiles; to the left of an open
    /// profile's direction.
    #[default]
    Normal,
    Reverse,
    /// Half on each side.
    MidPlane,
}

/// The first sheet metal feature: a flat tab from closed contours, or a bent
/// profile from one open chain of lines and arcs, extruded to a depth. Sharp
/// corners of an open profile get the default bend radius; arcs become bends of
/// their own radius.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BaseFlange {
    #[schemars(range(min = 1))]
    pub sketch: FeatureId,
    /// The new body's sheet metal defaults.
    pub sheet: SheetMetalParams,
    #[serde(default)]
    pub side: ThicknessSide,
    /// For an open profile: how far it is extruded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end1: Option<EndCondition>,
    /// For an open profile: a second direction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end2: Option<EndCondition>,
    /// Extrude an open profile the other way.
    #[serde(default)]
    pub reverse: bool,
}

/// Where a flange's bend sits relative to the selected edge.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FlangePosition {
    /// The flange's outer face passes through the edge; the bend eats into
    /// the face.
    #[default]
    MaterialInside,
    /// The flange's inner face passes through the edge.
    MaterialOutside,
    /// The bend starts at the edge.
    BendOutside,
    /// The outer virtual sharp lies on the edge.
    BendFromVirtualSharp,
    /// The bend's outer face touches the plane through the edge.
    TangentToBend,
}

/// What a flange's length is measured from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LengthMeasure {
    /// From the intersection of the two outer faces.
    #[default]
    OuterVirtualSharp,
    /// From the intersection of the two inner faces.
    InnerVirtualSharp,
    /// From where the bend ends.
    TangentToBend,
}

/// How long a flange is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FlangeLength {
    Blind {
        #[schemars(extend("exclusiveMinimum" = 0))]
        length: f64,
    },
    /// The flange end passes through a vertex.
    UpToVertex { target: Ref },
    /// The flange's inner face ends on a plane or planar face.
    UpToSurface { target: Ref },
    /// As `up_to_surface`, stopping `offset` short of it.
    OffsetFromSurface {
        target: Ref,
        #[schemars(range(min = 0.0))]
        offset: f64,
    },
    /// Up to an edge of another sheet metal body, joining the two bodies.
    UpToEdgeAndMerge { target: Ref },
}

impl FlangeLength {
    pub fn target(&self) -> Option<&Ref> {
        match self {
            FlangeLength::Blind { .. } => None,
            FlangeLength::UpToVertex { target }
            | FlangeLength::UpToSurface { target }
            | FlangeLength::OffsetFromSurface { target, .. }
            | FlangeLength::UpToEdgeAndMerge { target } => Some(target),
        }
    }
}

/// Walls bent up from linear edges of a sheet metal body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EdgeFlange {
    /// Edges between a face of the sheet and its thickness. Each flange rises
    /// from the face its edge lies on, unless `reverse`.
    #[schemars(length(min = 1))]
    pub edges: Vec<Ref>,
    /// Bend angle in radians, between 0 and π (default 90°).
    #[serde(default = "right_angle")]
    pub angle: f64,
    pub length: FlangeLength,
    #[serde(default)]
    pub measure: LengthMeasure,
    #[serde(default)]
    pub position: FlangePosition,
    /// Rise from the other face of the sheet.
    #[serde(default)]
    pub reverse: bool,
    /// Gap between flanges that meet at a corner, mm.
    #[serde(default = "default_gap")]
    #[schemars(range(min = 0.0))]
    pub gap: f64,
    /// Shorten each flange at the start and end of its edge, mm.
    #[serde(default)]
    #[schemars(range(min = 0.0))]
    pub start_offset: f64,
    #[serde(default)]
    #[schemars(range(min = 0.0))]
    pub end_offset: f64,
    /// Move the flange away from the edge into the face, mm.
    #[serde(default)]
    #[schemars(range(min = 0.0))]
    pub position_offset: f64,
    #[serde(default)]
    pub bend: BendOverride,
    /// Set the angle from a face instead: the flanges run parallel or square
    /// to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle_from: Option<AngleFrom>,
    /// Outlines replacing the rectangular wall of some flanges.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<FlangeProfile>,
}

/// A face that sets an edge flange's angle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AngleFrom {
    /// A planar face.
    pub face: Ref,
    /// Run parallel to the face; square to it otherwise.
    #[serde(default)]
    pub parallel: bool,
}

/// The wall outline of one flange of an edge flange, in the flange's plane:
/// `u` runs along the edge from its start and `v` up the wall from the end of
/// the bend, mm. The outline must rest on `v = 0`, which sets the bend's
/// extent; its highest point sets the flange length.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FlangeProfile {
    /// Index into `edges`.
    pub edge: usize,
    #[schemars(length(min = 3))]
    pub points: Vec<[f64; 2]>,
}

fn right_angle() -> f64 {
    std::f64::consts::FRAC_PI_2
}

fn default_gap() -> f64 {
    0.1
}

fn one() -> f64 {
    1.0
}

/// The whole part flattened. Added last and suppressed while the part is
/// formed; unsuppressing it shows every sheet metal body flat.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FlatPattern {
    /// The face that stays in place; each body's default otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_face: Option<Ref>,
    /// Round or bevel the flat's outside corners.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub break_corners: Option<CornerBreak>,
    /// Keep the bend regions as faces of their own, split from the walls
    /// along their boundaries, instead of one merged face per side.
    #[serde(default, skip_serializing_if = "is_false")]
    pub split_bend_regions: bool,
    /// Straighten curved edges in bend regions (cuts across bends).
    #[serde(default, skip_serializing_if = "is_false")]
    pub simplify_bends: bool,
    /// Round the inside corners of the flat's outline (relief corners) by
    /// half the thickness.
    #[serde(default, skip_serializing_if = "is_false")]
    pub corner_treatment: bool,
    /// Faces whose regions are left out of the flat.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<Ref>,
}

pub(crate) fn is_false(b: &bool) -> bool {
    !*b
}

/// One bend of a sheet metal body, written by the feature that made it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BendRecord {
    /// The feature that made the bend and its index there.
    pub feature: FeatureId,
    pub index: u32,
    /// The stable names of its two cylindrical faces.
    pub faces: [String; 2],
    /// Inside radius, mm.
    pub radius: f64,
    /// Radians.
    pub angle: f64,
    pub allowance: Allowance,
}

/// The sheet metal state of one body, rebuilt with it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetMetalBody {
    /// The feature that made the body.
    pub feature: FeatureId,
    /// A face of the body that stays in place when it is flattened; also how
    /// the body is found again after later features.
    pub fixed_face: String,
    pub params: SheetMetalParams,
    pub bends: Vec<BendRecord>,
    /// Bend lines, once the body is flattened.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bend_lines: Vec<BendLine>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cross_breaks: Vec<CrossBreakRecord>,
    /// Faces of faceted regions (lofted bends) on the fixed face's side;
    /// flattening unfolds across the sharp edges between them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facets: Vec<String>,
    /// Forms pressed into the body; undone in the flat and shown as marks.
    #[serde(skip)]
    pub forms: Vec<FormRecord>,
    /// Weld beads on closed corners.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub welds: Vec<CornerWeld>,
    /// Form marks, once the body is flattened.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub form_marks: Vec<FormMark>,
    /// The flat's front plane while the Flat-Pattern feature has flattened
    /// the body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flat_plane: Option<of_geom::Plane>,
}

/// A form's place in a flat pattern.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormMark {
    pub feature: FeatureId,
    pub center: [f64; 3],
    pub outline: Vec<[f64; 3]>,
    pub punch_id: Option<String>,
    /// Radians.
    pub angle: f64,
}

/// A weld bead on a closed corner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CornerWeld {
    pub feature: FeatureId,
    pub a: [f64; 3],
    pub b: [f64; 3],
    pub size: f64,
    pub symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fillet: Option<f64>,
}

impl PartialEq for FormRecord {
    fn eq(&self, other: &Self) -> bool {
        self.feature == other.feature
            && self.center == other.center
            && self.outline == other.outline
            && self.punch_id == other.punch_id
    }
}

/// The flat length of a bend.
pub fn bend_allowance(
    allowance: &Allowance,
    angle: f64,
    radius: f64,
    thickness: f64,
) -> Result<f64, String> {
    let ba = match allowance {
        Allowance::KFactor { k } => angle * (radius + k * thickness),
        Allowance::BendAllowance { value } => *value,
        Allowance::BendDeduction { value } => {
            if angle >= std::f64::consts::PI - 1e-9 {
                return Err("bend deduction needs bends of less than 180°".into());
            }
            2.0 * (radius + thickness) * (angle / 2.0).tan() - value
        }
        Allowance::BendTable { table } => {
            crate::sheet_tables::bend_table_allowance(table, angle, radius, thickness)?
        }
        Allowance::BendCalculation { ranges } => {
            let degrees = angle.to_degrees();
            let range = ranges
                .iter()
                .find(|r| degrees >= r.from - 1e-9 && degrees <= r.to + 1e-9)
                .ok_or_else(|| format!("no bend calculation range covers {degrees:.2}°"))?;
            crate::equations::evaluate_with(
                &range.formula,
                &[("angle", angle), ("degrees", degrees)],
                &[("radius", radius), ("thickness", thickness)],
            )?
        }
    };
    if !ba.is_finite() || ba <= 0.0 {
        return Err(format!("the bend allowance comes out at {ba:.4} mm"));
    }
    Ok(ba)
}

/// A bend line of a flat pattern, in model coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BendLine {
    pub a: [f64; 3],
    pub b: [f64; 3],
    /// The flange bends toward the flat's front face.
    pub up: bool,
    /// Radians.
    pub angle: f64,
    /// Inside radius, mm.
    pub radius: f64,
    /// The record it came from.
    pub feature: Option<FeatureId>,
    pub index: Option<u32>,
}

/// A sheet metal body laid flat, computed without changing the model.
#[derive(Debug, Clone)]
pub struct Flattened {
    pub shape: of_geom::Shape,
    pub names: crate::Naming,
    /// The plane of the flat's front face (the fixed face), its normal out of
    /// the material.
    pub plane: of_geom::Plane,
    pub thickness: f64,
    pub bend_lines: Vec<BendLine>,
    pub marks: Vec<FormMark>,
}

/// The index in `state.bodies` of sheet metal body `index`'s solid.
pub fn record_body(state: &State, index: usize) -> Option<usize> {
    crate::ops::sheet::record_body(state, state.sheet_metal.get(index)?)
}

/// The flat pattern of sheet metal body `index` of `state.sheet_metal`.
pub fn flattened(state: &State, index: usize) -> Result<Flattened, String> {
    let record = state
        .sheet_metal
        .get(index)
        .ok_or("no such sheet metal body")?;
    let body = crate::ops::sheet::record_body(state, record)
        .ok_or("the sheet metal body no longer exists")?;
    if let Some(plane) = record.flat_plane {
        // Already flat: the body as it is, with what flattening recorded.
        let geometry = &state.bodies[body].geometry;
        return Ok(Flattened {
            shape: geometry.shape.clone(),
            names: geometry.names.clone(),
            plane,
            thickness: record.params.thickness,
            bend_lines: record.bend_lines.clone(),
            marks: record.form_marks.clone(),
        });
    }
    let flat = crate::ops::sheet::flatten(&state.bodies[body].geometry, record, None)?;
    Ok(Flattened {
        shape: flat.shape,
        names: flat.names,
        plane: flat.plane,
        thickness: record.params.thickness,
        bend_lines: flat.bend_lines,
        marks: flat.marks,
    })
}

/// The shape of a hem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HemKind {
    /// Folded flat onto the sheet.
    Closed,
    /// Folded back with a gap, mm.
    Open {
        #[schemars(extend("exclusiveMinimum" = 0))]
        gap: f64,
    },
    /// Curled past 180° and back down onto the sheet.
    TearDrop {
        /// Radians, between 180° and 270°.
        angle: f64,
        /// Inside radius, mm.
        #[schemars(extend("exclusiveMinimum" = 0))]
        radius: f64,
    },
    /// Curled by `angle` with no straight part.
    Rolled {
        /// Radians, less than 360°.
        angle: f64,
        #[schemars(extend("exclusiveMinimum" = 0))]
        radius: f64,
    },
    /// Folded back, then forward again over itself, with a gap, mm.
    Double {
        #[schemars(range(min = 0.0))]
        gap: f64,
    },
}

/// Where a hem sits relative to its edge.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HemPosition {
    /// The fold's outside is at the edge.
    #[default]
    MaterialInside,
    /// The fold starts at the edge.
    BendOutside,
}

/// Folds edges of a sheet back on themselves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Hem {
    #[schemars(length(min = 1))]
    pub edges: Vec<Ref>,
    pub kind: HemKind,
    /// Length of the folded-back part for closed, open and double hems, mm.
    #[serde(default = "hem_length")]
    #[schemars(range(min = 0.0))]
    pub length: f64,
    #[serde(default)]
    pub position: HemPosition,
    /// Fold toward the other face of the sheet.
    #[serde(default)]
    pub reverse: bool,
    /// Gap where hems on adjacent edges meet, mm.
    #[serde(default = "default_gap")]
    #[schemars(range(min = 0.0))]
    pub gap: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowance: Option<Allowance>,
}

fn hem_length() -> f64 {
    10.0
}

/// Flanges along a chain of edges from one open profile, mitered where the
/// edges meet. The profile is sketched on a plane square to the first edge,
/// starting on it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MiterFlange {
    #[schemars(range(min = 1))]
    pub sketch: FeatureId,
    /// Edges of one face of the sheet, the first one under the profile.
    #[schemars(length(min = 1))]
    pub edges: Vec<Ref>,
    #[serde(default)]
    pub position: FlangePosition,
    #[serde(default = "default_gap")]
    #[schemars(range(min = 0.0))]
    pub gap: f64,
    /// Shorten the chain at its start and end, mm.
    #[serde(default)]
    #[schemars(range(min = 0.0))]
    pub start_offset: f64,
    #[serde(default)]
    #[schemars(range(min = 0.0))]
    pub end_offset: f64,
    #[serde(default)]
    pub bend: BendOverride,
}

/// Where a sketched bend or jog sits relative to its sketch line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BendPosition {
    /// The line is the middle of the bend in the flat.
    #[default]
    Centerline,
    /// The bent side's outer face passes through the line.
    MaterialInside,
    /// The bent side's inner face passes through the line.
    MaterialOutside,
    /// The bend starts at the line.
    BendOutside,
}

/// Bends a flat region of the sheet along a sketched line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SketchedBend {
    /// A sketch on a flat face of the sheet holding one line across it.
    #[schemars(range(min = 1))]
    pub sketch: FeatureId,
    /// A face on the side that stays put; the body's fixed face otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed: Option<Ref>,
    /// Radians, between 0 and 180°.
    #[serde(default = "right_angle")]
    pub angle: f64,
    #[serde(default)]
    pub position: BendPosition,
    /// Bend toward the other face of the sheet.
    #[serde(default)]
    pub reverse: bool,
    #[serde(default)]
    pub bend: BendOverride,
}

/// How a jog's offset is measured.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum JogDimension {
    /// Between the faces on the same side of the two walls.
    #[default]
    Outside,
    /// Between the facing faces of the two walls.
    Inside,
    /// Over both walls.
    Overall,
}

/// A Z-shaped offset of the sheet along a sketched line: two opposite bends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Jog {
    #[schemars(range(min = 1))]
    pub sketch: FeatureId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed: Option<Ref>,
    /// mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub offset: f64,
    #[serde(default)]
    pub dimension: JogDimension,
    /// Radians, between 0 and 180° (default 90°).
    #[serde(default = "right_angle")]
    pub angle: f64,
    #[serde(default)]
    pub position: BendPosition,
    /// Keep the moved side where it was in plan, shortening the flat; when
    /// off, the flat keeps its length and the moved side shifts back.
    #[serde(default = "yes")]
    pub fix_projected_length: bool,
    #[serde(default)]
    pub reverse: bool,
    #[serde(default)]
    pub bend: BendOverride,
}

fn yes() -> bool {
    true
}

/// Cosmetic stiffening crosses on a flat face: diagonal lines in views and
/// the flat pattern. The geometry is not changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CrossBreak {
    pub face: Ref,
    /// Radians (default 3°).
    #[serde(default = "break_angle")]
    pub angle: f64,
    /// mm.
    #[serde(default = "break_radius")]
    pub radius: f64,
    /// Break toward the other side.
    #[serde(default)]
    pub reverse: bool,
}

fn break_angle() -> f64 {
    3f64.to_radians()
}

fn break_radius() -> f64 {
    1.0
}

/// A cross break kept on its body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossBreakRecord {
    pub feature: FeatureId,
    /// The stable name of the face it is on.
    pub face: String,
    pub angle: f64,
    pub radius: f64,
    pub up: bool,
}

/// An open profile swept along a path into a flange. Flattening works when
/// the swept faces are planes and cylinders: a straight profile along any
/// path of lines and arcs in a plane square to it, or any profile along a
/// straight path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SweptFlange {
    /// A sketch with one open chain of lines and arcs, square to the path at
    /// its start.
    #[schemars(range(min = 1))]
    pub profile: FeatureId,
    pub path: Ref,
    /// The new body's defaults when it does not join a sheet metal body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet: Option<SheetMetalParams>,
    #[serde(default)]
    pub side: ThicknessSide,
    #[serde(default)]
    pub bend: BendOverride,
}

/// How a lofted bend is made.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LoftMethod {
    /// Flat facets with bend lines between, as brake formed.
    #[default]
    Bent,
    /// A smooth (rolled) transition, modelled with facets fine enough to
    /// stay within `deviation`.
    Formed,
}

/// A sheet metal transition between two open profiles, such as round to
/// square. Each profile is a nearly closed chain whose gap is the seam.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LoftedBend {
    #[schemars(range(min = 1))]
    pub profile1: FeatureId,
    #[schemars(range(min = 1))]
    pub profile2: FeatureId,
    #[serde(default)]
    pub method: LoftMethod,
    /// Bent: facets per profile segment.
    #[serde(default = "facets")]
    #[schemars(range(min = 1, max = 64))]
    pub facets: u32,
    /// Formed: the most the facets may stray from the smooth surface, mm.
    #[serde(default = "deviation")]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub deviation: f64,
    pub sheet: SheetMetalParams,
    /// Thickness toward the other side of the profiles.
    #[serde(default)]
    pub reverse: bool,
}

fn facets() -> u32 {
    4
}

fn deviation() -> f64 {
    0.05
}

/// Which side of a ripped edge the gap goes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RipDirection {
    /// Half the gap into each face.
    #[default]
    Both,
    /// All of it into the first face of the edge.
    First,
    /// All of it into the second face.
    Second,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RipEdge {
    /// A sharp edge where two walls meet.
    pub edge: Ref,
    #[serde(default)]
    pub direction: RipDirection,
}

/// Cuts a gap along edges where walls meet, so the part can unfold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Rip {
    #[schemars(length(min = 1))]
    pub edges: Vec<RipEdge>,
    /// mm.
    #[serde(default = "default_gap")]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub gap: f64,
}

/// Turns a thin-walled solid (such as a shell) into sheet metal: its sharp
/// corners become bends of the given radius, after ripping `rips`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InsertBends {
    /// A flat face of the wall that stays put; its opposite face sets the
    /// thickness.
    pub fixed_face: Ref,
    /// Bend radius, allowance and relief; the thickness is measured.
    pub sheet: SheetMetalParams,
    #[serde(default)]
    pub rips: Vec<RipEdge>,
    #[serde(default = "default_gap")]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub rip_gap: f64,
}

/// Makes sheet metal from any solid: the faces joined to the fixed face
/// through `bend_edges` become walls of the given thickness inside the
/// solid, those edges become bends and the other edges between them rips.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ConvertToSheetMetal {
    pub fixed_face: Ref,
    pub sheet: SheetMetalParams,
    #[schemars(length(min = 1))]
    pub bend_edges: Vec<Ref>,
    #[serde(default = "default_gap")]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub rip_gap: f64,
    /// Keep the original solid as a separate body.
    #[serde(default)]
    pub keep_body: bool,
}

/// Flattens a sheet metal body so that features can cut across its bends;
/// the next Fold forms it again with those cuts.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Unfold {
    /// The face that stays put; it also chooses the body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_face: Option<Ref>,
}

/// Forms the body flattened by the last Unfold again, carrying through the
/// cuts made while it was flat.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Fold {}

/// A cut through sheet metal whose walls are square to the sheet, even
/// across bends: closed sketch regions projected along the sketch normal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NormalCut {
    #[schemars(range(min = 1))]
    pub sketch: FeatureId,
}

/// The shape of a corner relief, in the flat.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CornerReliefKind {
    /// A rectangle over the crossing of the two bend regions.
    #[default]
    Rectangular,
    /// A circle at the crossing of the bend lines.
    Circular,
    /// A narrow slit along the bisector of the corner.
    Tear,
    /// A rectangle with rounded ends along the bisector.
    Obround,
    /// A slot of constant width along the bisector.
    ConstantWidth,
    /// A circle just covering both bend regions.
    FullRound,
    /// A rectangle with rounded corners.
    Suitcase,
}

/// Relief cuts where two bends meet at a corner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CornerRelief {
    /// Bend faces of the corners to relieve; every corner when empty.
    #[serde(default)]
    pub bends: Vec<Ref>,
    #[serde(default)]
    pub kind: CornerReliefKind,
    /// How far the relief reaches past the bend regions, as a multiple of
    /// thickness.
    #[serde(default = "half")]
    #[schemars(range(min = 0.0))]
    pub ratio: f64,
    /// Center the relief on the bend lines rather than on the inside tangent
    /// lines.
    #[serde(default = "yes")]
    pub centered: bool,
    /// Add a narrow slit from each corner outward, so the bends close up
    /// without tearing.
    #[serde(default, skip_serializing_if = "is_false")]
    pub slit: bool,
}

/// Rounds or bevels corners of the sheet's outline.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CornerBreak {
    Chamfer {
        #[schemars(extend("exclusiveMinimum" = 0))]
        distance: f64,
    },
    Fillet {
        #[schemars(extend("exclusiveMinimum" = 0))]
        radius: f64,
    },
}

/// Chamfers or rounds edges through the sheet's thickness at outline corners.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BreakCorner {
    /// Edges through the thickness; every outside corner of the body when
    /// empty.
    #[serde(default)]
    pub edges: Vec<Ref>,
    pub corner: CornerBreak,
    /// Only inside (concave) corners.
    #[serde(default)]
    pub internal_only: bool,
}

/// How a closed corner closes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClosedCornerKind {
    /// The extended flange runs to the other one's inside face, less the gap,
    /// and the other one is squared off across the corner.
    #[default]
    Butt,
    /// The extended flange covers the other one's end, by the overlap ratio of
    /// its thickness.
    Overlap,
    /// The extended flange tucks in behind the other one, which covers its end
    /// by the overlap ratio.
    Underlap,
}

/// Extends flanges so they close a corner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ClosedCorner {
    /// End faces of flanges to extend toward the flange beside them.
    #[schemars(length(min = 1))]
    pub faces: Vec<Ref>,
    #[serde(default)]
    pub kind: ClosedCornerKind,
    /// mm.
    #[serde(default = "default_gap")]
    #[schemars(range(min = 0.0))]
    pub gap: f64,
    /// For overlap and underlap: how much of the covered flange's thickness
    /// the covering one spans, 0 to 1.
    #[serde(default = "one")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub overlap_ratio: f64,
    /// Also close the corners of end faces coplanar with the selected ones.
    #[serde(default, skip_serializing_if = "is_false")]
    pub coplanar: bool,
}

/// A weld bead along a closed corner's seam (shown in views and weld tables).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WeldedCorner {
    /// An edge of the seam.
    pub edge: Ref,
    /// Bead leg size, mm.
    #[serde(default = "weld_size")]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub size: f64,
    /// Weld symbol text shown with the bead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// A point where the bead stops along the seam; the whole seam otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<Ref>,
    /// Fillet radius of the bead's outside, mm (shown with the bead; the
    /// solid is not changed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub fillet: Option<f64>,
}

fn weld_size() -> f64 {
    2.0
}

/// The shape of a bend notch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotchShape {
    #[default]
    V,
    Rectangular,
}

/// Notches at the ends of bend lines, for lining up the press brake.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BendNotch {
    /// Bend faces; every bend when empty.
    #[serde(default)]
    pub bends: Vec<Ref>,
    #[serde(default)]
    pub shape: NotchShape,
    /// mm.
    #[serde(default = "notch")]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub width: f64,
    /// mm.
    #[serde(default = "notch")]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub depth: f64,
    /// Notch both ends of each bend line, not just the start.
    #[serde(default = "yes")]
    pub both_ends: bool,
}

fn notch() -> f64 {
    1.0
}

/// Interlocking tabs on one sheet metal body and slots in another.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TabAndSlot {
    /// An edge of the tab body that rests on the slot body's face.
    pub edge: Ref,
    /// The face of the slot body the tabs go through.
    pub face: Ref,
    /// Number of tabs, spread evenly along the edge.
    #[serde(default = "two")]
    #[schemars(range(min = 1))]
    pub count: u32,
    /// Spacing between tab centers instead of `count`, mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spacing: Option<f64>,
    /// Tab length along the edge, mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub length: f64,
    /// How far tabs stick out past the slot body; through it when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    /// Extra slot length and width, mm.
    #[serde(default = "clearance")]
    #[schemars(range(min = 0.0))]
    pub clearance: f64,
    /// Extra slot length only, in place of `clearance`, mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0))]
    pub length_clearance: Option<f64>,
    /// Extra slot width only, in place of `clearance`, mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0))]
    pub width_clearance: Option<f64>,
    /// Points along the edge where the tabs start and end; the whole edge
    /// otherwise. With only one, the tabs run to the edge's farther end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<Ref>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<Ref>,
    /// Corners of the tabs' ends and of the slots.
    #[serde(default)]
    pub corners: TabCorner,
    /// Cut the slots through the whole slot body, not just its sheet.
    #[serde(default, skip_serializing_if = "is_false")]
    pub through_all: bool,
}

/// How tab and slot corners are finished.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TabCorner {
    #[default]
    Sharp,
    Fillet {
        #[schemars(extend("exclusiveMinimum" = 0))]
        radius: f64,
    },
    Chamfer {
        #[schemars(extend("exclusiveMinimum" = 0))]
        distance: f64,
    },
}

fn two() -> u32 {
    2
}

fn clearance() -> f64 {
    0.1
}

/// A stiffening rib pressed across a bend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SheetMetalGusset {
    /// A face of the bend to stiffen.
    pub bend: Ref,
    /// Where along the bend, as a fraction of its length.
    #[serde(default = "middle")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub position: f64,
    /// Leg along each side from the inside of the bend, mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub depth: f64,
    /// Leg along the second wall, mm; `depth` on both when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub depth2: Option<f64>,
    /// Instead of `depth2`: the rib's back leaves the first wall at this
    /// angle, radians.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    /// Round the rib's back across its width instead of leaving it flat.
    #[serde(default, skip_serializing_if = "is_false")]
    pub rounded_back: bool,
    /// Width of the rib across the bend, mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub width: f64,
    #[serde(default)]
    pub punch_id: Option<String>,
}

fn middle() -> f64 {
    0.5
}

/// A built-in forming tool shape. Sizes in mm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FormShape {
    /// A raised slot-shaped vent open along one long side.
    Louver {
        length: f64,
        width: f64,
        height: f64,
    },
    /// A strip pressed out, joined at both ends.
    Lance {
        length: f64,
        width: f64,
        height: f64,
    },
    /// A round raised area.
    Emboss { diameter: f64, height: f64 },
    /// A hole with a drawn collar.
    Dimple { diameter: f64, height: f64 },
    /// A long raised stiffening rib.
    Rib {
        length: f64,
        width: f64,
        height: f64,
    },
    /// A part file whose Forming Tool feature defines the tool, in one of its
    /// configurations (its active one when omitted).
    File {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        configuration: Option<u32>,
    },
}

/// What makes a part a forming tool: the face that rests on the sheet, faces
/// that become openings, and where the tool is placed from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FormingToolDef {
    pub stopping_face: Ref,
    #[serde(default)]
    pub remove_faces: Vec<Ref>,
    /// The point placed on the sheet; the stopping face's centroid otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insertion_point: Option<Ref>,
}

/// Forming tools pressed into a sheet metal face at sketch points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FormTool {
    pub shape: FormShape,
    /// The flat face of the sheet the tools press into.
    pub face: Ref,
    /// A sketch on that face whose points place one tool each; the face's
    /// center when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sketch: Option<FeatureId>,
    /// Turn about the face normal, radians.
    #[serde(default)]
    pub rotation: f64,
    /// Press from the other side.
    #[serde(default)]
    pub flip: bool,
    /// Shown in punch tables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub punch_id: Option<String>,
}

/// A form pressed into a sheet: the material it added and took away, kept so
/// the flat pattern can undo it and mark it.
#[derive(Debug, Clone)]
pub struct FormRecord {
    pub feature: FeatureId,
    pub added: Option<of_geom::Shape>,
    pub removed: Option<of_geom::Shape>,
    /// Where it was placed and its outline on the sheet, in model space.
    pub center: [f64; 3],
    pub outline: Vec<[f64; 3]>,
    pub punch_id: Option<String>,
    pub angle: f64,
}

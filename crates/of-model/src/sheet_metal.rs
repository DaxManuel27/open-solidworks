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
}

impl FlangeLength {
    pub fn target(&self) -> Option<&Ref> {
        match self {
            FlangeLength::Blind { .. } => None,
            FlangeLength::UpToVertex { target }
            | FlangeLength::UpToSurface { target }
            | FlangeLength::OffsetFromSurface { target, .. } => Some(target),
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
}

fn right_angle() -> f64 {
    std::f64::consts::FRAC_PI_2
}

fn default_gap() -> f64 {
    0.1
}

/// The whole part flattened. Added last and suppressed while the part is
/// formed; unsuppressing it shows every sheet metal body flat.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FlatPattern {
    /// The face that stays in place; each body's default otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_face: Option<Ref>,
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
}

/// The flat pattern of sheet metal body `index` of `state.sheet_metal`.
pub fn flattened(state: &State, index: usize) -> Result<Flattened, String> {
    let record = state
        .sheet_metal
        .get(index)
        .ok_or("no such sheet metal body")?;
    let body = crate::ops::sheet::record_body(state, record)
        .ok_or("the sheet metal body no longer exists")?;
    let flat = crate::ops::sheet::flatten(&state.bodies[body].geometry, record, None)?;
    Ok(Flattened {
        shape: flat.shape,
        names: flat.names,
        plane: flat.plane,
        thickness: record.params.thickness,
        bend_lines: flat.bend_lines,
    })
}

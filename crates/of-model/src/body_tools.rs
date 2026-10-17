//! Body tools and shape features: delete or keep bodies, intersect, scale,
//! flex, deform, indent, insert another part, defeature and slicing.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{BodyId, Ref};

fn yes() -> bool {
    true
}

fn one() -> f64 {
    1.0
}

fn four() -> u32 {
    4
}

/// Remove bodies from here on in the tree, or keep only the chosen ones.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DeleteBody {
    /// Solid, surface or mesh bodies.
    #[schemars(length(min = 1))]
    pub bodies: Vec<BodyId>,
    /// Keep these bodies and delete every other one.
    #[serde(default)]
    pub keep: bool,
}

/// Which regions Intersect makes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntersectMode {
    /// Regions inside the selected solids.
    #[default]
    Intersection,
    /// Regions enclosed by the tools but inside no solid.
    Internal,
    /// Both.
    Both,
}

/// Solids from the regions where solids, surfaces and planes enclose space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Intersect {
    /// Bodies (solid or surface), faces or planes.
    #[schemars(length(min = 1))]
    pub tools: Vec<Ref>,
    #[serde(default)]
    pub mode: IntersectMode,
    /// Points inside regions to leave out, mm.
    #[serde(default)]
    pub exclude: Vec<[f64; 3]>,
    /// Join the regions into one body.
    #[serde(default = "yes")]
    pub merge: bool,
    /// Remove the surface bodies used as tools.
    #[serde(default = "yes")]
    pub consume_surfaces: bool,
    /// Close flat openings in the surfaces first.
    #[serde(default)]
    pub cap_planar: bool,
}

/// Where Scale scales from.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScaleAbout {
    /// The centre of mass of the solids.
    #[default]
    Centroid,
    Origin,
    /// A coordinate system, whose axes the factors follow.
    CoordinateSystem {
        coordinate_system: Ref,
    },
}

/// Scale bodies, uniformly or along each axis, as for mould shrinkage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Scale {
    /// The bodies to scale; empty scales every body.
    #[serde(default)]
    pub bodies: Vec<BodyId>,
    /// A uniform factor.
    #[serde(default = "one")]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub factor: f64,
    /// Factors along x, y and z instead of `factor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub factors: Option<[f64; 3]>,
    #[serde(default)]
    pub about: ScaleAbout,
    /// Scale by 1 + the part material's mould shrinkage instead, for a
    /// mould cavity.
    #[serde(default)]
    pub shrinkage: bool,
}

/// How Flex deforms.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FlexMode {
    /// Bend the region into an arc of this angle (radians), toward `toward`.
    Bend { angle: f64 },
    /// Twist the region about the axis by this angle (radians).
    Twist { angle: f64 },
    /// Scale the region across the axis, from 1 at the first trim to `factor`
    /// at the second.
    Taper { factor: f64 },
    /// Lengthen the region along the axis by `distance` (mm).
    Stretch { distance: f64 },
}

/// Bend, twist, taper or stretch bodies between two trim planes along an
/// axis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Flex {
    /// The bodies to flex; empty flexes every solid.
    #[serde(default)]
    pub bodies: Vec<BodyId>,
    pub mode: FlexMode,
    /// A point on the axis.
    pub origin: Ref,
    /// The flex axis direction.
    pub axis: Ref,
    /// For a bend, the direction the bodies bend toward, across the axis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toward: Option<Ref>,
    /// The trim planes' distances from the origin along the axis, mm. The
    /// region between them flexes; beyond the second it follows rigidly.
    /// Leave empty for the bodies' whole length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trim: Option<[f64; 2]>,
    /// How finely the shape follows the deformation, 1 to 16.
    #[serde(default = "four")]
    #[schemars(range(min = 1, max = 16))]
    pub accuracy: u32,
}

/// How Deform deforms.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeformMode {
    /// Pull a point, and the material around it within `radius`, along a
    /// direction by `distance`.
    Point {
        point: Ref,
        direction: Ref,
        distance: f64,
        #[schemars(extend("exclusiveMinimum" = 0))]
        radius: f64,
    },
    /// Move edges or curves onto other ones, carrying the material within
    /// `radius` along.
    CurveToCurve {
        #[schemars(length(min = 1))]
        from: Vec<Ref>,
        #[schemars(length(min = 1))]
        to: Vec<Ref>,
        #[schemars(extend("exclusiveMinimum" = 0))]
        radius: f64,
    },
    /// Push a tool body into the bodies; they take its shape where it enters.
    SurfacePush {
        tool: BodyId,
        /// The direction the tool moves.
        direction: Ref,
        /// How far it moves, mm.
        distance: f64,
    },
}

/// Freeform deformation of bodies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Deform {
    /// The bodies to deform; empty deforms every solid.
    #[serde(default)]
    pub bodies: Vec<BodyId>,
    pub mode: DeformMode,
    /// How finely the shape follows the deformation, 1 to 16.
    #[serde(default = "four")]
    #[schemars(range(min = 1, max = 16))]
    pub accuracy: u32,
}

/// Press a tool body into a target body, which wraps around it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Indent {
    pub target: BodyId,
    pub tool: BodyId,
    /// Wall thickness around the tool, mm.
    #[schemars(range(min = 0.0))]
    pub thickness: f64,
    /// Gap between the tool and the target, mm.
    #[serde(default)]
    #[schemars(range(min = 0.0))]
    pub clearance: f64,
    /// Only cut the tool's shape out of the target.
    #[serde(default)]
    pub cut: bool,
}

/// What an inserted part brings in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InsertTransfer {
    #[serde(default = "yes")]
    pub solids: bool,
    #[serde(default = "yes")]
    pub surfaces: bool,
    /// Reference planes, axes, points and coordinate systems.
    #[serde(default)]
    pub reference_geometry: bool,
    /// Custom properties (the part's own win where both define one).
    #[serde(default)]
    pub properties: bool,
    #[serde(default)]
    pub material: bool,
}

impl Default for InsertTransfer {
    fn default() -> Self {
        InsertTransfer {
            solids: true,
            surfaces: true,
            reference_geometry: false,
            properties: false,
            material: false,
        }
    }
}

/// Bring another part in as bodies, linked to its file: they update when
/// the file changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InsertPart {
    /// The `.ofp` file.
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<u32>,
    #[serde(default)]
    pub transfer: InsertTransfer,
    /// Only these of the part's bodies (by their IDs there); empty brings
    /// every body.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bodies: Vec<BodyId>,
    /// Mirror the part in this plane first, making the opposite hand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mirror: Option<Ref>,
    /// XYZ move after mirroring, mm.
    #[serde(default)]
    pub translation: [f64; 3],
    /// Euler XYZ angles in radians about the origin.
    #[serde(default)]
    pub rotation: [f64; 3],
}

/// Simplify solids: remove picked faces and small holes and fillets,
/// healing the gaps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Defeature {
    /// The solids to simplify; empty simplifies every solid.
    #[serde(default)]
    pub bodies: Vec<BodyId>,
    /// Faces to remove.
    #[serde(default)]
    pub faces: Vec<Ref>,
    /// Remove holes narrower than this diameter, mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub holes_below: Option<f64>,
    /// Remove fillets smaller than this radius, mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fillets_below: Option<f64>,
}

/// Section curves through bodies on evenly spaced planes, usable as loft
/// profiles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Slicing {
    /// The first slice's plane.
    pub plane: Ref,
    #[schemars(range(min = 1, max = 200))]
    pub count: u32,
    /// mm between slices.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub spacing: f64,
    /// mm from the plane to the first slice.
    #[serde(default)]
    pub offset: f64,
    /// The bodies to slice; empty slices every body.
    #[serde(default)]
    pub bodies: Vec<BodyId>,
}

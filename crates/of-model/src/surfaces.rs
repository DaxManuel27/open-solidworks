//! Surface features: zero-thickness surface bodies, how they are made, joined
//! and turned into solids, and face edits that leave surfaces behind.
//!
//! Surface bodies live in [`State::surfaces`](crate::State::surfaces), apart
//! from solids, so solid features never fuse a surface by accident. Features
//! that take surfaces say so in their fields; the rest refuse surface
//! geometry with an error.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{BodyId, EndCondition, FeatureId, Ref, RevolveAngle, SweepOrientation};

fn yes() -> bool {
    true
}

fn knit_tolerance() -> f64 {
    0.0025
}

/// Extrude a sketch's open and closed contours into a surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ExtrudedSurface {
    #[schemars(range(min = 1))]
    pub sketch: FeatureId,
    /// Blind, mid plane, up to vertex, up to surface or offset from surface.
    pub end1: EndCondition,
    /// Optional second direction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end2: Option<EndCondition>,
    #[serde(default)]
    pub reverse: bool,
    /// Close both ends of closed contours, making a closed surface body.
    #[serde(default)]
    pub cap_ends: bool,
}

/// Revolve a sketch's open and closed contours into a surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RevolvedSurface {
    #[schemars(range(min = 1))]
    pub sketch: FeatureId,
    /// A sketch line, reference axis or straight edge.
    pub axis: Ref,
    /// Blind, mid plane or two direction.
    pub angle: RevolveAngle,
    #[serde(default)]
    pub reverse: bool,
}

/// Sweep an open or closed profile along a path into a surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SweptSurface {
    /// A sketch contour or curve: open or closed.
    pub profile: Ref,
    /// A sketch contour, curve or edge chain.
    pub path: Ref,
    #[serde(default)]
    pub orientation: SweepOrientation,
}

/// A surface through two or more open or closed profiles, in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LoftedSurface {
    /// Sketch contours or curves, in order.
    #[schemars(length(min = 2))]
    pub profiles: Vec<Ref>,
    /// Straight lines between the profiles.
    #[serde(default)]
    pub ruled: bool,
}

/// A surface bounded by curves in two directions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BoundarySurface {
    /// Ordered curves in direction 1 (sketch contours, curves or edges).
    #[schemars(length(min = 2))]
    pub curves1: Vec<Ref>,
    /// Ordered curves in direction 2 crossing every curve in direction 1;
    /// leave empty to skin through direction 1 only.
    #[serde(default)]
    pub curves2: Vec<Ref>,
}

/// A flat surface filling a closed sketch or a closed loop of coplanar edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlanarSurface {
    /// A sketch whose closed regions become faces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sketch: Option<FeatureId>,
    /// Or coplanar model edges forming a closed loop.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<Ref>,
}

/// Copies of faces moved along their normals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OffsetSurface {
    #[schemars(length(min = 1))]
    pub faces: Vec<Ref>,
    /// mm; 0 makes an exact copy.
    #[schemars(range(min = 0.0))]
    pub distance: f64,
    /// Offset against the faces' normals instead.
    #[serde(default)]
    pub flip: bool,
}

/// Join surface bodies that share edges into one, optionally a solid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Knit {
    /// Surface bodies to join.
    #[schemars(length(min = 1))]
    pub bodies: Vec<BodyId>,
    /// Make a solid when the result encloses a volume, and fail when it doesn't.
    #[serde(default)]
    pub create_solid: bool,
    /// Edges closer than this are joined, mm.
    #[serde(default = "knit_tolerance")]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub tolerance: f64,
}

/// Which side of a surface a thickness or cut goes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ThickenSide {
    /// Along the surface's normal.
    #[default]
    Side1,
    /// Against it.
    Side2,
    /// Half each way.
    Both,
}

/// Turn a surface body into a solid by giving it thickness, or fill the
/// volume a closed surface body encloses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Thicken {
    pub body: BodyId,
    /// mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub thickness: f64,
    #[serde(default)]
    pub side: ThickenSide,
    /// Fill the volume a closed surface body encloses; `thickness` is unused.
    #[serde(default)]
    pub fill_volume: bool,
    /// Remove the thickened material from the solids instead of adding it.
    #[serde(default)]
    pub cut: bool,
    /// Join the new solid with the solids it touches.
    #[serde(default = "yes")]
    pub merge: bool,
}

/// Cut away the part of the solids on one side of a surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CutWithSurface {
    /// A surface body, a face, or a plane. It must reach across the solids.
    pub surface: Ref,
    /// By default the material on the side the surface's normal points to is
    /// removed; flip removes the other side.
    #[serde(default)]
    pub flip: bool,
}

/// How Delete Face closes the gap the faces leave.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeleteFaceMode {
    /// Leave the gap open; a solid becomes a surface body.
    #[default]
    Delete,
    /// Extend the neighbouring faces until they close the gap.
    Patch,
    /// Fill the gap with a new patch that meets the neighbours.
    Fill,
    /// Fill the gap with a patch tangent to the neighbours.
    FillTangent,
}

/// Remove faces from a solid or surface body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DeleteFace {
    /// Faces of one body.
    #[schemars(length(min = 1))]
    pub faces: Vec<Ref>,
    #[serde(default)]
    pub mode: DeleteFaceMode,
}

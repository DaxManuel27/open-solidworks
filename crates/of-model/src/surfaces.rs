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

/// How a patch meets the face beside one of its edges.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PatchContinuity {
    /// Touches it.
    #[default]
    Contact,
    /// Tangent to it.
    Tangent,
    /// Matching its curvature.
    Curvature,
}

/// One edge around a filled surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FillBoundary {
    /// A model edge, or a sketch contour or curve.
    pub edge: Ref,
    #[serde(default)]
    pub continuity: PatchContinuity,
    /// Use the face on the other side of the edge for tangent or curvature.
    #[serde(default)]
    pub alternate_face: bool,
}

/// A patch filling a boundary of edges and curves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FilledSurface {
    /// Edges and curves around the patch; they must form a closed loop.
    #[schemars(length(min = 1))]
    pub boundary: Vec<FillBoundary>,
    /// Curves the patch must pass through.
    #[serde(default)]
    pub constraints: Vec<Ref>,
    /// Join the patch to the surface body its edges belong to.
    #[serde(default = "yes")]
    pub merge: bool,
    /// When merged and closed, make the result a solid.
    #[serde(default)]
    pub create_solid: bool,
}

/// The direction a ruled surface's lines run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuledDirection {
    /// In the face's tangent plane, square to the edge.
    TangentToSurface,
    /// Along the face's normal.
    NormalToSurface,
    /// At `angle` (radians) from a direction.
    TaperedToVector { direction: Ref, angle: f64 },
    /// Square to both the edge and a direction.
    PerpendicularToVector { direction: Ref },
    /// Along a direction, made square to the edge.
    Sweep { direction: Ref },
}

/// Strips of straight lines from edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RuledSurface {
    #[schemars(length(min = 1))]
    pub edges: Vec<Ref>,
    pub direction: RuledDirection,
    /// mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub distance: f64,
    /// Go to the other side.
    #[serde(default)]
    pub alternate_side: bool,
    /// Join the strips to each other where they meet.
    #[serde(default = "yes")]
    pub knit: bool,
}

/// Strips spreading from edges, parallel to a plane, as for a parting surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RadiateSurface {
    #[schemars(length(min = 1))]
    pub edges: Vec<Ref>,
    /// The strips lie parallel to this plane.
    pub plane: Ref,
    /// mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub distance: f64,
    #[serde(default)]
    pub flip: bool,
}

/// Cut surface bodies with a tool, or with each other, keeping chosen pieces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TrimSurface {
    /// A plane, surface body or face that cuts the bodies. Omit it for a
    /// mutual trim, where the bodies cut each other.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<Ref>,
    /// The surface bodies to trim.
    #[schemars(length(min = 1))]
    pub bodies: Vec<BodyId>,
    /// Points on the pieces to keep (or to remove, with `remove`), mm.
    #[serde(default)]
    pub pieces: Vec<[f64; 3]>,
    /// The pieces listed are removed instead of kept.
    #[serde(default)]
    pub remove: bool,
    /// Keep every piece, split apart.
    #[serde(default)]
    pub split_all: bool,
}

/// What Untrim Surface restores.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UntrimMode {
    /// The face's whole underlying surface.
    #[default]
    Natural,
    /// Only fill the face's holes.
    FillHoles,
}

/// Restore faces of surface bodies to their natural boundary, or fill holes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UntrimSurface {
    #[schemars(length(min = 1))]
    pub faces: Vec<Ref>,
    #[serde(default)]
    pub mode: UntrimMode,
}

/// How far Extend Surface goes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExtendEnd {
    Distance {
        /// mm.
        #[schemars(extend("exclusiveMinimum" = 0))]
        distance: f64,
    },
    /// Until the extension reaches a point or vertex.
    UpToPoint { point: Ref },
    /// Until the extension meets a plane, face or surface.
    UpToSurface { target: Ref },
}

/// Extend surface bodies past their open edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ExtendSurface {
    /// Open edges of surface bodies.
    #[schemars(length(min = 1))]
    pub edges: Vec<Ref>,
    pub end: ExtendEnd,
    /// Continue tangent in straight lines instead of along the same surface.
    #[serde(default)]
    pub linear: bool,
}

/// Two faces facing each other across a wall.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FacePair {
    pub a: Ref,
    pub b: Ref,
}

/// A surface midway between pairs of faces, such as a thin wall's two sides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MidSurface {
    /// Face pairs; leave empty to find them in `body`.
    #[serde(default)]
    pub pairs: Vec<FacePair>,
    /// The solid to find face pairs in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<BodyId>,
    /// Faces further apart than this are not paired, mm.
    #[serde(default = "pair_threshold")]
    pub threshold: f64,
    /// Where the surface sits from face `a` (0) to face `b` (1).
    #[serde(default = "half")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub position: f64,
    /// Knit the mid faces together.
    #[serde(default = "yes")]
    pub knit: bool,
}

fn pair_threshold() -> f64 {
    10.0
}

fn half() -> f64 {
    0.5
}

/// One control point move of a freeform edit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FreeformMove {
    /// Where on the face, from 0 to 1 across its parameter range.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub u: f64,
    #[schemars(range(min = 0.0, max = 1.0))]
    pub v: f64,
    /// How far the point moves, mm.
    pub offset: [f64; 3],
}

/// Reshape a face by moving its control points; its edges stay put.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Freeform {
    pub face: Ref,
    /// Control points across the face in each direction.
    #[serde(default = "freeform_grid")]
    #[schemars(range(min = 4, max = 40))]
    pub grid: u32,
    #[schemars(length(min = 1))]
    pub moves: Vec<FreeformMove>,
    /// How the reshaped face meets its neighbours.
    #[serde(default)]
    pub continuity: PatchContinuity,
}

fn freeform_grid() -> u32 {
    8
}

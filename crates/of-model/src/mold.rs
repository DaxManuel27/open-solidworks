//! Mould tools: parting lines, shut-off and parting surfaces, tooling split
//! into core and cavity, and side cores. The surfaces and blocks they make
//! are recorded in mould folders that later mould features read.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{BodyId, FeatureId, Ref};

fn yes() -> bool {
    true
}

fn one_degree() -> f64 {
    1f64.to_radians()
}

/// Where the mould splits: the edges between faces that leave with the
/// cavity (facing along the pull) and with the core (facing against it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PartingLine {
    /// The solid to mould; defaults to the only solid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<BodyId>,
    /// The direction the cavity pulls away: a plane, face or edge.
    pub pull: Ref,
    #[serde(default)]
    pub reverse: bool,
    /// Draft the faces need, radians.
    #[serde(default = "one_degree")]
    pub draft_angle: f64,
    /// Split faces that face both ways at their silhouette.
    #[serde(default = "yes")]
    pub split_faces: bool,
}

/// How a shut-off loop is closed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShutOffPatch {
    /// A patch that touches the loop.
    #[default]
    Contact,
    /// A patch tangent to the cavity side's faces.
    Tangent,
    /// Leave the loop open.
    NoFill,
}

/// Patches closing the through openings of a moulded part, so the core and
/// cavity meet there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ShutOffSurfaces {
    /// The Parting Line feature whose openings to close.
    pub parting_line: FeatureId,
    #[serde(default)]
    pub patch: ShutOffPatch,
    /// Patch types for particular openings, by their order from largest.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patches: Vec<ShutOffPatch>,
}

/// Which way a parting surface spreads from the parting line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PartingSurfaceMode {
    /// Flat, square to the pull direction.
    #[default]
    PerpendicularToPull,
    /// Continuing the part's faces.
    TangentToSurface,
    /// Along the part's faces' normals.
    NormalToSurface,
}

/// A surface spreading out from the parting line, which divides the core
/// block from the cavity block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PartingSurface {
    pub parting_line: FeatureId,
    #[serde(default)]
    pub mode: PartingSurfaceMode,
    /// mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub distance: f64,
    #[serde(default)]
    pub reverse: bool,
}

/// A tapered lip around the parting surface, so the mould halves locate on
/// each other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Interlock {
    /// Taper, radians.
    pub angle: f64,
    /// Height of the lip above the parting surface, mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub height: f64,
    /// Width of the lip from the block's edge, mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub width: f64,
}

/// A block around the part split into core and cavity along the parting
/// and shut-off surfaces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolingSplit {
    /// The Parting Line feature to split along.
    pub parting_line: FeatureId,
    /// A sketch of the block outline, on a plane square to the pull.
    pub sketch: FeatureId,
    /// Block depth along the pull (cavity side) and against it (core side), mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub depth1: f64,
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub depth2: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interlock: Option<Interlock>,
}

/// Pull a side core or lifter out of a core or cavity block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MoldCore {
    /// The block to take it from.
    pub body: BodyId,
    /// A sketch of its outline; it extends along the sketch's normal.
    pub sketch: FeatureId,
    #[serde(default)]
    pub reverse: bool,
    /// mm along the direction.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub depth1: f64,
    /// mm the other way.
    #[serde(default)]
    pub depth2: f64,
    /// Taper, radians.
    #[serde(default)]
    pub draft: f64,
}

/// A parting line's result, for later mould features.
#[derive(Debug, Clone, PartialEq)]
pub struct PartingRecord {
    pub feature: FeatureId,
    pub body: BodyId,
    pub pull: of_geom::DVec3,
    /// The outer loop's edges, by stable name.
    pub edges: Vec<String>,
    /// The other loops (through openings), largest first.
    pub openings: Vec<Vec<String>>,
    pub cavity_faces: Vec<String>,
    pub core_faces: Vec<String>,
}

/// Mould folders: which bodies the mould features made.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MoldState {
    pub parting_lines: Vec<PartingRecord>,
    pub shut_off: Vec<BodyId>,
    pub parting_surfaces: Vec<BodyId>,
    pub cores: Vec<BodyId>,
    pub cavities: Vec<BodyId>,
}

impl MoldState {
    pub fn record(&self, feature: FeatureId) -> Result<&PartingRecord, String> {
        self.parting_lines
            .iter()
            .find(|r| r.feature == feature)
            .ok_or_else(|| format!("parting line {feature} is missing or failed"))
    }
}

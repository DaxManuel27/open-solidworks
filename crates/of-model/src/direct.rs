//! Direct editing: moving, offsetting and replacing faces without going back
//! to the features that made them. Each edit is a feature in the tree, so
//! editing an earlier feature re-applies it, and a moved face keeps its name.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Ref;

/// How Move Face moves its faces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FaceMotion {
    /// Each face along its own normal; a positive distance adds material.
    Offset {
        /// mm.
        distance: f64,
    },
    /// Along a direction by a distance, or until the faces reach a vertex or
    /// a flat face.
    Translate {
        /// An edge, axis, or plane whose normal is the direction.
        direction: Ref,
        /// mm; ignored with `up_to`.
        #[serde(default)]
        distance: f64,
        #[serde(default)]
        reverse: bool,
        /// A vertex, point or flat face to move up to.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        up_to: Option<Ref>,
    },
    /// About an axis.
    Rotate {
        /// A reference axis, straight edge or cylindrical face.
        axis: Ref,
        /// Radians.
        angle: f64,
    },
}

/// Move faces of a solid; the neighbouring faces grow or shrink to meet them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MoveFace {
    #[schemars(length(min = 1))]
    pub faces: Vec<Ref>,
    pub motion: FaceMotion,
}

/// Replace faces of a solid with a surface; the solid grows or shrinks to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ReplaceFace {
    #[schemars(length(min = 1))]
    pub faces: Vec<Ref>,
    /// A surface body, or a face of one.
    pub replacement: Ref,
}

/// Remove the holes (inner loops) of surface faces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DeleteHole {
    #[schemars(length(min = 1))]
    pub faces: Vec<Ref>,
}

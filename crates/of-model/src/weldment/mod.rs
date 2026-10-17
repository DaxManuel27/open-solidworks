//! Weldments: structural members cut from standard or custom profiles,
//! their trims, end caps and gussets, weld beads and cut lists.

pub mod beads;
pub mod cutlist;
pub mod details;
pub mod members;
pub mod profiles;
pub mod trim;

use of_geom::{Curve2, DVec2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::FeatureId;
pub use beads::{BeadKind, BeadPath, BeadSides, Intermittent, WeldBead, WeldRecord};
pub use details::{
    CapDirection, CapOffset, EndCap, GussetLocation, GussetShape, GussetSide, WeldGusset,
};
pub use members::{
    CornerTreatment, MemberGroup, MemberRecord, PiercePoint, ProfileMirror, StructuralMember,
};
pub use profiles::{Family, Section};
pub use trim::{TrimBoundary, TrimExtend, TrimKind};

/// Marks the part as a weldment: members and other weldment features make
/// bodies of their own, and the part has a cut list.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WeldmentDef {}

/// Where a member's profile comes from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProfileSource {
    /// A size from the built-in library (`weldment_profiles` lists them).
    Standard { family: Family, size: String },
    /// The closed region of a sketch in a part file.
    Custom {
        /// `.ofp` file.
        path: String,
        /// The sketch's feature ID in that part.
        sketch: FeatureId,
    },
}

impl ProfileSource {
    /// A short description for cut lists: `SHS 40x40x3` or the file's stem.
    pub fn description(&self) -> String {
        match self {
            ProfileSource::Standard { size, .. } => size.clone(),
            ProfileSource::Custom { path, sketch } => {
                let stem = std::path::Path::new(path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("profile");
                format!("{stem} (sketch {sketch})")
            }
        }
    }
}

/// The section a profile source draws.
pub fn section(source: &ProfileSource) -> Result<Section, String> {
    match source {
        ProfileSource::Standard { family, size } => profiles::section(*family, size),
        ProfileSource::Custom { path, sketch } => custom_section(path, *sketch),
    }
}

/// A part's sketch as a section: its largest closed region, centered on its
/// bounding box.
fn custom_section(path: &str, sketch: FeatureId) -> Result<Section, String> {
    let doc = crate::file::load(std::path::Path::new(path))
        .map_err(|e| format!("can't open profile {path}: {e}"))?;
    let model = crate::Model::new(doc);
    let (_, sk) = model
        .state()
        .sketches
        .get(&sketch)
        .ok_or_else(|| format!("{path} has no sketch {sketch}"))?;
    let contours = sk.contours().map_err(|e| e.to_string())?;
    let faces = crate::ops::closed_regions(&contours)?;
    let face = faces
        .into_iter()
        .max_by(|a, b| area(&a.outer).total_cmp(&area(&b.outer)))
        .ok_or("the profile sketch has no closed region")?;
    let mut s = Section {
        outer: face.outer,
        holes: face.holes,
        size: DVec2::ZERO,
        wall: 0.0,
    };
    let pts = profiles::outline_points(&s);
    let lo = pts.iter().fold(DVec2::splat(f64::MAX), |a, b| a.min(*b));
    let hi = pts.iter().fold(DVec2::splat(f64::MIN), |a, b| a.max(*b));
    let center = (lo + hi) / 2.0;
    s.outer = s.outer.iter().map(|c| shift(c, -center)).collect();
    s.holes = s
        .holes
        .iter()
        .map(|h| h.iter().map(|c| shift(c, -center)).collect())
        .collect();
    s.size = hi - lo;
    // Without a table, take a tenth of the smaller side as the wall.
    s.wall = s.size.min_element() / 10.0;
    Ok(s)
}

fn area(curves: &[Curve2]) -> f64 {
    let s = Section {
        outer: curves.to_vec(),
        holes: Vec::new(),
        size: DVec2::ZERO,
        wall: 0.0,
    };
    profiles::properties(&s).0.abs()
}

fn shift(c: &Curve2, d: DVec2) -> Curve2 {
    match c {
        Curve2::Line { a, b } => Curve2::Line {
            a: *a + d,
            b: *b + d,
        },
        Curve2::Arc { center, start, end } => Curve2::Arc {
            center: *center + d,
            start: *start + d,
            end: *end + d,
        },
        Curve2::Circle { center, radius } => Curve2::Circle {
            center: *center + d,
            radius: *radius,
        },
        Curve2::Bezier { points } => Curve2::Bezier {
            points: points.map(|p| p + d),
        },
        Curve2::Spline { points, closed } => Curve2::Spline {
            points: points.iter().map(|p| *p + d).collect(),
            closed: *closed,
        },
    }
}

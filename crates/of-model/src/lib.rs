//! The part document: feature tree, rebuild, rollback and suppression,
//! stable face/edge IDs (topological naming), and the `.ofp` file format.
//!
//! A [`Document`] is plain data: an ordered list of [`Feature`]s. A
//! [`Model`] owns a document and rebuilds it into geometry, keeping the state
//! after every feature so edits rebuild only from the changed feature on, and
//! so the model can be shown rolled back to any point.
//!
//! Features never refer to faces or edges by kernel index. They hold
//! [`Ref`]s with stable names (see [`naming`]) that are resolved against the
//! model as it is at that feature during every rebuild.

mod build;
mod features;
pub mod file;
pub mod materials;
pub mod measure;
pub mod naming;
mod ops;
mod resolve;
pub mod units;

pub use build::{Body, Datum, FeatureStatus, Model, State};
pub use features::*;
pub use naming::Naming;
pub use ops::{HoleDims, convert_edges, hole_dims};
pub use resolve::Resolved;
pub use units::Units;

pub use of_geom as geom;
pub use of_sketch as sketch;

use of_geom::DVec3;
use serde::{Deserialize, Serialize};

pub type FeatureId = u32;

/// A reference to geometry, by stable name rather than kernel index.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Ref {
    Origin,
    FrontPlane,
    TopPlane,
    RightPlane,
    Face {
        name: String,
    },
    Edge {
        name: String,
    },
    Vertex {
        name: String,
    },
    /// A reference plane, axis, point, coordinate system or sketch.
    Feature {
        id: FeatureId,
    },
    /// A point or line in a sketch.
    SketchEntity {
        sketch: FeatureId,
        entity: of_sketch::Id,
    },
}

/// A feature in the tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Feature {
    pub id: FeatureId,
    pub name: String,
    #[serde(default)]
    pub suppressed: bool,
    #[serde(flatten)]
    pub kind: FeatureKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FeatureKind {
    Sketch(SketchFeature),
    Plane(PlaneDef),
    Axis(AxisDef),
    Point(PointDef),
    CoordSys(CoordSysDef),
    Extrude(Extrude),
    Revolve(Revolve),
    Fillet(Fillet),
    Chamfer(Chamfer),
    Shell(ShellDef),
    Hole(Hole),
    LinearPattern(LinearPattern),
    CircularPattern(CircularPattern),
    Mirror(MirrorDef),
    Import(Import),
}

impl FeatureKind {
    /// Base for default feature names ("Boss-Extrude1", "Fillet2", ...).
    pub fn base_name(&self) -> &'static str {
        match self {
            FeatureKind::Sketch(_) => "Sketch",
            FeatureKind::Plane(_) => "Plane",
            FeatureKind::Axis(_) => "Axis",
            FeatureKind::Point(_) => "Point",
            FeatureKind::CoordSys(_) => "Coordinate System",
            FeatureKind::Extrude(e) if e.cut => "Cut-Extrude",
            FeatureKind::Extrude(_) => "Boss-Extrude",
            FeatureKind::Revolve(r) if r.cut => "Cut-Revolve",
            FeatureKind::Revolve(_) => "Revolve",
            FeatureKind::Fillet(_) => "Fillet",
            FeatureKind::Chamfer(_) => "Chamfer",
            FeatureKind::Shell(_) => "Shell",
            FeatureKind::Hole(_) => "Hole",
            FeatureKind::LinearPattern(_) => "LPattern",
            FeatureKind::CircularPattern(_) => "CirPattern",
            FeatureKind::Mirror(_) => "Mirror",
            FeatureKind::Import(_) => "Imported",
        }
    }

    /// Features this one depends on directly (not counting geometry refs).
    pub fn depends_on(&self) -> Vec<FeatureId> {
        let mut out = Vec::new();
        let mut refs: Vec<&Ref> = Vec::new();
        match self {
            FeatureKind::Sketch(s) => refs.push(&s.plane),
            FeatureKind::Extrude(e) => {
                out.push(e.sketch);
                for end in std::iter::once(&e.end1).chain(e.end2.as_ref()) {
                    refs.extend(end.target());
                }
            }
            FeatureKind::Revolve(r) => {
                out.push(r.sketch);
                refs.push(&r.axis);
            }
            FeatureKind::Hole(h) => out.push(h.sketch),
            FeatureKind::LinearPattern(p) => {
                out.extend(&p.features);
                refs.push(&p.direction1.reference);
                if let Some(d) = &p.direction2 {
                    refs.push(&d.reference);
                }
            }
            FeatureKind::CircularPattern(p) => {
                out.extend(&p.features);
                refs.push(&p.axis);
            }
            FeatureKind::Mirror(m) => {
                out.extend(&m.features);
                refs.push(&m.plane);
            }
            FeatureKind::Plane(p) => refs.extend(p.refs()),
            FeatureKind::Axis(a) => refs.extend(a.refs()),
            FeatureKind::Point(p) => refs.extend(p.refs()),
            FeatureKind::CoordSys(c) => {
                refs.push(&c.origin);
                refs.extend(c.x_axis.iter().chain(&c.z_axis));
            }
            _ => {}
        }
        for r in refs {
            match r {
                Ref::Feature { id } | Ref::SketchEntity { sketch: id, .. } => out.push(*id),
                _ => {}
            }
        }
        out
    }
}

/// The part document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub units: Units,
    /// Material name from [`materials::LIBRARY`], or a custom density.
    #[serde(default)]
    pub material: Option<materials::Material>,
    pub features: Vec<Feature>,
    /// Only the first `rollback` features are built. `None` builds all.
    #[serde(default)]
    pub rollback: Option<usize>,
    next_id: FeatureId,
}

impl Default for Document {
    fn default() -> Self {
        Document {
            units: Units::default(),
            material: None,
            features: Vec::new(),
            rollback: None,
            next_id: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ModelError {
    #[error("no feature {0}")]
    NoFeature(FeatureId),
    #[error("{0}")]
    Invalid(String),
}

impl Document {
    pub fn new() -> Document {
        Document::default()
    }

    pub fn feature(&self, id: FeatureId) -> Option<&Feature> {
        self.features.iter().find(|f| f.id == id)
    }

    pub fn feature_mut(&mut self, id: FeatureId) -> Option<&mut Feature> {
        self.features.iter_mut().find(|f| f.id == id)
    }

    pub fn index_of(&self, id: FeatureId) -> Option<usize> {
        self.features.iter().position(|f| f.id == id)
    }

    /// A name like "Fillet3" that isn't used yet.
    pub fn next_name(&self, kind: &FeatureKind) -> String {
        let base = kind.base_name();
        (1..)
            .map(|i| format!("{base}{i}"))
            .find(|n| !self.features.iter().any(|f| &f.name == n))
            .unwrap()
    }

    /// Insert a feature at the rollback position (the end when not rolled
    /// back), returning its id.
    pub fn add(&mut self, kind: FeatureKind) -> FeatureId {
        let id = self.next_id;
        self.next_id += 1;
        let name = self.next_name(&kind);
        let at = self
            .rollback
            .unwrap_or(self.features.len())
            .min(self.features.len());
        self.features.insert(
            at,
            Feature {
                id,
                name,
                suppressed: false,
                kind,
            },
        );
        if let Some(r) = &mut self.rollback {
            *r += 1;
        }
        id
    }

    /// Delete features, and every feature that depends on them.
    pub fn delete(&mut self, ids: &[FeatureId]) -> Vec<FeatureId> {
        let mut doomed: Vec<FeatureId> = ids.to_vec();
        loop {
            let more: Vec<FeatureId> = self
                .features
                .iter()
                .filter(|f| !doomed.contains(&f.id))
                .filter(|f| f.kind.depends_on().iter().any(|d| doomed.contains(d)))
                .map(|f| f.id)
                .collect();
            if more.is_empty() {
                break;
            }
            doomed.extend(more);
        }
        if let Some(r) = &mut self.rollback {
            let before = self.features[..(*r).min(self.features.len())]
                .iter()
                .filter(|f| doomed.contains(&f.id))
                .count();
            *r -= before;
        }
        self.features.retain(|f| !doomed.contains(&f.id));
        doomed
    }

    /// Move a feature to a new position, if that keeps every feature after
    /// the things it depends on.
    pub fn reorder(&mut self, id: FeatureId, to: usize) -> Result<(), ModelError> {
        let from = self.index_of(id).ok_or(ModelError::NoFeature(id))?;
        let mut trial = self.features.clone();
        let f = trial.remove(from);
        trial.insert(to.min(trial.len()), f);
        for (i, f) in trial.iter().enumerate() {
            for d in f.kind.depends_on() {
                if !trial[..i].iter().any(|g| g.id == d) {
                    return Err(ModelError::Invalid(format!(
                        "{} must come after the features it uses",
                        f.name
                    )));
                }
            }
        }
        self.features = trial;
        Ok(())
    }
}

/// Unit vector helper used across features.
pub(crate) fn unit(v: DVec3) -> DVec3 {
    v.normalize_or(DVec3::Z)
}

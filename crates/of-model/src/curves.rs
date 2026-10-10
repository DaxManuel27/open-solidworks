//! Referenceable construction curves and section/path consumers.
use crate::{FeatureId, Ref, Resolved, State, resolve};
use of_geom::{DVec3, Kind, Plane, Shape};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct CurveBody {
    pub shape: Shape,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Helix {
    pub plane: Ref,
    pub radius: f64,
    pub pitch: f64,
    pub turns: f64,
    #[serde(default)]
    pub start_angle: f64,
    #[serde(default)]
    pub left_handed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurveSelection {
    pub reference: Ref,
    #[serde(default)]
    pub reverse: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositeCurve {
    pub segments: Vec<CurveSelection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectedCurve {
    pub source: Ref,
    pub target: Ref,
    pub direction: DVec3,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SweepOrientation {
    #[default]
    MinimumTwist,
    Frenet,
    Fixed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sweep {
    pub profile: FeatureId,
    pub path: Ref,
    #[serde(default)]
    pub reverse_path: bool,
    #[serde(default)]
    pub orientation: SweepOrientation,
    #[serde(default)]
    pub cut: bool,
    #[serde(default = "yes")]
    pub merge: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Loft {
    pub profiles: Vec<FeatureId>,
    #[serde(default)]
    pub reversed: Vec<FeatureId>,
    #[serde(default)]
    pub ruled: bool,
    #[serde(default)]
    pub cut: bool,
    #[serde(default = "yes")]
    pub merge: bool,
}

/// Return an entire connected sketch path or a persistent entity reference.
pub(crate) fn path(state: &State, reference: &Ref) -> Result<Shape, String> {
    match reference {
        Ref::Feature { id } => {
            if let Some(curve) = state.curves.get(id) {
                return Ok(curve.shape.clone());
            }
            let (plane, sketch) = state
                .sketches
                .get(id)
                .ok_or("the path feature is missing or failed")?;
            let contours = sketch.contours().map_err(|e| e.to_string())?;
            let mut chains = contours.loops.iter().chain(&contours.open);
            let chain = chains.next().ok_or("the sketch has no path")?;
            if chains.next().is_some() {
                return Err("select a sketch with one connected path".into());
            }
            Shape::wire(
                plane,
                &chain
                    .curves
                    .iter()
                    .map(crate::ops::to_geom)
                    .collect::<Vec<_>>(),
            )
            .map_err(|e| e.0)
        }
        Ref::SketchEntity { sketch, entity } => {
            let (plane, sketch) = state
                .sketches
                .get(sketch)
                .ok_or("the path sketch is missing")?;
            let curve = sketch.curve(*entity).map_err(|e| e.to_string())?;
            Shape::wire(plane, &[crate::ops::to_geom(&curve)]).map_err(|e| e.0)
        }
        _ => {
            let (kind, index) = state.index_of(reference)?;
            if kind != Kind::Edge {
                return Err("select an edge or curve path".into());
            }
            let shape = state
                .body
                .as_ref()
                .ok_or("there is no body")?
                .shape
                .sub_shape(kind, index)
                .map_err(|e| e.0)?;
            Shape::join_curves(&[shape]).map_err(|e| e.0)
        }
    }
}

pub(crate) fn section(state: &State, id: FeatureId) -> Result<(Plane, Shape), String> {
    let (plane, sketch) = state
        .sketches
        .get(&id)
        .ok_or("the profile sketch is missing or failed")?;
    let contours = sketch.contours().map_err(|e| e.to_string())?;
    if contours.loops.len() != 1 || !contours.open.is_empty() {
        return Err("this operation requires one closed profile without holes".into());
    }
    Ok((*plane, path(state, &Ref::Feature { id })?))
}

pub(crate) fn helix(p: &Helix, prev: &State) -> Result<Shape, String> {
    let plane = resolve(prev, &p.plane)?.plane()?;
    Shape::helix(
        &plane,
        p.radius,
        p.pitch,
        p.turns,
        p.start_angle,
        p.left_handed,
    )
    .map_err(|e| e.0)
}

pub(crate) fn composite(p: &CompositeCurve, prev: &State) -> Result<Shape, String> {
    if p.segments.is_empty() {
        return Err("select curves in path order".into());
    }
    let shapes = p
        .segments
        .iter()
        .map(|s| {
            let shape = path(prev, &s.reference)?;
            Ok(if s.reverse { shape.reversed() } else { shape })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Shape::join_curves(&shapes).map_err(|e| e.0)
}

pub(crate) fn projected(p: &ProjectedCurve, prev: &State) -> Result<Shape, String> {
    let source = path(prev, &p.source)?;
    let target = match resolve(prev, &p.target)? {
        Resolved::Body { id, .. } => prev.solid(&id)?.geometry.shape.clone(),
        Resolved::Face { index, .. } => prev
            .body
            .as_ref()
            .ok_or("there is no body")?
            .shape
            .sub_shape(Kind::Face, index)
            .map_err(|e| e.0)?,
        _ => return Err("select a face or body to project onto".into()),
    };
    source.project_onto(&target, p.direction).map_err(|e| e.0)
}

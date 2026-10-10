//! Rebuilding a document into geometry.

use std::collections::HashMap;

use of_geom::{BoolOp, DVec3, Kind, Plane, Shape};
use of_sketch::{Geometry, Sketch, SolveReport};
use serde::{Deserialize, Serialize};

use crate::naming::Naming;
use crate::resolve::{RResult, Resolved, resolve};
use crate::{
    AxisDef, CoordSysDef, Document, Feature, FeatureId, FeatureKind, PlaneDef, PointDef, Ref,
    SketchFeature, ops, unit,
};

/// The solid geometry and its face/edge names.
#[derive(Debug, Clone)]
pub struct Body {
    pub shape: Shape,
    pub names: Naming,
}

/// Reference geometry.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Datum {
    Plane(Plane),
    Axis {
        origin: DVec3,
        dir: DVec3,
    },
    Point(DVec3),
    CoordSys {
        origin: DVec3,
        x: DVec3,
        y: DVec3,
        z: DVec3,
    },
}

/// What a feature added or removed, kept so patterns and mirrors can repeat it.
#[derive(Debug, Clone)]
pub(crate) struct Tool {
    pub shape: Shape,
    pub names: Naming,
    pub op: BoolOp,
}

/// The model after some prefix of the feature tree.
#[derive(Debug, Clone, Default)]
pub struct State {
    pub curves: HashMap<FeatureId, crate::curves::CurveBody>,
    pub bodies: Vec<crate::SolidBody>,
    /// Derived aggregate cache for legacy feature operations and whole-part I/O.
    pub body: Option<Body>,
    pub datums: HashMap<FeatureId, Datum>,
    /// Each sketch's plane and solved geometry.
    pub sketches: HashMap<FeatureId, (Plane, Sketch)>,
    pub(crate) tools: HashMap<FeatureId, Vec<Tool>>,
    pub(crate) tool_order: Vec<FeatureId>,
}

/// How a feature's rebuild went.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureStatus {
    pub id: FeatureId,
    /// Why the feature failed; it is skipped when it does.
    pub error: Option<String>,
    /// Sketch solve result, for sketches.
    pub sketch: Option<SolveReport>,
    pub suppressed: bool,
    /// Past the rollback bar, so not built.
    pub rolled_back: bool,
}

/// A document plus its rebuilt geometry.
pub struct Model {
    pub doc: Document,
    /// `states[i]` is the model after feature `i`.
    states: Vec<State>,
    statuses: Vec<FeatureStatus>,
    empty: State,
}

impl Model {
    pub fn new(doc: Document) -> Model {
        let mut m = Model {
            doc,
            states: Vec::new(),
            statuses: Vec::new(),
            empty: State::default(),
        };
        m.rebuild();
        m
    }

    /// Forget cached results from feature `index` on.
    pub fn invalidate_from(&mut self, index: usize) {
        // A changed authored parameter may drive an earlier feature. Until a
        // reverse dependency cache is available, rebuild every equation consumer.
        let index = if self.doc.equations.is_empty()
            && self.doc.configurations == crate::configurations::Configurations::default()
        {
            index
        } else {
            0
        };
        self.states.truncate(index);
        self.statuses.truncate(index);
    }

    /// Forget cached results from a feature on.
    pub fn invalidate(&mut self, id: FeatureId) {
        if let Some(i) = self.doc.index_of(id) {
            self.invalidate_from(i);
        }
    }

    /// Number of features that are built (before the rollback bar).
    pub fn active_len(&self) -> usize {
        self.doc
            .rollback
            .unwrap_or(self.doc.features.len())
            .min(self.doc.features.len())
    }

    /// Rebuild whatever isn't cached, up to the rollback bar.
    pub fn rebuild(&mut self) {
        let end = self.active_len();
        let mut evaluation = crate::equations::effective(&self.doc);
        while self.states.len() < end {
            let i = self.states.len();
            let prev = if i == 0 {
                State::default()
            } else {
                self.states[i - 1].clone()
            };
            let (state, status) = evaluated_feature(&mut evaluation, i, prev);
            if self.doc.equations.is_empty()
                && self.doc.configurations == crate::configurations::Configurations::default()
            {
                self.doc.features[i].kind = evaluation.document.features[i].kind.clone();
            }
            self.states.push(state);
            self.statuses.push(status);
        }
    }

    /// Rebuild everything from scratch.
    pub fn rebuild_all(&mut self) {
        self.invalidate_from(0);
        self.rebuild();
    }

    /// The model at the rollback bar (or the end).
    pub fn state(&self) -> &State {
        let end = self.active_len();
        if end == 0 {
            &self.empty
        } else {
            &self.states[end.min(self.states.len()) - 1]
        }
    }

    /// The model just before a feature, for editing that feature.
    pub fn state_before(&mut self, id: FeatureId) -> &State {
        let Some(i) = self.doc.index_of(id) else {
            return &self.empty;
        };
        let mut evaluation = crate::equations::effective(&self.doc);
        while self.states.len() < i {
            let k = self.states.len();
            let prev = if k == 0 {
                State::default()
            } else {
                self.states[k - 1].clone()
            };
            let (state, status) = evaluated_feature(&mut evaluation, k, prev);
            self.states.push(state);
            self.statuses.push(status);
        }
        if i == 0 {
            &self.empty
        } else {
            &self.states[i - 1]
        }
    }

    /// The model just after a feature, if it's built.
    pub fn state_after(&self, id: FeatureId) -> Option<&State> {
        self.states.get(self.doc.index_of(id)?)
    }

    /// Build a feature without adding it, for a live preview: on top of the
    /// current model, or in place of `editing`.
    pub fn preview(
        &mut self,
        kind: &FeatureKind,
        editing: Option<FeatureId>,
    ) -> Result<State, String> {
        let scope = editing
            .and_then(|id| self.doc.feature(id))
            .and_then(|f| f.scope.clone());
        self.preview_scoped(kind, editing, scope.as_deref())
    }

    pub fn preview_scoped(
        &mut self,
        kind: &FeatureKind,
        editing: Option<FeatureId>,
        scope: Option<&[crate::BodyId]>,
    ) -> Result<State, String> {
        if let Some(id) = editing
            && (!self.doc.equations.is_empty()
                || self.doc.configurations != crate::configurations::Configurations::default())
        {
            // Editing a later parameter can also drive earlier geometry. Build the
            // complete trial prefix with the same resolution rules as commit.
            let mut trial = self.doc.clone();
            let index = trial.index_of(id).ok_or("no such feature")?;
            trial.features[index].kind = kind.clone();
            trial.features[index].scope = scope.map(<[_]>::to_vec);
            trial.rollback = Some(index + 1);
            let preview = Model::new(trial);
            if let Some(error) = preview.status(id).and_then(|status| status.error) {
                return Err(error);
            }
            return Ok(preview.state().clone());
        }
        let prev = match editing {
            Some(id) => self.state_before(id).clone(),
            None => self.state().clone(),
        };
        let mut state = prev.clone();
        let id = editing.unwrap_or(FeatureId::MAX);
        build_scoped(id, kind, scope, &prev, &mut state)?;
        Ok(state)
    }

    /// Status of every feature, including rolled-back ones.
    pub fn statuses(&self) -> Vec<FeatureStatus> {
        let active = self.active_len();
        self.doc
            .features
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let cached = self.statuses.get(i).filter(|_| i < active);
                cached.cloned().unwrap_or(FeatureStatus {
                    id: f.id,
                    error: None,
                    sketch: None,
                    suppressed: f.suppressed,
                    rolled_back: true,
                })
            })
            .collect()
    }

    pub fn status(&self, id: FeatureId) -> Option<FeatureStatus> {
        self.statuses().into_iter().find(|s| s.id == id)
    }

    /// Resolve a reference against the current model.
    pub fn resolve(&self, r: &Ref) -> RResult<Resolved> {
        resolve(self.state(), r)
    }
}

fn evaluated_feature(
    evaluation: &mut crate::equations::Evaluation,
    index: usize,
    prev: State,
) -> (State, FeatureStatus) {
    let f = &evaluation.document.features[index];
    if !f.suppressed
        && let Some(error) = evaluation.errors.get(&f.id)
    {
        return (
            prev,
            FeatureStatus {
                id: f.id,
                error: Some(error.clone()),
                sketch: None,
                suppressed: false,
                rolled_back: false,
            },
        );
    }
    build_feature(&mut evaluation.document, index, prev)
}

fn build_feature(doc: &mut Document, index: usize, prev: State) -> (State, FeatureStatus) {
    let f: Feature = doc.features[index].clone();
    let mut status = FeatureStatus {
        id: f.id,
        error: None,
        sketch: None,
        suppressed: f.suppressed,
        rolled_back: false,
    };
    if f.suppressed {
        return (prev, status);
    }
    let mut state = prev.clone();
    let result: RResult<()> = match &f.kind {
        FeatureKind::Sketch(s) => match build_sketch(&prev, s) {
            Ok((plane, sketch, report)) => {
                // Keep the solved geometry in the document.
                if let FeatureKind::Sketch(stored) = &mut doc.features[index].kind {
                    stored.sketch = sketch.clone();
                }
                let failed = report.status == of_sketch::SolveStatus::Failed;
                status.sketch = Some(report);
                state.sketches.insert(f.id, (plane, sketch));
                if failed {
                    Err("the sketch can't be solved; check its relations".into())
                } else {
                    Ok(())
                }
            }
            Err(e) => Err(e),
        },
        kind => build_scoped(f.id, kind, f.scope.as_deref(), &prev, &mut state),
    };
    if let Err(e) = result {
        status.error = Some(e);
        // A failed sketch still shows (and can be fixed); other features are skipped.
        if !matches!(f.kind, FeatureKind::Sketch(_)) {
            state = prev;
        }
    }
    (state, status)
}

fn build_scoped(
    id: FeatureId,
    kind: &FeatureKind,
    scope: Option<&[crate::BodyId]>,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let Some(scope) = scope else {
        return build_kind(id, kind, prev, state);
    };
    if !matches!(
        kind,
        FeatureKind::Extrude(_)
            | FeatureKind::Draft(_)
            | FeatureKind::VariableFillet(_)
            | FeatureKind::Sweep(_)
            | FeatureKind::Loft(_)
            | FeatureKind::Revolve(_)
            | FeatureKind::Fillet(_)
            | FeatureKind::Chamfer(_)
            | FeatureKind::Shell(_)
            | FeatureKind::Hole(_)
            | FeatureKind::LinearPattern(_)
            | FeatureKind::CircularPattern(_)
            | FeatureKind::Mirror(_)
    ) {
        return Err("this feature does not accept a body scope".into());
    }
    if scope.is_empty() {
        return Err("select at least one body for the feature scope".into());
    }
    for body in scope {
        prev.solid(body)?;
    }
    let mut selected = prev.clone();
    selected.bodies.retain(|b| scope.contains(&b.id));
    selected.aggregate_bodies()?;
    let mut result = selected.clone();
    build_kind(id, kind, &selected, &mut result)?;
    result.bodies.extend(
        prev.bodies
            .iter()
            .filter(|b| !scope.contains(&b.id))
            .cloned(),
    );
    result.aggregate_bodies()?;
    *state = result;
    Ok(())
}

/// Build any feature except a sketch onto `state`.
fn build_kind(id: FeatureId, kind: &FeatureKind, prev: &State, state: &mut State) -> RResult<()> {
    let curve = match kind {
        FeatureKind::Helix(p) => Some(crate::curves::helix(p, prev)?),
        FeatureKind::CompositeCurve(p) => Some(crate::curves::composite(p, prev)?),
        FeatureKind::ProjectedCurve(p) => Some(crate::curves::projected(p, prev)?),
        _ => None,
    };
    if let Some(shape) = curve {
        state.curves.insert(id, crate::curves::CurveBody { shape });
        return Ok(());
    }
    let datum = match kind {
        FeatureKind::Plane(p) => build_plane(prev, p)?,
        FeatureKind::Axis(a) => build_axis(prev, a)?,
        FeatureKind::Point(p) => build_point(prev, p)?,
        FeatureKind::CoordSys(c) => build_coord_sys(prev, c)?,
        FeatureKind::Sketch(s) => {
            let (plane, sketch, _) = build_sketch(prev, s)?;
            state.sketches.insert(id, (plane, sketch));
            return Ok(());
        }
        FeatureKind::Combine(p) => return crate::bodies::combine(id, p, prev, state),
        FeatureKind::MoveBody(p) => return crate::bodies::move_body(id, p, prev, state),
        FeatureKind::SplitBody(p) => return crate::bodies::split_body(id, p, prev, state),
        _ => {
            ops::build(id, kind, prev, state)?;
            return state.refresh_bodies(id, prev);
        }
    };
    state.datums.insert(id, datum);
    Ok(())
}

fn build_sketch(prev: &State, s: &SketchFeature) -> RResult<(Plane, Sketch, SolveReport)> {
    let plane = resolve(prev, &s.plane)?.plane()?;
    let mut sketch = s.sketch.clone();
    update_external(prev, &plane, &mut sketch)?;
    sketch.validate().map_err(|e| e.to_string())?;
    let report = sketch.solve();
    Ok((plane, sketch, report))
}

/// Refresh geometry converted from model edges.
fn update_external(prev: &State, plane: &Plane, sketch: &mut Sketch) -> RResult<()> {
    let externals: Vec<(of_sketch::Id, String)> = sketch
        .entities
        .iter()
        .filter_map(|(id, e)| e.external.clone().map(|x| (*id, x)))
        .collect();
    for (id, name) in externals {
        let r = if name.starts_with('{') {
            serde_json::from_str(&name).map_err(|_| "invalid converted edge reference")?
        } else {
            Ref::Edge { name }
        };
        let Ok(Resolved::Edge { info, .. }) = resolve(prev, &r) else {
            return Err("an edge converted into this sketch no longer exists".into());
        };
        let geo = sketch.entities[&id].geo.clone();
        let local = |p: DVec3| plane.to_local(p);
        match geo {
            Geometry::Line { a, b } => {
                sketch.set_point(a, local(info.start)).ok();
                sketch.set_point(b, local(info.end)).ok();
            }
            Geometry::Circle { center, .. } => {
                sketch.set_point(center, local(info.center)).ok();
                if let Some(e) = sketch.entities.get_mut(&id)
                    && let Geometry::Circle { radius, .. } = &mut e.geo
                {
                    *radius = info.radius;
                }
            }
            Geometry::Arc { center, start, end } => {
                sketch.set_point(center, local(info.center)).ok();
                // Kernel arcs run counter-clockwise about their axis; sketch
                // arcs about the sketch normal.
                let (s, e) = (local(info.start), local(info.end));
                let (s, e) = if info.axis.dot(plane.normal) >= 0.0 {
                    (s, e)
                } else {
                    (e, s)
                };
                sketch.set_point(start, s).ok();
                sketch.set_point(end, e).ok();
            }
            _ => {}
        }
    }
    Ok(())
}

fn build_plane(prev: &State, p: &PlaneDef) -> RResult<Datum> {
    let plane = match p {
        PlaneDef::Offset {
            reference,
            distance,
            flip,
        } => {
            let base = resolve(prev, reference)?.plane()?;
            base.offset(if *flip { -distance } else { *distance })
        }
        PlaneDef::Angle {
            reference,
            axis,
            angle,
        } => {
            let base = resolve(prev, reference)?.plane()?;
            let (origin, dir) = resolve(prev, axis)?.axis()?;
            let rot = of_geom::DAffine3::from_axis_angle(dir, *angle);
            let n = rot.transform_vector3(base.normal);
            Plane::new(origin, dir, n)
        }
        PlaneDef::ThreePoints { points } => {
            let p: Vec<DVec3> = points
                .iter()
                .map(|r| resolve(prev, r)?.point())
                .collect::<RResult<_>>()?;
            let n = (p[1] - p[0]).cross(p[2] - p[0]);
            if n.length() < 1e-9 {
                return Err("the three points are in a line".into());
            }
            Plane::new(p[0], p[1] - p[0], n)
        }
        PlaneDef::MidPlane { a, b } => {
            let (pa, pb) = (resolve(prev, a)?.plane()?, resolve(prev, b)?.plane()?);
            let nb = if pa.normal.dot(pb.normal) < 0.0 {
                -pb.normal
            } else {
                pb.normal
            };
            if pa.normal.cross(nb).length() > 1e-6 {
                // Bisect the angle between them, through their intersection.
                let n = unit(pa.normal - nb);
                let origin = (pa.origin + pb.origin) * 0.5;
                Plane::new(origin, pa.x_dir, n)
            } else {
                let d = pb.distance(pa.origin);
                Plane::new(pa.origin - pb.normal * (d * 0.5), pa.x_dir, pa.normal)
            }
        }
        PlaneDef::ParallelAtPoint { reference, point } => {
            let base = resolve(prev, reference)?.plane()?;
            let p = resolve(prev, point)?.point()?;
            Plane { origin: p, ..base }
        }
        PlaneDef::NormalToCurve { edge, at_end } => match resolve(prev, edge)? {
            Resolved::Edge { info, index } => {
                let body = prev.body.as_ref().unwrap();
                let pts = body.shape.edge_samples(index, 0.01, 0.1).map_err(|e| e.0)?;
                let n = pts.len();
                let (p, t) = if *at_end {
                    (info.end, pts[n - 1] - pts[n - 2])
                } else {
                    (info.start, pts[1] - pts[0])
                };
                Plane::new(p, t.any_orthonormal_vector(), t)
            }
            Resolved::Axis { origin, dir } => Plane::new(origin, dir.any_orthonormal_vector(), dir),
            _ => return Err("select an edge".into()),
        },
    };
    Ok(Datum::Plane(plane))
}

fn build_axis(prev: &State, a: &AxisDef) -> RResult<Datum> {
    let (origin, dir) = match a {
        AxisDef::TwoPoints { a, b } => {
            let (p, q) = (resolve(prev, a)?.point()?, resolve(prev, b)?.point()?);
            if (q - p).length() < 1e-9 {
                return Err("the points are at the same place".into());
            }
            (p, unit(q - p))
        }
        AxisDef::TwoPlanes { a, b } => {
            let (pa, pb) = (resolve(prev, a)?.plane()?, resolve(prev, b)?.plane()?);
            let dir = pa.normal.cross(pb.normal);
            if dir.length() < 1e-9 {
                return Err("the planes are parallel".into());
            }
            // A point on both planes: solve along the two normals.
            let (n1, n2) = (pa.normal, pb.normal);
            let (d1, d2) = (n1.dot(pa.origin), n2.dot(pb.origin));
            let det = n1.dot(n1) * n2.dot(n2) - n1.dot(n2).powi(2);
            let c1 = (d1 * n2.dot(n2) - d2 * n1.dot(n2)) / det;
            let c2 = (d2 * n1.dot(n1) - d1 * n1.dot(n2)) / det;
            (n1 * c1 + n2 * c2, unit(dir))
        }
        AxisDef::Cylinder { face } => resolve(prev, face)?.axis()?,
        AxisDef::Edge { edge } => resolve(prev, edge)?.axis()?,
        AxisDef::PointAndFace { point, face } => {
            let p = resolve(prev, point)?.point()?;
            (p, resolve(prev, face)?.plane()?.normal)
        }
    };
    Ok(Datum::Axis { origin, dir })
}

fn build_point(prev: &State, p: &PointDef) -> RResult<Datum> {
    Ok(Datum::Point(match p {
        PointDef::Coordinates { x, y, z } => DVec3::new(*x, *y, *z),
        PointDef::At { target } => resolve(prev, target)?.point()?,
    }))
}

fn build_coord_sys(prev: &State, c: &CoordSysDef) -> RResult<Datum> {
    let origin = resolve(prev, &c.origin)?.point()?;
    let z = match &c.z_axis {
        Some(r) => resolve(prev, r)?.direction()?,
        None => DVec3::Z,
    };
    let x = match &c.x_axis {
        Some(r) => resolve(prev, r)?.direction()?,
        None => DVec3::X,
    };
    let x = x - z * x.dot(z);
    if x.length() < 1e-9 {
        return Err("the X and Z directions are parallel".into());
    }
    let x = unit(x);
    Ok(Datum::CoordSys {
        origin,
        x,
        y: z.cross(x),
        z,
    })
}

impl State {
    /// Kernel index of a face, edge or vertex reference.
    pub fn index_of(&self, r: &Ref) -> RResult<(Kind, u32)> {
        crate::resolve::index_of(self, r)
    }

    /// The stable reference for a face, edge or vertex picked by index.
    pub fn ref_for(&self, kind: Kind, index: u32) -> Option<Ref> {
        let names = &self.body.as_ref()?.names;
        Some(match kind {
            Kind::Face => Ref::Face {
                name: names.face_name(index)?.to_string(),
            },
            Kind::Edge => Ref::Edge {
                name: names.edges.get(index as usize)?.clone(),
            },
            Kind::Vertex => Ref::Vertex {
                name: names.vertices.get(index as usize)?.clone(),
            },
            Kind::Solid => return None,
        })
    }
}

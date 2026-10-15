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
    pub cosmetic_threads: HashMap<FeatureId, crate::CosmeticThread>,
    /// Cosmetic threads of tapped holes, per hole feature.
    pub hole_threads: HashMap<FeatureId, Vec<crate::cosmetic_thread::HoleThread>>,
    pub sketches3d: HashMap<FeatureId, (of_sketch::spatial::Sketch3D, SolveReport)>,
    pub curves: HashMap<FeatureId, crate::curves::CurveBody>,
    pub bodies: Vec<crate::SolidBody>,
    /// Derived aggregate cache for legacy feature operations and whole-part I/O.
    pub body: Option<Body>,
    pub datums: HashMap<FeatureId, Datum>,
    /// Each sketch's plane and solved geometry.
    pub sketches: HashMap<FeatureId, (Plane, Sketch)>,
    pub(crate) tools: HashMap<FeatureId, Vec<Tool>>,
    /// Sheet metal bodies, with their parameters and bends.
    pub sheet_metal: Vec<crate::sheet_metal::SheetMetalBody>,
    /// The part's material, for features that take defaults from it.
    pub material: Option<String>,
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
    /// The effective definition (after equations and the active configuration)
    /// and any equation error that `states[i]` was built from. A state is reused
    /// only while its input and every earlier input are unchanged.
    inputs: Vec<(Feature, Option<String>)>,
    /// Features built since the model was created, for cache diagnostics.
    builds: usize,
}

impl Model {
    pub fn new(doc: Document) -> Model {
        let mut m = Model {
            doc,
            states: Vec::new(),
            statuses: Vec::new(),
            empty: State::default(),
            inputs: Vec::new(),
            builds: 0,
        };
        m.rebuild();
        m
    }

    /// Forget cached results from feature `index` on. Rebuilding also discards
    /// any earlier state whose effective definition changed, such as a feature
    /// driven by an equation on the edited one.
    pub fn invalidate_from(&mut self, index: usize) {
        self.states.truncate(index);
        self.statuses.truncate(index);
        self.inputs.truncate(index);
    }

    /// How many features have been built since this model was created.
    pub fn build_count(&self) -> usize {
        self.builds
    }

    /// Drop cached states from the first one whose effective input changed.
    fn sync_cache(&mut self, evaluation: &crate::equations::Evaluation) {
        let features = &evaluation.document.features;
        let changed = self
            .inputs
            .iter()
            .enumerate()
            .find(|(i, (feature, error))| {
                features.get(*i).is_none_or(|current| {
                    current != feature || evaluation.errors.get(&current.id) != error.as_ref()
                })
            })
            .map(|(i, _)| i);
        if let Some(i) = changed {
            self.invalidate_from(i);
        }
    }

    /// Build feature `i` on top of the cached states and cache the result.
    fn build_next(&mut self, evaluation: &mut crate::equations::Evaluation) {
        let i = self.states.len();
        let prev = if i == 0 {
            State::default()
        } else {
            self.states[i - 1].clone()
        };
        let input = evaluation.document.features[i].clone();
        let error = evaluation.errors.get(&input.id).cloned();
        let (state, status) = evaluated_feature(evaluation, i, prev);
        self.builds += 1;
        self.states.push(state);
        self.statuses.push(status);
        self.inputs.push((input, error));
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
        self.sync_cache(&evaluation);
        while self.states.len() < end {
            let i = self.states.len();
            self.build_next(&mut evaluation);
            if self.doc.equations.is_empty()
                && self.doc.configurations == crate::configurations::Configurations::default()
            {
                self.doc.features[i].kind = evaluation.document.features[i].kind.clone();
            }
        }
        // An annotation follows its referenced cylinder through later body
        // operations. Surface removal or trimming must not leave a stale glyph.
        let annotation_errors: Vec<_> = self
            .state()
            .cosmetic_threads
            .iter()
            .map(|(id, thread)| {
                (
                    *id,
                    crate::cosmetic_thread::annotation(thread, self.state()).err(),
                )
            })
            .collect();
        for (id, error) in annotation_errors {
            if let Some(status) = self.statuses.iter_mut().find(|status| status.id == id) {
                status.error = error;
            }
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
        self.sync_cache(&evaluation);
        while self.states.len() < i {
            self.build_next(&mut evaluation);
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
        // A preview must expose the same persistent body/piece/curve identities
        // as its eventual commit. No ID is consumed; callers guard stale revisions.
        let id = editing.unwrap_or(self.doc.next_id);
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

fn build_feature(doc: &mut Document, index: usize, mut prev: State) -> (State, FeatureStatus) {
    let f: Feature = doc.features[index].clone();
    prev.material = doc.material.as_ref().map(|m| m.name.clone());
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
        FeatureKind::Sketch3d(s) => match build_spatial(&prev, s) {
            Ok((sketch, report)) => {
                let failed = report.status == of_sketch::SolveStatus::Failed;
                status.sketch = Some(report.clone());
                state.sketches3d.insert(f.id, (sketch.clone(), report));
                if failed {
                    Err("the 3D sketch can't be solved; check its conflicting relations".into())
                } else {
                    doc.features[index].kind = FeatureKind::Sketch3d(sketch);
                    Ok(())
                }
            }
            Err(e) => Err(e),
        },
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
        if !matches!(f.kind, FeatureKind::Sketch(_) | FeatureKind::Sketch3d(_)) {
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
            | FeatureKind::Dome(_)
            | FeatureKind::Rib(_)
            | FeatureKind::Thread(_)
            | FeatureKind::Wrap(_)
            | FeatureKind::Draft(_)
            | FeatureKind::VariableFillet(_)
            | FeatureKind::FaceFillet(_)
            | FeatureKind::Sweep(_)
            | FeatureKind::Loft(_)
            | FeatureKind::Boundary(_)
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
    if let Some(curve) = crate::curves::generated(kind, prev)? {
        state.curves.insert(id, curve);
        return Ok(());
    }
    let datum = match kind {
        FeatureKind::CosmeticThread(thread) => {
            crate::cosmetic_thread::annotation(thread, prev)?;
            state.cosmetic_threads.insert(id, thread.clone());
            return Ok(());
        }
        FeatureKind::Plane(p) => build_plane(prev, p)?,
        FeatureKind::Axis(a) => build_axis(prev, a)?,
        FeatureKind::Point(p) => build_point(prev, p)?,
        FeatureKind::CoordSys(c) => build_coord_sys(prev, c)?,
        FeatureKind::Sketch3d(s) => {
            let (sketch, report) = build_spatial(prev, s)?;
            if report.status == of_sketch::SolveStatus::Failed {
                return Err(
                    "the 3D sketch can't be solved; check its conflicting relations".into(),
                );
            }
            state.sketches3d.insert(id, (sketch, report));
            return Ok(());
        }
        FeatureKind::Sketch(s) => {
            let (plane, sketch, _) = build_sketch(prev, s)?;
            state.sketches.insert(id, (plane, sketch));
            return Ok(());
        }
        FeatureKind::Combine(p) => return crate::bodies::combine(id, p, prev, state),
        FeatureKind::MoveBody(p) => return crate::bodies::move_body(id, p, prev, state),
        FeatureKind::SplitBody(p) => return crate::bodies::split_body(id, p, prev, state),
        FeatureKind::FlatPattern(p) => return ops::sheet::flat_pattern(id, p, prev, state),
        FeatureKind::SketchedBend(p) => return ops::sheet::sketched_bend(id, p, prev, state),
        FeatureKind::Jog(p) => return ops::sheet::jog(id, p, prev, state),
        FeatureKind::CrossBreak(p) => return ops::sheet::cross_break(id, p, prev, state),
        _ => {
            ops::build(id, kind, prev, state)?;
            return state.refresh_bodies(id, prev);
        }
    };
    state.datums.insert(id, datum);
    Ok(())
}

fn build_spatial(
    prev: &State,
    s: &of_sketch::spatial::Sketch3D,
) -> RResult<(of_sketch::spatial::Sketch3D, SolveReport)> {
    let mut sketch = s.clone();
    update_spatial_external(prev, &mut sketch)?;
    let report = sketch.solve(None)?;
    Ok((sketch, report))
}

/// Move associative fixed points and workplanes to their model references.
fn update_spatial_external(prev: &State, sketch: &mut of_sketch::spatial::Sketch3D) -> RResult<()> {
    use of_sketch::spatial::Relation3;
    for constraint in &mut sketch.constraints {
        let Some(text) = &constraint.external else {
            continue;
        };
        let reference: Ref = serde_json::from_str(text)
            .map_err(|_| format!("relation {} has an invalid model reference", constraint.id))?;
        let missing = |_| {
            format!(
                "the model geometry that relation {} follows no longer exists",
                constraint.id
            )
        };
        let resolved = resolve(prev, &reference).map_err(missing)?;
        match &mut constraint.relation {
            Relation3::Fixed { at, .. } => {
                *at = resolved.point().map_err(|_| {
                    format!("relation {} must follow a vertex or point", constraint.id)
                })?;
            }
            Relation3::PointOnPlane { origin, normal, .. } => {
                let plane = resolved.plane().map_err(|_| {
                    format!(
                        "relation {} must follow a plane or planar face",
                        constraint.id
                    )
                })?;
                *origin = plane.origin;
                *normal = plane.normal;
            }
            _ => {
                return Err(format!(
                    "only fixed and point-on-plane relations can follow model geometry (relation {})",
                    constraint.id
                ));
            }
        }
    }
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

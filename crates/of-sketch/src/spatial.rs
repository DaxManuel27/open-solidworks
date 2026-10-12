//! Independent spatial sketch geometry and a transactional constraint solver.
//! Point IDs, not entity order, define connectivity and constraint ownership.

use crate::{Definition, Id, SolveReport, SolveStatus};
use glam::DVec3;
use nalgebra::{DMatrix, DVector};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Geometry3 {
    Point {
        #[schemars(with = "[f64; 3]")]
        at: DVec3,
    },
    Line {
        a: Id,
        b: Id,
    },
    Bezier {
        points: [Id; 4],
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Entity3 {
    pub id: Id,
    #[serde(flatten)]
    pub geometry: Geometry3,
    #[serde(default)]
    pub construction: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Axis3 {
    #[default]
    X,
    Y,
    Z,
}

impl Axis3 {
    fn index(self) -> usize {
        match self {
            Self::X => 0,
            Self::Y => 1,
            Self::Z => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Relation3 {
    Coincident {
        a: Id,
        b: Id,
    },
    Fixed {
        point: Id,
        #[schemars(with = "[f64; 3]")]
        at: DVec3,
    },
    Distance {
        a: Id,
        b: Id,
        value: f64,
    },
    Axis {
        line: Id,
        axis: Axis3,
    },
    Parallel {
        a: Id,
        b: Id,
    },
    Perpendicular {
        a: Id,
        b: Id,
    },
    PointOnPlane {
        point: Id,
        #[schemars(with = "[f64; 3]")]
        origin: DVec3,
        #[schemars(with = "[f64; 3]")]
        normal: DVec3,
    },
    /// Tangency at the selected ends, including positional coincidence.
    Tangent {
        a: Id,
        b: Id,
        #[serde(default)]
        end_a: bool,
        #[serde(default)]
        end_b: bool,
    },
    /// A point anywhere on a finite line or cubic Bezier segment.
    PointOnCurve {
        point: Id,
        curve: Id,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Constraint3 {
    pub id: Id,
    #[serde(flatten)]
    pub relation: Relation3,
    /// A model reference, as JSON, that sets this relation's location on every
    /// rebuild: a vertex or reference point for `fixed`, a plane or planar face
    /// for `point_on_plane`. The sketch then follows that geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Sketch3D {
    #[serde(default)]
    pub entities: Vec<Entity3>,
    #[serde(default)]
    pub constraints: Vec<Constraint3>,
    /// Monotonically increasing; deleted entity IDs must never be reused.
    pub next_id: Id,
}

impl Default for Sketch3D {
    fn default() -> Self {
        Self {
            entities: vec![],
            constraints: vec![],
            next_id: 1,
        }
    }
}

impl Sketch3D {
    pub fn add_point(&mut self, at: DVec3) -> Id {
        self.add(Geometry3::Point { at })
    }
    pub fn add(&mut self, geometry: Geometry3) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        self.entities.push(Entity3 {
            id,
            geometry,
            construction: false,
        });
        id
    }
    pub fn constrain(&mut self, relation: Relation3) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        self.constraints.push(Constraint3 {
            id,
            relation,
            external: None,
        });
        id
    }
    pub fn entity(&self, id: Id) -> Result<&Entity3, String> {
        self.entities
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| format!("no spatial entity {id}"))
    }
    pub fn point(&self, id: Id) -> Result<DVec3, String> {
        match self.entity(id)?.geometry {
            Geometry3::Point { at } => Ok(at),
            _ => Err(format!("spatial entity {id} is not a point")),
        }
    }
    pub fn point_ids(&self, id: Id) -> Result<Vec<Id>, String> {
        Ok(match &self.entity(id)?.geometry {
            Geometry3::Point { .. } => vec![id],
            Geometry3::Line { a, b } => vec![*a, *b],
            Geometry3::Bezier { points } => points.to_vec(),
        })
    }
    pub fn control_points(&self, id: Id) -> Result<Vec<DVec3>, String> {
        self.point_ids(id)?
            .into_iter()
            .map(|p| self.point(p))
            .collect()
    }
    pub fn sample(&self, id: Id, t: f64) -> Result<DVec3, String> {
        let p = self.control_points(id)?;
        Ok(match p.as_slice() {
            [a, b] => a.lerp(*b, t),
            [a, b, c, d] => {
                *a * (1.0 - t).powi(3)
                    + *b * (3.0 * t * (1.0 - t).powi(2))
                    + *c * (3.0 * t * t * (1.0 - t))
                    + *d * t.powi(3)
            }
            _ => return Err("select a spatial line or cubic spline".into()),
        })
    }
    fn endpoint(&self, id: Id, end: bool) -> Result<(DVec3, DVec3), String> {
        let p = self.control_points(id)?;
        if p.len() < 2 {
            return Err("select a spatial line or cubic spline".into());
        }
        let (at, tangent) = if end {
            (p[p.len() - 1], p[p.len() - 1] - p[p.len() - 2])
        } else {
            (p[0], p[1] - p[0])
        };
        if tangent.length() < 1e-12 {
            return Err("a spatial curve has a zero-length endpoint tangent".into());
        }
        Ok((at, tangent.normalize()))
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.next_id == 0 || self.next_id == Id::MAX {
            return Err("invalid or exhausted spatial sketch ID allocation".into());
        }
        let mut ids = BTreeSet::new();
        for id in self
            .entities
            .iter()
            .map(|e| e.id)
            .chain(self.constraints.iter().map(|c| c.id))
        {
            if id == 0 || id >= self.next_id || !ids.insert(id) {
                return Err("invalid or duplicate spatial sketch IDs".into());
            }
        }
        for e in &self.entities {
            match e.geometry {
                Geometry3::Point { at } if !at.is_finite() => {
                    return Err("spatial coordinates must be finite".into());
                }
                Geometry3::Point { .. } => {}
                _ => {
                    self.endpoint(e.id, false)?;
                    self.endpoint(e.id, true)?;
                    for p in self.point_ids(e.id)? {
                        self.point(p)?;
                    }
                }
            }
        }
        for c in &self.constraints {
            match &c.relation {
                Relation3::Distance { a, b, value } => {
                    if *a == *b || !value.is_finite() || *value <= 0.0 {
                        return Err(
                            "spatial distance must be positive and join distinct points".into()
                        );
                    }
                }
                Relation3::Fixed { at, .. } if !at.is_finite() => {
                    return Err("fixed coordinates must be finite".into());
                }
                Relation3::PointOnPlane { origin, normal, .. }
                    if !origin.is_finite() || !normal.is_finite() || normal.length() < 1e-12 =>
                {
                    return Err("point-on-plane requires a finite origin and nonzero normal".into());
                }
                _ => {}
            }
            let r = self.residual(&c.relation)?;
            if r.iter().any(|v| !v.is_finite()) {
                return Err("invalid spatial constraint".into());
            }
        }
        Ok(())
    }
    fn line_dir(&self, id: Id) -> Result<DVec3, String> {
        if !matches!(self.entity(id)?.geometry, Geometry3::Line { .. }) {
            return Err("axis, parallel and perpendicular relations require lines".into());
        }
        Ok(self.endpoint(id, false)?.1)
    }
    fn residual(&self, c: &Relation3) -> Result<Vec<f64>, String> {
        Ok(match *c {
            Relation3::Coincident { a, b } => (self.point(a)? - self.point(b)?).to_array().to_vec(),
            Relation3::Fixed { point, at } => (self.point(point)? - at).to_array().to_vec(),
            Relation3::Distance { a, b, value } => {
                vec![self.point(a)?.distance(self.point(b)?) - value]
            }
            Relation3::Axis { line, axis } => {
                self.line_dir(line)?;
                let points = self.control_points(line)?;
                let d = points[1] - points[0];
                (0..3)
                    .filter(|i| *i != axis.index())
                    .map(|i| d[i])
                    .collect()
            }
            Relation3::Parallel { a, b } => self
                .line_dir(a)?
                .cross(self.line_dir(b)?)
                .to_array()
                .to_vec(),
            Relation3::Perpendicular { a, b } => vec![self.line_dir(a)?.dot(self.line_dir(b)?)],
            Relation3::PointOnPlane {
                point,
                origin,
                normal,
            } => vec![(self.point(point)? - origin).dot(normal.normalize())],
            Relation3::Tangent { a, b, end_a, end_b } => {
                let (pa, da) = self.endpoint(a, end_a)?;
                let (pb, db) = self.endpoint(b, end_b)?;
                (pa - pb)
                    .to_array()
                    .into_iter()
                    .chain(da.cross(db).to_array())
                    .chain([da.dot(db) - if end_a == end_b { -1.0 } else { 1.0 }])
                    .collect()
            }
            Relation3::PointOnCurve { point, curve } => {
                let p = self.point(point)?;
                // Bracket every local minimum, then refine the best. This avoids
                // binding the solver to an arbitrary tessellation sample.
                let mut best = (f64::INFINITY, DVec3::ZERO);
                for i in 0..32 {
                    let (mut lo, mut hi) = (i as f64 / 32.0, (i + 1) as f64 / 32.0);
                    for _ in 0..72 {
                        let a = lo + (hi - lo) / 3.0;
                        let b = hi - (hi - lo) / 3.0;
                        if self.sample(curve, a)?.distance_squared(p)
                            < self.sample(curve, b)?.distance_squared(p)
                        {
                            hi = b;
                        } else {
                            lo = a;
                        }
                    }
                    for t in [i as f64 / 32.0, (lo + hi) * 0.5, (i + 1) as f64 / 32.0] {
                        let q = self.sample(curve, t)?;
                        let d = q.distance_squared(p);
                        if d < best.0 {
                            best = (d, q);
                        }
                    }
                }
                (p - best.1).to_array().to_vec()
            }
        })
    }
    /// Hard constraints win over a cursor target. Failure leaves all geometry intact.
    pub fn solve(&mut self, drag: Option<(Id, DVec3)>) -> Result<SolveReport, String> {
        self.validate()?;
        if let Some((id, p)) = drag {
            self.point(id)?;
            if !p.is_finite() {
                return Err("drag coordinates must be finite".into());
            }
        }
        let sys = System3::new(self);
        let mut x = sys.initial.clone();
        if drag.is_some() {
            sys.iterate(&mut x, drag, 50);
        }
        let ok = sys.iterate(&mut x, None, 200);
        let solved = sys.with_points(&x);
        let mut report = sys.report(&x);
        if ok && report.status == SolveStatus::Solved && solved.validate().is_ok() {
            *self = solved;
        } else {
            report.status = SolveStatus::Failed;
        }
        Ok(report)
    }
}

struct System3<'a> {
    sketch: &'a Sketch3D,
    points: Vec<Id>,
    initial: Vec<f64>,
    curve_parameters: BTreeMap<Id, usize>,
    tolerance: f64,
}
impl<'a> System3<'a> {
    fn new(sketch: &'a Sketch3D) -> Self {
        let points: Vec<_> = sketch
            .entities
            .iter()
            .filter_map(|e| matches!(e.geometry, Geometry3::Point { .. }).then_some(e.id))
            .collect();
        let mut initial: Vec<_> = points
            .iter()
            .flat_map(|id| sketch.point(*id).unwrap().to_array())
            .collect();
        let scale = initial.iter().fold(1.0_f64, |s, v| s.max(v.abs()));
        let mut curve_parameters = BTreeMap::new();
        for c in &sketch.constraints {
            if let Relation3::PointOnCurve { point, curve } = c.relation {
                let p = sketch.point(point).unwrap();
                let t = (0..=128)
                    .map(|i| i as f64 / 128.0)
                    .min_by(|a, b| {
                        sketch
                            .sample(curve, *a)
                            .unwrap()
                            .distance_squared(p)
                            .total_cmp(&sketch.sample(curve, *b).unwrap().distance_squared(p))
                    })
                    .unwrap();
                curve_parameters.insert(c.id, initial.len());
                initial.push(t);
            }
        }
        Self {
            sketch,
            points,
            initial,
            curve_parameters,
            tolerance: 1e-9 * scale.sqrt(),
        }
    }
    fn with_points(&self, x: &[f64]) -> Sketch3D {
        let mut sk = self.sketch.clone();
        for e in &mut sk.entities {
            if let Some(i) = self.points.iter().position(|id| *id == e.id) {
                e.geometry = Geometry3::Point {
                    at: DVec3::from_slice(&x[i * 3..i * 3 + 3]),
                };
            }
        }
        sk
    }
    fn residual(&self, x: &[f64], drag: Option<(Id, DVec3)>) -> DVector<f64> {
        let sk = self.with_points(x);
        let mut r: Vec<_> = sk
            .constraints
            .iter()
            .flat_map(|c| {
                if let Relation3::PointOnCurve { point, curve } = c.relation {
                    let t = x[self.curve_parameters[&c.id]];
                    (sk.point(point).unwrap() - sk.sample(curve, t).unwrap())
                        .to_array()
                        .to_vec()
                } else {
                    sk.residual(&c.relation).unwrap_or_else(|_| vec![1e20])
                }
            })
            .collect();
        if let Some((id, p)) = drag {
            r.extend(((sk.point(id).unwrap() - p) * 0.1).to_array());
        }
        DVector::from_vec(r)
    }
    fn jacobian(&self, x: &[f64], drag: Option<(Id, DVec3)>) -> DMatrix<f64> {
        let rows = self.residual(x, drag).len();
        let mut j = DMatrix::zeros(rows, x.len());
        let mut xs = x.to_vec();
        for k in 0..x.len() {
            let h = 1e-7 * x[k].abs().max(1.0);
            xs[k] = x[k] + h;
            let p = self.residual(&xs, drag);
            xs[k] = x[k] - h;
            let m = self.residual(&xs, drag);
            xs[k] = x[k];
            if p.len() == rows && m.len() == rows {
                for i in 0..rows {
                    j[(i, k)] = (p[i] - m[i]) / (2.0 * h);
                }
            }
        }
        j
    }
    fn iterate(&self, x: &mut [f64], drag: Option<(Id, DVec3)>, iterations: usize) -> bool {
        let mut r = self.residual(x, drag);
        let mut lambda = 1e-6;
        for _ in 0..iterations {
            if r.amax() < self.tolerance {
                return true;
            }
            if x.is_empty() {
                return false;
            }
            let j = self.jacobian(x, drag);
            let jt = j.transpose();
            let g = &jt * &r;
            let a = &jt * &j;
            let mut improved = false;
            for _ in 0..12 {
                let mut damped = a.clone();
                for k in 0..x.len() {
                    damped[(k, k)] += lambda;
                }
                let Some(step) = damped.cholesky().map(|c| c.solve(&g)) else {
                    lambda *= 10.0;
                    continue;
                };
                let mut trial: Vec<_> = x.iter().zip(step.iter()).map(|(p, d)| p - d).collect();
                for &i in self.curve_parameters.values() {
                    trial[i] = trial[i].clamp(0.0, 1.0);
                }
                let rt = self.residual(&trial, drag);
                if trial.iter().all(|v| v.is_finite()) && rt.norm_squared() < r.norm_squared() {
                    x.copy_from_slice(&trial);
                    r = rt;
                    lambda = (lambda / 3.0).max(1e-15);
                    improved = true;
                    break;
                }
                lambda *= 4.0;
            }
            if !improved {
                break;
            }
        }
        r.amax() < self.tolerance
    }
    fn report(&self, x: &[f64]) -> SolveReport {
        let j = self.jacobian(x, None);
        let sk = self.with_points(x);
        let mut basis: Vec<DVector<f64>> = vec![];
        let mut row = 0;
        let mut conflicting = vec![];
        let mut redundant = vec![];
        for c in &sk.constraints {
            let residual = sk.residual(&c.relation).unwrap_or_else(|_| vec![1e20]);
            let mut independent = false;
            for _ in &residual {
                let mut v = j.row(row).transpose();
                row += 1;
                let norm = v.norm();
                if norm < 1e-12 {
                    continue;
                }
                v /= norm;
                for _ in 0..2 {
                    for q in &basis {
                        let d = v.dot(q);
                        v -= q * d;
                    }
                }
                let norm = v.norm();
                if norm > 1e-6 {
                    basis.push(v / norm);
                    independent = true;
                }
            }
            if residual.iter().any(|v| v.abs() > self.tolerance * 10.0) {
                conflicting.push(c.id);
            } else if !independent {
                redundant.push(c.id);
            }
        }
        // Point-on-curve introduces a curve parameter as a solver unknown. A
        // stationary tangent can free that parameter while every geometric point
        // remains fixed, so auxiliary-only freedom must not be reported as DOF.
        let auxiliary_rank = self
            .curve_parameters
            .values()
            .filter(|&&column| {
                let norm_squared: f64 = (0..j.nrows())
                    .map(|row| {
                        let norm = j.row(row).norm();
                        if norm > 1e-12 {
                            (j[(row, column)] / norm).powi(2)
                        } else {
                            0.0
                        }
                    })
                    .sum();
                norm_squared > 1e-12
            })
            .count();
        let dof = (self.points.len() * 3 + auxiliary_rank).saturating_sub(basis.len());
        let mut over = BTreeSet::new();
        for c in &sk.constraints {
            if !conflicting.contains(&c.id) && !redundant.contains(&c.id) {
                continue;
            }
            let refs = match c.relation {
                Relation3::Coincident { a, b }
                | Relation3::Distance { a, b, .. }
                | Relation3::Parallel { a, b }
                | Relation3::Perpendicular { a, b }
                | Relation3::Tangent { a, b, .. } => vec![a, b],
                Relation3::Fixed { point, .. } | Relation3::PointOnPlane { point, .. } => {
                    vec![point]
                }
                Relation3::Axis { line, .. } => vec![line],
                Relation3::PointOnCurve { point, curve } => vec![point, curve],
            };
            for id in refs {
                over.insert(id);
                over.extend(sk.point_ids(id).unwrap_or_default());
            }
        }
        let free: Vec<_> = (0..x.len())
            .map(|i| dof > 0 && 1.0 - basis.iter().map(|q| q[i] * q[i]).sum::<f64>() > 1e-8)
            .collect();
        let entities: BTreeMap<_, _> = sk
            .entities
            .iter()
            .map(|e| {
                let under = sk.point_ids(e.id).unwrap_or_default().iter().any(|id| {
                    self.points
                        .iter()
                        .position(|p| p == id)
                        .is_some_and(|i| free[i * 3..i * 3 + 3].iter().any(|v| *v))
                });
                (
                    e.id,
                    if over.contains(&e.id) {
                        Definition::Over
                    } else if under {
                        Definition::Under
                    } else {
                        Definition::Fully
                    },
                )
            })
            .collect();
        SolveReport {
            status: if conflicting.is_empty() {
                SolveStatus::Solved
            } else {
                SolveStatus::Failed
            },
            dof,
            entities,
            conflicting,
            redundant,
        }
    }
}

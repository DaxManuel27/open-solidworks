//! Constraint solving and degrees-of-freedom analysis.
//!
//! Unknowns are point coordinates and circle radii. The origin and external
//! (converted) geometry are constants. Equations come from driving
//! constraints plus one per arc keeping its endpoints on the same radius.
//! Steps are damped least-squares (Levenberg–Marquardt) steps, which move the
//! geometry as little as possible, so under-defined geometry stays where the
//! user drew it.

use std::collections::{BTreeMap, HashMap};

use glam::DVec2;
use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};

use crate::constraint::Geo;
use crate::{Geometry, Id, ORIGIN, Sketch};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolveStatus {
    /// All driving constraints are satisfied.
    Solved,
    /// The constraints can't all be satisfied; geometry was left unchanged.
    Failed,
}

/// How well an entity is pinned down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Definition {
    Under,
    Fully,
    /// Involved in a conflicting or redundant constraint.
    Over,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolveReport {
    pub status: SolveStatus,
    /// Remaining degrees of freedom.
    pub dof: usize,
    pub entities: BTreeMap<Id, Definition>,
    /// Constraints that can't be satisfied together with the others.
    pub conflicting: Vec<Id>,
    /// Constraints already implied by the others.
    pub redundant: Vec<Id>,
}

impl SolveReport {
    /// The sketch status as shown in the status bar.
    pub fn summary(&self) -> &'static str {
        if self.status == SolveStatus::Failed
            || !self.conflicting.is_empty()
            || !self.redundant.is_empty()
        {
            "Over defined"
        } else if self.dof == 0 {
            "Fully defined"
        } else {
            "Under defined"
        }
    }
}

const TOL: f64 = 1e-9;

/// Where each unknown lives in the parameter vector.
struct System<'a> {
    sk: &'a Sketch,
    point_idx: HashMap<Id, usize>,
    radius_idx: HashMap<Id, usize>,
    x0: Vec<f64>,
    eqs: Vec<Equation>,
}

enum Source {
    Constraint(Id),
    Arc(Id),
    Drag(Id, DVec2),
}

struct Equation {
    source: Source,
    /// Parameter indices the equation depends on.
    deps: Vec<usize>,
    rows: usize,
}

struct ParamGeo<'a, 'b> {
    sys: &'b System<'a>,
    x: &'b [f64],
}

impl Geo for ParamGeo<'_, '_> {
    fn pt(&self, id: Id) -> DVec2 {
        match self.sys.point_idx.get(&id) {
            Some(&i) => DVec2::new(self.x[i], self.x[i + 1]),
            None => self.sys.sk.point(id).unwrap_or_default(),
        }
    }
    fn radius(&self, id: Id) -> f64 {
        match &self.sys.sk.entities[&id].geo {
            Geometry::Circle { radius, .. } => match self.sys.radius_idx.get(&id) {
                Some(&i) => self.x[i],
                None => *radius,
            },
            Geometry::Arc { center, start, .. } => (self.pt(*start) - self.pt(*center)).length(),
            _ => 0.0,
        }
    }
    fn sketch(&self) -> &Sketch {
        self.sys.sk
    }
}

/// Points owned by external geometry are held fixed.
fn fixed_points(sk: &Sketch) -> Vec<Id> {
    let mut v = vec![ORIGIN];
    for e in sk.entities.values() {
        if e.external.is_some() {
            v.extend(e.point_refs());
        }
    }
    v
}

impl<'a> System<'a> {
    fn new(sk: &'a Sketch, drags: &[(Id, DVec2)]) -> System<'a> {
        let fixed = fixed_points(sk);
        let mut point_idx = HashMap::new();
        let mut radius_idx = HashMap::new();
        let mut x0 = Vec::new();
        for (id, e) in &sk.entities {
            match &e.geo {
                Geometry::Point { p } if !fixed.contains(id) && e.external.is_none() => {
                    point_idx.insert(*id, x0.len());
                    x0.extend([p.x, p.y]);
                }
                Geometry::Circle { radius, .. } if e.external.is_none() => {
                    radius_idx.insert(*id, x0.len());
                    x0.push(*radius);
                }
                _ => {}
            }
        }
        let mut sys = System {
            sk,
            point_idx,
            radius_idx,
            x0,
            eqs: Vec::new(),
        };
        for (id, e) in &sk.entities {
            if let Geometry::Arc { .. } = e.geo {
                let deps = sys.deps(&[*id]);
                sys.eqs.push(Equation {
                    source: Source::Arc(*id),
                    deps,
                    rows: 1,
                });
            }
        }
        for (id, c) in &sk.constraints {
            if !c.driving {
                continue;
            }
            let rows = c.kind.equation_count(sk);
            if rows == 0 {
                continue;
            }
            let deps = sys.deps(&c.kind.refs());
            sys.eqs.push(Equation {
                source: Source::Constraint(*id),
                deps,
                rows,
            });
        }
        for (p, t) in drags {
            if sys.point_idx.contains_key(p) {
                let deps = sys.deps(&[*p]);
                sys.eqs.push(Equation {
                    source: Source::Drag(*p, *t),
                    deps,
                    rows: 2,
                });
            }
        }
        sys
    }

    /// Parameter indices used by the given entities.
    fn deps(&self, ids: &[Id]) -> Vec<usize> {
        let mut out = Vec::new();
        for id in ids {
            let Some(e) = self.sk.entities.get(id) else {
                continue;
            };
            if let Some(&i) = self.point_idx.get(id) {
                out.extend([i, i + 1]);
            }
            if let Some(&i) = self.radius_idx.get(id) {
                out.push(i);
            }
            for p in e.point_refs() {
                if let Some(&i) = self.point_idx.get(&p) {
                    out.extend([i, i + 1]);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    fn eval(&self, eq: &Equation, x: &[f64], out: &mut Vec<f64>) {
        let g = ParamGeo { sys: self, x };
        match &eq.source {
            Source::Constraint(id) => self.sk.constraints[id].kind.residuals(&g, out),
            Source::Arc(id) => {
                if let Geometry::Arc { center, start, end } = self.sk.entities[id].geo {
                    let c = g.pt(center);
                    out.push((g.pt(start) - c).length() - (g.pt(end) - c).length());
                }
            }
            Source::Drag(p, t) => {
                // Weak pull so constraints win over the cursor.
                let d = (g.pt(*p) - *t) * 0.1;
                out.extend([d.x, d.y]);
            }
        }
    }

    fn residual(&self, eqs: &[&Equation], x: &[f64]) -> DVector<f64> {
        let mut out = Vec::new();
        for eq in eqs {
            self.eval(eq, x, &mut out);
        }
        DVector::from_vec(out)
    }

    fn jacobian(&self, eqs: &[&Equation], x: &[f64]) -> DMatrix<f64> {
        let m: usize = eqs.iter().map(|e| e.rows).sum();
        let mut j = DMatrix::zeros(m, x.len());
        let mut xs = x.to_vec();
        let mut row = 0;
        let (mut plus, mut minus) = (Vec::new(), Vec::new());
        for eq in eqs {
            for &k in &eq.deps {
                let h = 1e-7 * x[k].abs().max(1.0);
                xs[k] = x[k] + h;
                plus.clear();
                self.eval(eq, &xs, &mut plus);
                xs[k] = x[k] - h;
                minus.clear();
                self.eval(eq, &xs, &mut minus);
                xs[k] = x[k];
                for r in 0..eq.rows {
                    j[(row + r, k)] = (plus[r] - minus[r]) / (2.0 * h);
                }
            }
            row += eq.rows;
        }
        j
    }

    /// Damped least squares. Returns true when the residual reaches zero.
    fn least_squares(&self, eqs: &[&Equation], x: &mut [f64], iters: usize) -> bool {
        let n = x.len();
        if n == 0 || eqs.is_empty() {
            return self.residual(eqs, x).amax() < TOL;
        }
        let mut r = self.residual(eqs, x);
        let mut cost = r.norm_squared();
        let mut lambda = 1e-6;
        for _ in 0..iters {
            if r.amax() < TOL {
                return true;
            }
            let j = self.jacobian(eqs, x);
            let jt = j.transpose();
            let g = &jt * &r;
            let mut a = &jt * &j;
            let mut improved = false;
            for _ in 0..12 {
                for i in 0..n {
                    a[(i, i)] += lambda;
                }
                let step = a.clone().cholesky().map(|c| c.solve(&g));
                for i in 0..n {
                    a[(i, i)] -= lambda;
                }
                let Some(step) = step else {
                    lambda *= 10.0;
                    continue;
                };
                let trial: Vec<f64> = x.iter().zip(step.iter()).map(|(a, s)| a - s).collect();
                let rt = self.residual(eqs, &trial);
                let ct = rt.norm_squared();
                if ct < cost {
                    x.copy_from_slice(&trial);
                    r = rt;
                    let small_step = step.amax() < 1e-14;
                    cost = ct;
                    lambda = (lambda / 3.0).max(1e-15);
                    improved = !small_step;
                    break;
                }
                lambda *= 4.0;
            }
            if !improved {
                break;
            }
        }
        r.amax() < TOL
    }

    fn write_back(&self, sk: &mut Sketch, x: &[f64]) {
        for (id, &i) in &self.point_idx {
            if let Some(e) = sk.entities.get_mut(id)
                && let Geometry::Point { p } = &mut e.geo
            {
                *p = DVec2::new(x[i], x[i + 1]);
            }
        }
        for (id, &i) in &self.radius_idx {
            if let Some(e) = sk.entities.get_mut(id)
                && let Geometry::Circle { radius, .. } = &mut e.geo
            {
                *radius = x[i];
            }
        }
    }
}

pub(crate) fn solve(sk: &mut Sketch, drags: &[(Id, DVec2)]) -> SolveReport {
    let snapshot = sk.clone();
    let sys = System::new(&snapshot, drags);
    let hard: Vec<&Equation> = sys
        .eqs
        .iter()
        .filter(|e| !matches!(e.source, Source::Drag(..)))
        .collect();
    let mut x = sys.x0.clone();
    if !drags.is_empty() {
        let all: Vec<&Equation> = sys.eqs.iter().collect();
        sys.least_squares(&all, &mut x, 50);
    }
    let ok = sys.least_squares(&hard, &mut x, 200);
    if ok {
        sys.write_back(sk, &x);
        drop(sys);
        return analyse(sk);
    }
    // Leave geometry as it was and report what's in conflict.
    let mut report = analyse_with(&sys, &hard, &x);
    report.status = SolveStatus::Failed;
    report
}

pub(crate) fn analyse(sk: &Sketch) -> SolveReport {
    let sys = System::new(sk, &[]);
    let eqs: Vec<&Equation> = sys.eqs.iter().collect();
    let x = sys.x0.clone();
    let solved = sys.residual(&eqs, &x).amax() < 1e-6;
    let mut report = analyse_with(&sys, &eqs, &x);
    if !solved {
        report.status = SolveStatus::Failed;
    }
    report
}

fn analyse_with(sys: &System, eqs: &[&Equation], x: &[f64]) -> SolveReport {
    let n = x.len();
    let j = sys.jacobian(eqs, x);

    // Rows that add nothing new are redundant (if satisfied) or conflicting.
    let mut basis: Vec<DVector<f64>> = Vec::new();
    let (mut redundant, mut conflicting) = (Vec::new(), Vec::new());
    let mut row = 0;
    for eq in eqs {
        let mut independent = false;
        for r in 0..eq.rows {
            let mut v: DVector<f64> = j.row(row + r).transpose();
            let norm = v.norm();
            if norm < 1e-12 {
                continue;
            }
            v /= norm;
            for q in &basis {
                let d = v.dot(q);
                v -= q * d;
            }
            if v.norm() > 1e-6 {
                let len = v.norm();
                basis.push(v / len);
                independent = true;
            }
        }
        if let Source::Constraint(id) = eq.source
            && !independent
        {
            let mut out = Vec::new();
            sys.eval(eq, x, &mut out);
            if out.iter().all(|r| r.abs() < 1e-6) {
                redundant.push(id);
            } else {
                conflicting.push(id);
            }
        }
        row += eq.rows;
    }
    let rank = basis.len();
    let dof = n.saturating_sub(rank);

    // A parameter is free if it moves in some direction the constraints allow.
    let mut free = vec![false; n];
    if dof > 0 {
        let jtj = j.transpose() * &j;
        let eig = jtj.symmetric_eigen();
        let scale = eig.eigenvalues.amax().max(1.0);
        for (k, &ev) in eig.eigenvalues.iter().enumerate() {
            if ev.abs() < 1e-9 * scale {
                for (i, f) in free.iter_mut().enumerate() {
                    if eig.eigenvectors[(i, k)].abs() > 1e-6 {
                        *f = true;
                    }
                }
            }
        }
    }

    let over: Vec<Id> = conflicting
        .iter()
        .chain(&redundant)
        .flat_map(|c| sys.sk.constraints[c].kind.refs())
        .collect();
    let mut entities = BTreeMap::new();
    for (id, e) in &sys.sk.entities {
        let mut params = sys.deps(&[*id]);
        if let Some(&i) = sys.point_idx.get(id) {
            params.extend([i, i + 1]);
        }
        let status = if over.contains(id) {
            Definition::Over
        } else if params.iter().any(|&i| free[i]) {
            Definition::Under
        } else {
            Definition::Fully
        };
        let _ = e;
        entities.insert(*id, status);
    }

    SolveReport {
        status: SolveStatus::Solved,
        dof,
        entities,
        conflicting,
        redundant,
    }
}

#[cfg(test)]
mod tests {
    use crate::{ConstraintKind as K, Definition, Sketch, SolveStatus};
    use glam::DVec2;

    fn rectangle(sk: &mut Sketch) -> ([u32; 4], [u32; 4]) {
        let p = [
            sk.add_point(DVec2::new(0.1, -0.2)),
            sk.add_point(DVec2::new(9.0, 0.3)),
            sk.add_point(DVec2::new(8.7, 5.2)),
            sk.add_point(DVec2::new(-0.3, 4.9)),
        ];
        let l = [
            sk.add_line(p[0], p[1]).unwrap(),
            sk.add_line(p[1], p[2]).unwrap(),
            sk.add_line(p[2], p[3]).unwrap(),
            sk.add_line(p[3], p[0]).unwrap(),
        ];
        sk.add_constraint(K::Horizontal { line: l[0] }).unwrap();
        sk.add_constraint(K::Horizontal { line: l[2] }).unwrap();
        sk.add_constraint(K::Vertical { line: l[1] }).unwrap();
        sk.add_constraint(K::Vertical { line: l[3] }).unwrap();
        (p, l)
    }

    #[test]
    fn rectangle_becomes_fully_defined() {
        let mut sk = Sketch::new();
        let (p, l) = rectangle(&mut sk);
        let r = sk.solve();
        assert_eq!(r.status, SolveStatus::Solved);
        assert_eq!(r.dof, 4);
        assert_eq!(r.summary(), "Under defined");

        sk.add_constraint(K::Coincident {
            a: p[0],
            b: crate::ORIGIN,
        })
        .unwrap();
        sk.add_constraint(K::Length {
            line: l[0],
            value: 20.0,
        })
        .unwrap();
        sk.add_constraint(K::Length {
            line: l[1],
            value: 10.0,
        })
        .unwrap();
        let r = sk.solve();
        assert_eq!(r.status, SolveStatus::Solved);
        assert_eq!(r.dof, 0, "{r:?}");
        assert_eq!(r.summary(), "Fully defined");
        assert!(r.entities.values().all(|d| *d == Definition::Fully));
        let c = sk.point(p[2]).unwrap();
        assert!((c - DVec2::new(20.0, 10.0)).length() < 1e-7, "{c}");
    }

    #[test]
    fn under_defined_geometry_barely_moves() {
        let mut sk = Sketch::new();
        let (p, _) = rectangle(&mut sk);
        sk.solve();
        // Squaring the corners moves points by fractions of a millimetre.
        let a = sk.point(p[1]).unwrap();
        assert!((a - DVec2::new(9.0, 0.3)).length() < 0.6, "{a}");
    }

    #[test]
    fn redundant_and_conflicting_dimensions_are_reported() {
        let mut sk = Sketch::new();
        let (_, l) = rectangle(&mut sk);
        sk.add_constraint(K::Length {
            line: l[0],
            value: 10.0,
        })
        .unwrap();
        sk.solve();
        let eq = sk.add_constraint(K::Equal { a: l[0], b: l[2] }).unwrap();
        let r = sk.solve();
        assert_eq!(r.redundant, vec![eq]);

        sk.delete(&[eq]);
        let bad = sk
            .add_constraint(K::Length {
                line: l[2],
                value: 12.0,
            })
            .unwrap();
        let r = sk.solve();
        assert_eq!(r.status, SolveStatus::Failed);
        assert!(r.conflicting.contains(&bad), "{r:?}");
    }

    #[test]
    fn tangent_arc_and_radius() {
        let mut sk = Sketch::new();
        let (line, a, b) = sk.add_line_at(DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0));
        sk.add_constraint(K::Horizontal { line }).unwrap();
        sk.add_constraint(K::Coincident {
            a,
            b: crate::ORIGIN,
        })
        .unwrap();
        let c = sk.add_point(DVec2::new(10.0, 3.0));
        let e = sk.add_point(DVec2::new(13.0, 3.5));
        let arc = sk.add_arc(c, b, e).unwrap();
        sk.add_constraint(K::Tangent {
            a: line,
            b: arc,
            inside: false,
        })
        .unwrap();
        sk.add_constraint(K::Radius {
            curve: arc,
            value: 4.0,
        })
        .unwrap();
        let r = sk.solve();
        assert_eq!(r.status, SolveStatus::Solved, "{r:?}");
        let (center, radius) = sk.circle(arc).unwrap();
        assert!((radius - 4.0).abs() < 1e-7);
        assert!((center.y.abs() - 4.0).abs() < 1e-7);
        assert!((sk.point(e).unwrap() - center).length() - 4.0 < 1e-7);
    }

    #[test]
    fn drag_respects_constraints() {
        let mut sk = Sketch::new();
        let (line, a, b) = sk.add_line_at(DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0));
        sk.add_constraint(K::Horizontal { line }).unwrap();
        sk.add_constraint(K::Coincident {
            a,
            b: crate::ORIGIN,
        })
        .unwrap();
        sk.solve();
        sk.drag(b, DVec2::new(15.0, 5.0));
        let p = sk.point(b).unwrap();
        assert!(p.y.abs() < 1e-7, "{p}");
        assert!((p.x - 15.0).abs() < 0.5, "{p}");
    }

    #[test]
    fn angle_and_symmetry() {
        let mut sk = Sketch::new();
        let (l1, a, _) = sk.add_line_at(DVec2::ZERO, DVec2::new(10.0, 0.0));
        let (l2, c, _) = sk.add_line_at(DVec2::ZERO, DVec2::new(5.0, 8.0));
        sk.add_constraint(K::Coincident { a, b: c }).unwrap();
        sk.add_constraint(K::Horizontal { line: l1 }).unwrap();
        sk.add_constraint(K::Angle {
            a: l1,
            b: l2,
            value: 30f64.to_radians(),
        })
        .unwrap();
        assert_eq!(sk.solve().status, SolveStatus::Solved);
        let (p, q) = sk.line(l2).unwrap();
        let ang = (q - p).y.atan2((q - p).x);
        assert!((ang - 30f64.to_radians()).abs() < 1e-7);

        let p1 = sk.add_point(DVec2::new(-3.0, 1.0));
        let p2 = sk.add_point(DVec2::new(2.0, -1.5));
        sk.add_constraint(K::Symmetric {
            a: p1,
            b: p2,
            about: l1,
        })
        .unwrap();
        assert_eq!(sk.solve().status, SolveStatus::Solved);
        let (u, v) = (sk.point(p1).unwrap(), sk.point(p2).unwrap());
        let axis_y = sk.line(l1).unwrap().0.y;
        assert!(
            (u.x - v.x).abs() < 1e-7 && ((u.y + v.y) / 2.0 - axis_y).abs() < 1e-7,
            "{u} {v}"
        );
    }

    #[test]
    fn construction_and_fix() {
        let mut sk = Sketch::new();
        let circle = sk.add_circle_at(DVec2::new(3.0, 4.0), 2.0);
        sk.add_constraint(K::Fix {
            entity: circle,
            at: vec![],
        })
        .unwrap();
        let r = sk.solve();
        assert_eq!(r.dof, 0);
        assert_eq!(sk.circle(circle).unwrap(), (DVec2::new(3.0, 4.0), 2.0));
    }
}

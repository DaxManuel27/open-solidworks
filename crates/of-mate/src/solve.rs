//! Damped least squares over rigid poses, solved per connected group of
//! floating bodies, with drag targets and diagnostics.

use std::collections::BTreeMap;

use glam::DVec3;
use nalgebra::{DMatrix, DVector};

use crate::eval::{self, Poses};
use crate::sparse::{Profile, Rows};
use crate::{Pose, System, Term};

/// Pull a body toward a target while solving. Mates always win.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Drag {
    /// Move the body so a local point goes toward a target point.
    Point {
        body: usize,
        local: DVec3,
        target: DVec3,
    },
    /// Move the body toward a whole pose (translations and rotations).
    Pose { body: usize, pose: Pose },
}

impl Drag {
    fn body(&self) -> usize {
        match self {
            Drag::Point { body, .. } | Drag::Pose { body, .. } => *body,
        }
    }
}

/// What a solve found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    /// Every mate holds. When false, the failing groups kept their poses.
    pub solved: bool,
    /// Remaining degrees of freedom per body (0 for fixed bodies). Coupled
    /// bodies each report the motion they share.
    pub dof: Vec<u32>,
    /// Independent motions left in the whole assembly.
    pub free: u32,
    /// Mates that do not hold.
    pub failed: Vec<u32>,
    /// A minimal set of mates that cannot hold together.
    pub conflicting: Vec<u32>,
    /// Mates that hold but add no constraint beyond the others.
    pub redundant: Vec<u32>,
    /// Coupler motion values after the solve (unwrapped), by mate ID.
    pub couplers: BTreeMap<u32, Vec<f64>>,
}

/// How hard to try.
const ITERS: usize = 200;
const DRAG_ITERS: usize = 60;
/// Largest group checked mate by mate for a minimal conflict set.
const CONFLICT_SEARCH_LIMIT: usize = 48;

/// One equation: which mate and term it comes from.
#[derive(Clone, Copy)]
struct Eq<'a> {
    mate: usize,
    term: &'a Term,
    rows: usize,
}

/// The working problem for one group of floating bodies.
struct Group<'a> {
    /// Body indices solved here; parameter block `6 * i` belongs to `bodies[i]`.
    bodies: Vec<usize>,
    eqs: Vec<Eq<'a>>,
    scale: f64,
    tol: f64,
}

impl Group<'_> {
    fn column(&self, body: usize) -> Option<usize> {
        self.bodies.iter().position(|b| *b == body).map(|i| 6 * i)
    }

    fn n(&self) -> usize {
        6 * self.bodies.len()
    }

    /// Bodies sharing an equation, by group position.
    fn adjacency(&self) -> Vec<Vec<usize>> {
        let mut adj = vec![Vec::new(); self.bodies.len()];
        for eq in &self.eqs {
            let cols: Vec<usize> = eq
                .term
                .bodies()
                .into_iter()
                .filter_map(|b| self.column(b).map(|c| c / 6))
                .collect();
            for a in &cols {
                for b in &cols {
                    if a != b && !adj[*a].contains(b) {
                        adj[*a].push(*b);
                    }
                }
            }
        }
        adj
    }

    fn residual(&self, eqs: &[Eq], drags: &[Drag], poses: &[Pose]) -> Vec<f64> {
        let p = Poses::new(poses);
        let mut out = Vec::new();
        for eq in eqs {
            eval::residuals(&p, eq.term, self.scale, &mut out);
        }
        for d in drags {
            drag_rows(&p, d, self.scale, &mut out);
        }
        out
    }

    /// Sparse Jacobian with respect to small pose steps; rotation parameters
    /// are scaled by the model size so all columns have length units.
    fn jacobian(&self, eqs: &[Eq], drags: &[Drag], poses: &[Pose]) -> Rows {
        let mut rows: Rows = Vec::new();
        let h = 1e-6 * self.scale;
        let (mut plus, mut minus) = (Vec::new(), Vec::new());
        let mut block = |count: usize,
                         bodies: Vec<usize>,
                         eval: &dyn Fn(&Poses, &mut Vec<f64>),
                         rows: &mut Rows| {
            let base = rows.len();
            rows.extend((0..count).map(|_| Vec::new()));
            for b in bodies {
                let Some(col) = self.column(b) else { continue };
                for k in 0..6 {
                    let step = |s: f64| {
                        let mut dt = DVec3::ZERO;
                        let mut dw = DVec3::ZERO;
                        if k < 3 {
                            dt[k] = s;
                        } else {
                            dw[k - 3] = s / self.scale;
                        }
                        poses[b].nudged(dt, dw)
                    };
                    plus.clear();
                    eval(&Poses::with(poses, b, step(h)), &mut plus);
                    minus.clear();
                    eval(&Poses::with(poses, b, step(-h)), &mut minus);
                    for r in 0..count {
                        let v = (plus[r] - minus[r]) / (2.0 * h);
                        if v != 0.0 {
                            rows[base + r].push((col + k, v));
                        }
                    }
                }
            }
        };
        let scale = self.scale;
        for eq in eqs {
            block(
                eq.rows,
                eq.term.bodies(),
                &|p, out| eval::residuals(p, eq.term, scale, out),
                &mut rows,
            );
        }
        for d in drags {
            block(
                9,
                vec![d.body()],
                &|p, out| drag_rows(p, d, scale, out),
                &mut rows,
            );
        }
        rows
    }

    fn apply(&self, poses: &[Pose], step: &[f64]) -> Vec<Pose> {
        let mut out = poses.to_vec();
        for (i, b) in self.bodies.iter().enumerate() {
            let o = 6 * i;
            let dt = DVec3::new(step[o], step[o + 1], step[o + 2]);
            let dw = DVec3::new(step[o + 3], step[o + 4], step[o + 5]) / self.scale;
            out[*b] = poses[*b].nudged(-dt, -dw);
        }
        out
    }

    /// Damped least squares from `poses`. Returns whether the equations
    /// (not the drags) reached zero.
    fn least_squares(
        &self,
        eqs: &[Eq],
        drags: &[Drag],
        poses: &mut Vec<Pose>,
        iters: usize,
    ) -> bool {
        let amax = |v: &[f64]| v.iter().fold(0.0f64, |a, b| a.max(b.abs()));
        let hard = |poses: &[Pose]| amax(&self.residual(eqs, &[], poses)) < self.tol;
        if self.n() == 0 || (eqs.is_empty() && drags.is_empty()) {
            return hard(poses);
        }
        let mut profile = Profile::new(&self.adjacency());
        let mut r = self.residual(eqs, drags, poses);
        let mut cost: f64 = r.iter().map(|v| v * v).sum();
        let mut lambda: f64 = 1e-6;
        for _ in 0..iters {
            if amax(&r) < self.tol {
                break;
            }
            let j = self.jacobian(eqs, drags, poses);
            let mut improved = false;
            for _ in 0..12 {
                let g = profile.normal(&j, &r, lambda.max(1e-12));
                if profile.factor(false).is_none() {
                    lambda *= 10.0;
                    continue;
                }
                let step = profile.solve(&g);
                let trial = self.apply(poses, &step);
                if trial.iter().any(|p| !p.is_finite()) {
                    lambda *= 4.0;
                    continue;
                }
                let rt = self.residual(eqs, drags, &trial);
                let ct: f64 = rt.iter().map(|v| v * v).sum();
                if ct < cost {
                    let small = amax(&step) < 1e-13 * self.scale;
                    *poses = trial;
                    r = rt;
                    cost = ct;
                    lambda = (lambda / 3.0).max(1e-15);
                    improved = !small;
                    break;
                }
                lambda *= 4.0;
            }
            if !improved {
                break;
            }
        }
        hard(poses)
    }
}

/// Rows pulling a dragged body: 9 rows (three reference points), weak so
/// mates win.
fn drag_rows(p: &Poses, d: &Drag, scale: f64, out: &mut Vec<f64>) {
    const WEIGHT: f64 = 0.1;
    match d {
        Drag::Point {
            body,
            local,
            target,
        } => {
            let q = (p.pose(Some(*body)).point(*local) - *target) * WEIGHT;
            out.extend([q.x, q.y, q.z]);
            out.extend([0.0; 6]);
        }
        Drag::Pose { body, pose: target } => {
            let have = p.pose(Some(*body));
            for axis in [DVec3::ZERO, DVec3::X * scale, DVec3::Y * scale] {
                let q = (have.point(axis) - target.point(axis)) * WEIGHT;
                out.extend([q.x, q.y, q.z]);
            }
        }
    }
}

/// Characteristic model size: the spread of referenced points and poses.
fn model_scale(sys: &System) -> f64 {
    let mut size: f64 = 10.0;
    for p in &sys.poses {
        size = size.max(p.translation.length());
    }
    for m in &sys.mates {
        for t in &m.terms {
            visit_points(t, &mut |p| size = size.max(p.length()));
        }
    }
    size
}

fn visit_points(t: &Term, f: &mut impl FnMut(DVec3)) {
    use crate::Measure as M;
    match t {
        Term::Points { a, b } => {
            f(a.at);
            f(b.at);
        }
        Term::PointOnLine { point, line } => {
            f(point.at);
            f(line.origin);
        }
        Term::Value { measure, .. } => match measure {
            M::PlaneDistance { point, plane } => {
                f(point.at);
                f(plane.origin);
            }
            M::PointDistance { a, b } => {
                f(a.at);
                f(b.at);
            }
            M::LineDistance { point, line } => {
                f(point.at);
                f(line.origin);
            }
            M::Along { point, origin, .. } => {
                f(point.at);
                f(origin.at);
            }
            M::CurveDistance { point, curve, .. } | M::CurveParameter { point, curve } => {
                f(point.at);
                curve.points.iter().for_each(|p| f(*p));
            }
            M::MirrorPlaneDistance {
                point,
                mirror,
                plane,
            } => {
                f(point.at);
                f(mirror.origin);
                f(plane.origin);
            }
            M::Sum(parts) => {
                for (m, _) in parts {
                    visit_points(
                        &Term::Value {
                            measure: m.clone(),
                            target: crate::Target::Equal(0.0),
                        },
                        f,
                    );
                }
            }
            _ => {}
        },
        Term::MirrorPoints { a, b, plane } => {
            f(a.at);
            f(b.at);
            f(plane.origin);
        }
        Term::CurveTangent { point, curve, .. } | Term::CurveUp { point, curve, .. } => {
            f(point.at);
            curve.points.iter().for_each(|p| f(*p));
        }
        Term::PointAtParameter { point, curve, .. } => {
            f(point.at);
            curve.points.iter().for_each(|p| f(*p));
        }
        _ => {}
    }
}

/// Union-find over floating bodies connected by mates.
fn groups(sys: &System) -> Vec<Vec<usize>> {
    let n = sys.poses.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while p[r] != r {
            r = p[r];
        }
        let mut i = i;
        while p[i] != r {
            let next = p[i];
            p[i] = r;
            i = next;
        }
        r
    }
    let floating = |b: usize| !sys.fixed.get(b).copied().unwrap_or(false);
    for m in &sys.mates {
        let bodies: Vec<usize> = m
            .terms
            .iter()
            .flat_map(|t| t.bodies())
            .filter(|b| *b < n && floating(*b))
            .collect();
        for w in bodies.windows(2) {
            let (a, b) = (find(&mut parent, w[0]), find(&mut parent, w[1]));
            parent[a] = b;
        }
    }
    let mut out: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for b in 0..n {
        if floating(b) {
            let r = find(&mut parent, b);
            out.entry(r).or_default().push(b);
        }
    }
    out.into_values().collect()
}

/// Solve every mate. Returns new poses (failing groups keep theirs) and a
/// report. Set `analyse` false to skip DOF analysis (drag steps).
pub fn solve(sys: &System, drags: &[Drag], analyse: bool) -> (Vec<Pose>, Report) {
    let n = sys.poses.len();
    let scale = model_scale(sys);
    let tol = 1e-8 * scale;
    let mut poses = sys.poses.clone();
    let mut report = Report {
        solved: true,
        dof: vec![0; n],
        ..Default::default()
    };
    let valid_body = |b: &usize| *b < n;
    let floating = |b: usize| !sys.fixed.get(b).copied().unwrap_or(false);

    // Mates touching only fixed bodies cannot move anything: just check them.
    for m in &sys.mates {
        let bodies: Vec<usize> = m.terms.iter().flat_map(|t| t.bodies()).collect();
        if bodies.iter().any(|b| !valid_body(b)) {
            report.solved = false;
            report.failed.push(m.id);
            report.conflicting.push(m.id);
            continue;
        }
        if bodies.iter().all(|b| !floating(*b)) {
            let p = Poses::new(&poses);
            let mut r = Vec::new();
            for t in &m.terms {
                eval::residuals(&p, t, scale, &mut r);
            }
            if r.iter().any(|v| v.abs() > tol) {
                report.solved = false;
                report.failed.push(m.id);
                report.conflicting.push(m.id);
            }
        }
    }

    for bodies in groups(sys) {
        let in_group = |b: &usize| bodies.contains(b);
        let mate_ids: Vec<usize> = sys
            .mates
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                let bs: Vec<usize> = m.terms.iter().flat_map(|t| t.bodies()).collect();
                bs.iter().all(valid_body) && bs.iter().any(in_group)
            })
            .map(|(i, _)| i)
            .collect();
        let eqs: Vec<Eq> = mate_ids
            .iter()
            .flat_map(|&mi| {
                sys.mates[mi].terms.iter().map(move |t| Eq {
                    mate: mi,
                    term: t,
                    rows: eval::rows(t),
                })
            })
            .collect();
        let group = Group {
            bodies: bodies.clone(),
            eqs,
            scale,
            tol,
        };
        let group_drags: Vec<Drag> = drags
            .iter()
            .filter(|d| in_group(&d.body()))
            .copied()
            .collect();
        let start = poses.clone();
        let mut trial = start.clone();
        if !group_drags.is_empty() {
            group.least_squares(&group.eqs, &group_drags, &mut trial, DRAG_ITERS);
        }
        let ok = group.least_squares(&group.eqs, &[], &mut trial, ITERS);
        if ok {
            for b in &bodies {
                poses[*b] = trial[*b];
            }
        } else {
            report.solved = false;
            // Mates that do not hold at the best attempt.
            let p = Poses::new(&trial);
            for &mi in &mate_ids {
                let mut r = Vec::new();
                for t in &sys.mates[mi].terms {
                    eval::residuals(&p, t, scale, &mut r);
                }
                if r.iter().any(|v| v.abs() > tol) {
                    report.failed.push(sys.mates[mi].id);
                }
            }
            let conflict = conflict_set(&group, &mate_ids, &start);
            report
                .conflicting
                .extend(conflict.iter().map(|mi| sys.mates[*mi].id));
        }
        // Coupler values after the solve, for continuity next time.
        let p = Poses::new(&poses);
        for &mi in &mate_ids {
            for t in &sys.mates[mi].terms {
                if let Term::Coupler { motions, start, .. } = t {
                    let values = motions
                        .iter()
                        .enumerate()
                        .map(|(i, (m, _))| {
                            eval::motion(&p, m, start.get(i).copied().unwrap_or(0.0))
                        })
                        .collect();
                    report.couplers.insert(sys.mates[mi].id, values);
                }
            }
        }
        if analyse {
            let (dof, free, redundant) = analyse_group(&group, &poses);
            for (i, b) in bodies.iter().enumerate() {
                report.dof[*b] = dof[i];
            }
            report.free += free;
            if ok {
                report
                    .redundant
                    .extend(redundant.iter().map(|mi| sys.mates[*mi].id));
            }
        }
    }
    report.failed.sort_unstable();
    report.failed.dedup();
    report.conflicting.sort_unstable();
    report.conflicting.dedup();
    report.redundant.sort_unstable();
    report.redundant.dedup();
    (poses, report)
}

/// Deletion filter: drop each mate whose removal still leaves the rest
/// unsolvable. What remains is a minimal conflicting set.
fn conflict_set(group: &Group, mates: &[usize], start: &[Pose]) -> Vec<usize> {
    if mates.len() > CONFLICT_SEARCH_LIMIT {
        return mates.to_vec();
    }
    let feasible = |set: &[usize]| {
        let eqs: Vec<Eq> = group
            .eqs
            .iter()
            .filter(|e| set.contains(&e.mate))
            .copied()
            .collect();
        let mut poses = start.to_vec();
        group.least_squares(&eqs, &[], &mut poses, ITERS)
    };
    let mut set = mates.to_vec();
    let mut i = 0;
    while i < set.len() {
        let mut without = set.clone();
        without.remove(i);
        if !feasible(&without) {
            set = without;
        } else {
            i += 1;
        }
    }
    set
}

/// Largest group (in unknowns) checked for redundant mates.
const REDUNDANCY_LIMIT: usize = 240;

/// DOF per group body, total free motions and redundant mates at the given
/// poses. Range terms inside their range constrain nothing.
fn analyse_group(group: &Group, poses: &[Pose]) -> (Vec<u32>, u32, Vec<usize>) {
    let p = Poses::new(poses);
    let eqs: Vec<Eq> = group
        .eqs
        .iter()
        .filter(|e| !eval::inactive(&p, e.term))
        .copied()
        .collect();
    let n = group.n();
    let j = group.jacobian(&eqs, &[], poses);

    // Mates whose rows add nothing new to the ones before them.
    let mut redundant = Vec::new();
    if n <= REDUNDANCY_LIMIT {
        let mut basis: Vec<DVector<f64>> = Vec::new();
        let mut row = 0;
        let mut i = 0;
        while i < eqs.len() {
            let mate = eqs[i].mate;
            let mut independent = false;
            while i < eqs.len() && eqs[i].mate == mate {
                for r in 0..eqs[i].rows {
                    let mut v = DVector::zeros(n);
                    for (c, val) in &j[row + r] {
                        v[*c] = *val;
                    }
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
                    if v.norm() > 1e-6 {
                        let len = v.norm();
                        basis.push(v / len);
                        independent = true;
                    }
                }
                row += eqs[i].rows;
                i += 1;
            }
            if !independent {
                redundant.push(mate);
            }
        }
    }

    // Null space of JᵀJ: motions every mate allows.
    let mut profile = Profile::new(&group.adjacency());
    let zeros = vec![0.0; j.len()];
    profile.normal(&j, &zeros, 0.0);
    let zero = profile.factor(true).unwrap_or_default();
    let null = profile.null_space(&zero);
    let dof = (0..group.bodies.len())
        .map(|b| {
            if null.is_empty() {
                return 0;
            }
            let block = DMatrix::from_fn(6, null.len(), |r, c| null[c][6 * b + r]);
            let sv = block.singular_values();
            let top = sv.amax().max(1.0);
            sv.iter().filter(|s| **s > 1e-6 * top).count() as u32
        })
        .collect();
    (dof, null.len() as u32, redundant)
}

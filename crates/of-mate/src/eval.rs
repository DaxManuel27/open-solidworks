//! Residuals of terms at given poses.

use glam::DVec3;

use crate::{Body, Curve, Dir, Line, Measure, Motion, Plane, Pose, Pt, Target, Term, wrap};

/// Poses with an optional temporary override, for finite differences.
pub(crate) struct Poses<'a> {
    base: &'a [Pose],
    over: Option<(usize, Pose)>,
}

impl<'a> Poses<'a> {
    pub(crate) fn new(base: &'a [Pose]) -> Poses<'a> {
        Poses { base, over: None }
    }

    pub(crate) fn with(base: &'a [Pose], body: usize, pose: Pose) -> Poses<'a> {
        Poses {
            base,
            over: Some((body, pose)),
        }
    }

    pub(crate) fn pose(&self, body: Body) -> Pose {
        match body {
            None => Pose::IDENTITY,
            Some(b) => match self.over {
                Some((o, p)) if o == b => p,
                _ => self.base.get(b).copied().unwrap_or(Pose::IDENTITY),
            },
        }
    }

    fn pt(&self, p: &Pt) -> DVec3 {
        self.pose(p.body).point(p.at)
    }

    fn dir(&self, d: &Dir) -> DVec3 {
        self.pose(d.body).vector(d.dir).normalize_or_zero()
    }

    fn line(&self, l: &Line) -> (DVec3, DVec3) {
        let pose = self.pose(l.body);
        (pose.point(l.origin), pose.vector(l.dir).normalize_or_zero())
    }

    fn plane(&self, p: &Plane) -> (DVec3, DVec3) {
        let pose = self.pose(p.body);
        (
            pose.point(p.origin),
            pose.vector(p.normal).normalize_or_zero(),
        )
    }

    fn curve(&self, c: &Curve) -> Vec<DVec3> {
        let pose = self.pose(c.body);
        c.points.iter().map(|p| pose.point(*p)).collect()
    }
}

/// Segments of a polyline, closing it if needed.
fn segments(points: &[DVec3], closed: bool) -> impl Iterator<Item = (DVec3, DVec3)> + '_ {
    let n = points.len();
    let count = if closed && n > 2 {
        n
    } else {
        n.saturating_sub(1)
    };
    (0..count).map(move |i| (points[i], points[(i + 1) % n]))
}

/// Nearest point on a polyline: (point, arc length to it).
pub(crate) fn nearest(points: &[DVec3], closed: bool, p: DVec3) -> Option<(DVec3, f64)> {
    let mut best: Option<(f64, DVec3, f64)> = None;
    let mut s = 0.0;
    for (a, b) in segments(points, closed) {
        let ab = b - a;
        let len2 = ab.length_squared();
        let t = if len2 > 0.0 {
            ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let q = a + ab * t;
        let d = q.distance_squared(p);
        if best.is_none_or(|(bd, ..)| d < bd) {
            best = Some((d, q, s + len2.sqrt() * t));
        }
        s += len2.sqrt();
    }
    if best.is_none() {
        return points.first().map(|q| (*q, 0.0));
    }
    best.map(|(_, q, s)| (q, s))
}

/// The point at arc length `s` along a polyline (clamped, or wrapped if closed).
pub(crate) fn at_length(points: &[DVec3], closed: bool, s: f64) -> Option<DVec3> {
    let total: f64 = segments(points, closed).map(|(a, b)| a.distance(b)).sum();
    if points.is_empty() {
        return None;
    }
    let mut s = if closed && total > 0.0 {
        s.rem_euclid(total)
    } else {
        s.clamp(0.0, total)
    };
    for (a, b) in segments(points, closed) {
        let len = a.distance(b);
        if s <= len && len > 0.0 {
            return Some(a + (b - a) * (s / len));
        }
        s -= len;
    }
    points.last().copied()
}

fn project(v: DVec3, n: DVec3) -> DVec3 {
    v - n * v.dot(n)
}

fn signed_angle(a: DVec3, b: DVec3, axis: DVec3) -> f64 {
    let (a, b) = (project(a, axis), project(b, axis));
    a.cross(b).dot(axis).atan2(a.dot(b))
}

pub(crate) fn measure(p: &Poses, m: &Measure) -> f64 {
    match m {
        Measure::PlaneDistance { point, plane } => {
            let (o, n) = p.plane(plane);
            (p.pt(point) - o).dot(n)
        }
        Measure::PointDistance { a, b } => p.pt(a).distance(p.pt(b)),
        Measure::LineDistance { point, line } => {
            let (o, d) = p.line(line);
            project(p.pt(point) - o, d).length()
        }
        Measure::Angle { a, b } => {
            let (a, b) = (p.dir(a), p.dir(b));
            a.cross(b).length().atan2(a.dot(b))
        }
        Measure::SignedAngle { a, b, axis } => signed_angle(p.dir(a), p.dir(b), p.dir(axis)),
        Measure::Dot { a, b } => p.dir(a).dot(p.dir(b)),
        Measure::Along { point, origin, dir } => (p.pt(point) - p.pt(origin)).dot(p.dir(dir)),
        Measure::CurveDistance {
            point,
            curve,
            normal,
        } => {
            let mut pts = p.curve(curve);
            let mut q = p.pt(point);
            if let Some(n) = normal {
                let n = p.dir(n);
                let base = pts.first().copied().unwrap_or_default();
                for c in &mut pts {
                    *c -= n * (*c - base).dot(n);
                }
                q -= n * (q - base).dot(n);
            }
            nearest(&pts, curve.closed, q).map_or(0.0, |(c, _)| c.distance(q))
        }
        Measure::CurveParameter { point, curve } => {
            let pts = p.curve(curve);
            nearest(&pts, curve.closed, p.pt(point)).map_or(0.0, |(_, s)| s)
        }
        Measure::MirrorPlaneDistance {
            point,
            mirror,
            plane,
        } => {
            let (mo, mn) = p.plane(mirror);
            let q = reflect_point(p.pt(point), mo, mn);
            let (o, n) = p.plane(plane);
            (q - o).dot(n)
        }
        Measure::Sum(parts) => parts.iter().map(|(m, w)| w * measure(p, m)).sum(),
    }
}

fn reflect_point(q: DVec3, o: DVec3, n: DVec3) -> DVec3 {
    q - n * (2.0 * (q - o).dot(n))
}

fn reflect_vector(v: DVec3, n: DVec3) -> DVec3 {
    v - n * (2.0 * v.dot(n))
}

/// Unit tangent of a polyline at the segment nearest `q`.
fn tangent(points: &[DVec3], closed: bool, q: DVec3) -> DVec3 {
    let mut best: Option<(f64, DVec3)> = None;
    for (a, b) in segments(points, closed) {
        let ab = b - a;
        let len2 = ab.length_squared();
        if len2 == 0.0 {
            continue;
        }
        let t = ((q - a).dot(ab) / len2).clamp(0.0, 1.0);
        let d = (a + ab * t).distance_squared(q);
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, ab / len2.sqrt()));
        }
    }
    best.map_or(DVec3::ZERO, |(_, t)| t)
}

/// Raw motion value; rotations are wrapped to −π..π.
pub(crate) fn motion_raw(p: &Poses, m: &Motion) -> f64 {
    match m {
        Motion::Rotation {
            vector,
            reference,
            axis,
        } => signed_angle(p.dir(reference), p.dir(vector), p.dir(axis)),
        Motion::Translation { point, origin, dir } => (p.pt(point) - p.pt(origin)).dot(p.dir(dir)),
    }
}

/// Motion value continued from `start` (rotations unwrapped).
pub(crate) fn motion(p: &Poses, m: &Motion, start: f64) -> f64 {
    let raw = motion_raw(p, m);
    match m {
        Motion::Rotation { .. } => start + wrap(raw - start),
        Motion::Translation { .. } => raw,
    }
}

/// Whether a term's scalar is an angle, so its rows scale with model size.
fn angular(m: &Measure) -> bool {
    matches!(
        m,
        Measure::Angle { .. } | Measure::SignedAngle { .. } | Measure::Dot { .. }
    )
}

/// Residual rows of a term. `scale` (model size) makes direction rows
/// comparable to length rows.
pub(crate) fn residuals(p: &Poses, term: &Term, scale: f64, out: &mut Vec<f64>) {
    match term {
        Term::Points { a, b } => {
            let d = p.pt(a) - p.pt(b);
            out.extend([d.x, d.y, d.z]);
        }
        Term::PointOnLine { point, line } => {
            let (o, d) = p.line(line);
            let r = project(p.pt(point) - o, d);
            out.extend([r.x, r.y, r.z]);
        }
        Term::Parallel { a, b, sign } => {
            let (a, b) = (p.dir(a), p.dir(b));
            let r = match sign {
                Some(s) => a - b * *s,
                None => a.cross(b),
            } * scale;
            out.extend([r.x, r.y, r.z]);
        }
        Term::Value { measure: m, target } => {
            let v = measure(p, m);
            let r = match *target {
                Target::Equal(t) => v - t,
                Target::Range { min, max } => {
                    if v < min {
                        v - min
                    } else if v > max {
                        v - max
                    } else {
                        0.0
                    }
                }
            };
            out.push(if angular(m) { r * scale } else { r });
        }
        Term::Lock { a, b, relative } => {
            let want = p.pose(*a).then(relative);
            let have = p.pose(*b);
            let d = have.translation - want.translation;
            let q = have.rotation * want.rotation.inverse();
            // Shortest rotation between the two orientations.
            let q = if q.w < 0.0 { -q } else { q };
            let w = q.to_scaled_axis() * scale;
            out.extend([d.x, d.y, d.z, w.x, w.y, w.z]);
        }
        Term::PointAtParameter { point, curve, at } => {
            let pts = p.curve(curve);
            let target = at_length(&pts, curve.closed, *at).unwrap_or_default();
            let d = p.pt(point) - target;
            out.extend([d.x, d.y, d.z]);
        }
        Term::MirrorPoints { a, b, plane } => {
            let (o, n) = p.plane(plane);
            let d = p.pt(b) - reflect_point(p.pt(a), o, n);
            out.extend([d.x, d.y, d.z]);
        }
        Term::MirrorParallel { a, b, plane, sign } => {
            let (_, n) = p.plane(plane);
            let d = (p.dir(b) - reflect_vector(p.dir(a), n) * *sign) * scale;
            out.extend([d.x, d.y, d.z]);
        }
        Term::CurveTangent { dir, point, curve } => {
            let t = tangent(&p.curve(curve), curve.closed, p.pt(point));
            let d = p.dir(dir).cross(t) * scale;
            out.extend([d.x, d.y, d.z]);
        }
        Term::CurveUp {
            axis,
            up,
            point,
            curve,
        } => {
            let t = tangent(&p.curve(curve), curve.closed, p.pt(point));
            let want = project(p.dir(up), t).normalize_or_zero();
            let d = (p.dir(axis) - want) * scale;
            out.extend([d.x, d.y, d.z]);
        }
        Term::Coupler {
            motions,
            start,
            phase,
        } => {
            let mut sum = 0.0;
            for (i, (m, ratio)) in motions.iter().enumerate() {
                sum += ratio * motion(p, m, start.get(i).copied().unwrap_or(0.0));
            }
            let r = sum - phase;
            let rotational = matches!(motions.first(), Some((Motion::Rotation { .. }, _)));
            out.push(if rotational { r * scale } else { r });
        }
    }
}

/// Number of rows a term produces.
pub(crate) fn rows(term: &Term) -> usize {
    match term {
        Term::Points { .. }
        | Term::PointOnLine { .. }
        | Term::Parallel { .. }
        | Term::PointAtParameter { .. }
        | Term::MirrorPoints { .. }
        | Term::MirrorParallel { .. }
        | Term::CurveTangent { .. }
        | Term::CurveUp { .. } => 3,
        Term::Value { .. } | Term::Coupler { .. } => 1,
        Term::Lock { .. } => 6,
    }
}

/// Whether a range term is currently inside its range (contributes nothing
/// to the rank analysis).
pub(crate) fn inactive(p: &Poses, term: &Term) -> bool {
    match term {
        Term::Value {
            measure: m,
            target: Target::Range { min, max },
        } => {
            let v = measure(p, m);
            v > *min + 1e-9 && v < *max - 1e-9
        }
        _ => false,
    }
}

//! 2D curve geometry: sampling, parameters and intersections.

use std::f64::consts::TAU;

use glam::DVec2;
use serde::{Deserialize, Serialize};

/// A 2D curve. Arcs run counter-clockwise from `start` to `end`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Curve2 {
    Line {
        a: DVec2,
        b: DVec2,
    },
    Arc {
        center: DVec2,
        start: DVec2,
        end: DVec2,
    },
    Circle {
        center: DVec2,
        radius: f64,
    },
    Spline {
        points: Vec<DVec2>,
        closed: bool,
    },
}

const EPS: f64 = 1e-9;

/// Start angle and counter-clockwise sweep (in `(0, 2π]`) of an arc.
pub fn arc_angles(center: DVec2, start: DVec2, end: DVec2) -> (f64, f64) {
    let a0 = (start - center).to_angle();
    let a1 = (end - center).to_angle();
    let mut sweep = (a1 - a0).rem_euclid(TAU);
    if sweep < 1e-12 {
        sweep = TAU;
    }
    (a0, sweep)
}

fn angle_in_sweep(a0: f64, sweep: f64, a: f64) -> Option<f64> {
    let d = (a - a0).rem_euclid(TAU);
    if d <= sweep + 1e-9 {
        Some(d.min(sweep))
    } else if TAU - d < 1e-9 {
        Some(0.0)
    } else {
        None
    }
}

impl Curve2 {
    pub fn start(&self) -> DVec2 {
        self.point_at(0.0)
    }

    pub fn end(&self) -> DVec2 {
        self.point_at(1.0)
    }

    pub fn is_closed(&self) -> bool {
        matches!(
            self,
            Curve2::Circle { .. } | Curve2::Spline { closed: true, .. }
        )
    }

    /// Point at normalised parameter `t` in `[0, 1]`.
    pub fn point_at(&self, t: f64) -> DVec2 {
        match self {
            Curve2::Line { a, b } => a.lerp(*b, t),
            Curve2::Arc { center, start, end } => {
                let (a0, sweep) = arc_angles(*center, *start, *end);
                let r = (*start - *center).length();
                *center + DVec2::from_angle(a0 + sweep * t) * r
            }
            Curve2::Circle { center, radius } => *center + DVec2::from_angle(TAU * t) * *radius,
            Curve2::Spline { .. } => {
                let pts = self.sample(0.01);
                polyline_point_at(&pts, t)
            }
        }
    }

    /// Normalised parameter of the point on the curve closest to `p`.
    pub fn param_of(&self, p: DVec2) -> f64 {
        match self {
            Curve2::Line { a, b } => {
                let d = *b - *a;
                ((p - *a).dot(d) / d.length_squared().max(EPS)).clamp(0.0, 1.0)
            }
            Curve2::Arc { center, start, end } => {
                let (a0, sweep) = arc_angles(*center, *start, *end);
                let ang = (p - *center).to_angle();
                match angle_in_sweep(a0, sweep, ang) {
                    Some(d) => d / sweep,
                    None => {
                        // Outside the arc: snap to the nearer end.
                        if (p - *start).length() < (p - *end).length() {
                            0.0
                        } else {
                            1.0
                        }
                    }
                }
            }
            Curve2::Circle { center, .. } => (p - *center).to_angle().rem_euclid(TAU) / TAU,
            Curve2::Spline { .. } => {
                let pts = self.sample(0.01);
                polyline_param_of(&pts, p)
            }
        }
    }

    pub fn closest_point(&self, p: DVec2) -> DVec2 {
        self.point_at(self.param_of(p))
    }

    /// Polyline approximation within `tol` (chord deviation), endpoints
    /// included. Closed curves repeat the first point at the end.
    pub fn sample(&self, tol: f64) -> Vec<DVec2> {
        let tol = tol.max(1e-6);
        match self {
            Curve2::Line { a, b } => vec![*a, *b],
            Curve2::Arc { center, start, end } => {
                let (_, sweep) = arc_angles(*center, *start, *end);
                let r = (*start - *center).length();
                let n = segments_for(r, sweep, tol);
                (0..=n)
                    .map(|i| self.point_at(i as f64 / n as f64))
                    .collect()
            }
            Curve2::Circle { radius, .. } => {
                let n = segments_for(*radius, TAU, tol);
                (0..=n)
                    .map(|i| self.point_at(i as f64 / n as f64))
                    .collect()
            }
            Curve2::Spline { points, closed } => catmull_rom(points, *closed, 16),
        }
    }

    /// Bounding box (min, max).
    pub fn bounds(&self) -> (DVec2, DVec2) {
        let pts = self.sample(0.05);
        let mut lo = DVec2::splat(f64::MAX);
        let mut hi = DVec2::splat(f64::MIN);
        for p in pts {
            lo = lo.min(p);
            hi = hi.max(p);
        }
        (lo, hi)
    }

    pub fn length(&self) -> f64 {
        match self {
            Curve2::Line { a, b } => (*b - *a).length(),
            Curve2::Arc { center, start, end } => {
                let (_, sweep) = arc_angles(*center, *start, *end);
                sweep * (*start - *center).length()
            }
            Curve2::Circle { radius, .. } => TAU * radius,
            Curve2::Spline { .. } => self
                .sample(0.01)
                .windows(2)
                .map(|w| (w[1] - w[0]).length())
                .sum(),
        }
    }
}

fn segments_for(r: f64, sweep: f64, tol: f64) -> usize {
    if r < EPS {
        return 1;
    }
    let step = 2.0 * (1.0 - (tol / r).min(1.0)).acos();
    let step = if step.is_finite() && step > 1e-3 {
        step
    } else {
        0.1
    };
    ((sweep / step).ceil() as usize).clamp(4, 512)
}

fn catmull_rom(points: &[DVec2], closed: bool, per_segment: usize) -> Vec<DVec2> {
    let n = points.len();
    if n < 2 {
        return points.to_vec();
    }
    let get = |i: isize| -> DVec2 {
        if closed {
            points[i.rem_euclid(n as isize) as usize]
        } else if i < 0 {
            points[0] * 2.0 - points[1]
        } else if i as usize >= n {
            points[n - 1] * 2.0 - points[n - 2]
        } else {
            points[i as usize]
        }
    };
    let segs = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(segs * per_segment + 1);
    for s in 0..segs as isize {
        let (p0, p1, p2, p3) = (get(s - 1), get(s), get(s + 1), get(s + 2));
        for k in 0..per_segment {
            let t = k as f64 / per_segment as f64;
            let t2 = t * t;
            let t3 = t2 * t;
            out.push(
                0.5 * ((2.0 * p1)
                    + (-p0 + p2) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                    + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3),
            );
        }
    }
    out.push(if closed { points[0] } else { points[n - 1] });
    out
}

fn polyline_point_at(pts: &[DVec2], t: f64) -> DVec2 {
    let total: f64 = pts.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    let mut target = t.clamp(0.0, 1.0) * total;
    for w in pts.windows(2) {
        let l = (w[1] - w[0]).length();
        if target <= l || l == 0.0 && target <= 0.0 {
            return w[0].lerp(w[1], if l > 0.0 { target / l } else { 0.0 });
        }
        target -= l;
    }
    *pts.last().unwrap()
}

fn polyline_param_of(pts: &[DVec2], p: DVec2) -> f64 {
    let total: f64 = pts.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    let (mut best, mut best_d, mut acc) = (0.0, f64::MAX, 0.0);
    for w in pts.windows(2) {
        let seg = Curve2::Line { a: w[0], b: w[1] };
        let t = seg.param_of(p);
        let d = (seg.point_at(t) - p).length();
        let l = (w[1] - w[0]).length();
        if d < best_d {
            best_d = d;
            best = acc + t * l;
        }
        acc += l;
    }
    if total > 0.0 { best / total } else { 0.0 }
}

/// Intersections of two curves, as points.
pub fn intersect(a: &Curve2, b: &Curve2) -> Vec<DVec2> {
    use Curve2::*;
    let mut pts = match (a, b) {
        (Line { a: p, b: q }, Line { a: r, b: s }) => {
            line_line(*p, *q, *r, *s, false).into_iter().collect()
        }
        (Line { a: p, b: q }, _) if round(b).is_some() => line_round(*p, *q, b),
        (_, Line { a: p, b: q }) if round(a).is_some() => line_round(*p, *q, a),
        _ if round(a).is_some() && round(b).is_some() => round_round(a, b),
        _ => {
            // Splines: intersect the polylines.
            let (pa, pb) = (a.sample(0.01), b.sample(0.01));
            let mut out = Vec::new();
            for w in pa.windows(2) {
                for v in pb.windows(2) {
                    if let Some(p) = line_line(w[0], w[1], v[0], v[1], false) {
                        out.push(p);
                    }
                }
            }
            out
        }
    };
    pts.dedup_by(|x, y| (*x - *y).length() < 1e-7);
    pts
}

/// Center, radius and, for arcs, (start angle, sweep).
type Round = (DVec2, f64, Option<(f64, f64)>);

fn round(c: &Curve2) -> Option<Round> {
    match c {
        Curve2::Circle { center, radius } => Some((*center, *radius, None)),
        Curve2::Arc { center, start, end } => Some((
            *center,
            (*start - *center).length(),
            Some(arc_angles(*center, *start, *end)),
        )),
        _ => None,
    }
}

fn on_round(c: &Curve2, p: DVec2) -> bool {
    match round(c) {
        Some((center, _, Some((a0, sweep)))) => {
            angle_in_sweep(a0, sweep, (p - center).to_angle()).is_some()
        }
        Some(_) => true,
        None => false,
    }
}

/// Segment–segment intersection; with `infinite`, the lines through them.
pub fn line_line(p: DVec2, q: DVec2, r: DVec2, s: DVec2, infinite: bool) -> Option<DVec2> {
    let d1 = q - p;
    let d2 = s - r;
    let den = d1.perp_dot(d2);
    if den.abs() < 1e-12 {
        return None;
    }
    let t = (r - p).perp_dot(d2) / den;
    let u = (r - p).perp_dot(d1) / den;
    let tol = 1e-9;
    if infinite || ((-tol..=1.0 + tol).contains(&t) && (-tol..=1.0 + tol).contains(&u)) {
        Some(p + d1 * t)
    } else {
        None
    }
}

/// Intersections of the infinite line through `p`, `q` with a circle.
pub fn line_circle(p: DVec2, q: DVec2, center: DVec2, r: f64) -> Vec<(f64, DVec2)> {
    let d = q - p;
    let f = p - center;
    let a = d.dot(d);
    let b = 2.0 * f.dot(d);
    let c = f.dot(f) - r * r;
    let disc = b * b - 4.0 * a * c;
    if a < 1e-18 || disc < -1e-9 {
        return vec![];
    }
    let sq = disc.max(0.0).sqrt();
    let mut ts = vec![(-b - sq) / (2.0 * a), (-b + sq) / (2.0 * a)];
    if sq < 1e-12 {
        ts.pop();
    }
    ts.into_iter().map(|t| (t, p + d * t)).collect()
}

fn line_round(p: DVec2, q: DVec2, c: &Curve2) -> Vec<DVec2> {
    let (center, r, _) = round(c).unwrap();
    line_circle(p, q, center, r)
        .into_iter()
        .filter(|(t, pt)| (-1e-9..=1.0 + 1e-9).contains(t) && on_round(c, *pt))
        .map(|(_, pt)| pt)
        .collect()
}

fn round_round(a: &Curve2, b: &Curve2) -> Vec<DVec2> {
    let (c1, r1, _) = round(a).unwrap();
    let (c2, r2, _) = round(b).unwrap();
    let d = (c2 - c1).length();
    if d < 1e-12 || d > r1 + r2 + 1e-9 || d < (r1 - r2).abs() - 1e-9 {
        return vec![];
    }
    let x = (d * d + r1 * r1 - r2 * r2) / (2.0 * d);
    let h = (r1 * r1 - x * x).max(0.0).sqrt();
    let u = (c2 - c1) / d;
    let m = c1 + u * x;
    let mut pts = vec![m + u.perp() * h];
    if h > 1e-12 {
        pts.push(m - u.perp() * h);
    }
    pts.into_iter()
        .filter(|p| on_round(a, *p) && on_round(b, *p))
        .collect()
}

/// Signed area of a closed polyline (positive when counter-clockwise).
pub fn signed_area(pts: &[DVec2]) -> f64 {
    let n = pts.len();
    (0..n)
        .map(|i| pts[i].perp_dot(pts[(i + 1) % n]))
        .sum::<f64>()
        * 0.5
}

/// Even-odd point-in-polygon test.
pub fn point_in_polygon(p: DVec2, poly: &[DVec2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_parameters() {
        let arc = Curve2::Arc {
            center: DVec2::ZERO,
            start: DVec2::new(1.0, 0.0),
            end: DVec2::new(0.0, 1.0),
        };
        assert!(
            (arc.point_at(0.5) - DVec2::from_angle(std::f64::consts::FRAC_PI_4)).length() < 1e-12
        );
        assert!((arc.length() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!((arc.param_of(DVec2::new(1.0, 1.0)) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn intersections() {
        let l = Curve2::Line {
            a: DVec2::new(-5.0, 0.0),
            b: DVec2::new(5.0, 0.0),
        };
        let c = Curve2::Circle {
            center: DVec2::ZERO,
            radius: 2.0,
        };
        assert_eq!(intersect(&l, &c).len(), 2);
        let upper = Curve2::Arc {
            center: DVec2::ZERO,
            start: DVec2::new(2.0, 0.0),
            end: DVec2::new(-2.0, 0.0),
        };
        let l2 = Curve2::Line {
            a: DVec2::new(0.0, -5.0),
            b: DVec2::new(0.0, 5.0),
        };
        let hits = intersect(&l2, &upper);
        assert_eq!(hits.len(), 1);
        assert!((hits[0] - DVec2::new(0.0, 2.0)).length() < 1e-12);
        let c2 = Curve2::Circle {
            center: DVec2::new(3.0, 0.0),
            radius: 2.0,
        };
        assert_eq!(intersect(&c, &c2).len(), 2);
    }

    #[test]
    fn polygon_helpers() {
        let sq = [DVec2::ZERO, DVec2::X, DVec2::ONE, DVec2::Y];
        assert!((signed_area(&sq) - 1.0).abs() < 1e-12);
        assert!(point_in_polygon(DVec2::splat(0.5), &sq));
        assert!(!point_in_polygon(DVec2::splat(1.5), &sq));
    }
}

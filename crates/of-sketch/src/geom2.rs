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
    Bezier {
        points: [DVec2; 4],
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
            Curve2::Bezier { points } => bezier_at(*points, t),
            Curve2::Spline { .. } => {
                let spans = self.beziers();
                let u = t.clamp(0.0, 1.0) * spans.len() as f64;
                let i = (u.floor() as usize).min(spans.len() - 1);
                bezier_at(spans[i], u - i as f64)
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
            Curve2::Spline { .. } | Curve2::Bezier { .. } => {
                // Locate the best span before refining; a spline may have several minima.
                let n = self.beziers().len() * 64;
                let i = (0..=n)
                    .min_by(|a, b| {
                        self.point_at(*a as f64 / n as f64)
                            .distance_squared(p)
                            .total_cmp(&self.point_at(*b as f64 / n as f64).distance_squared(p))
                    })
                    .unwrap();
                let mut lo = i.saturating_sub(1) as f64 / n as f64;
                let mut hi = (i + 1).min(n) as f64 / n as f64;
                for _ in 0..48 {
                    let a = lo + (hi - lo) / 3.0;
                    let b = hi - (hi - lo) / 3.0;
                    if self.point_at(a).distance_squared(p) < self.point_at(b).distance_squared(p) {
                        hi = b;
                    } else {
                        lo = a;
                    }
                }
                (lo + hi) * 0.5
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
            Curve2::Spline { .. } | Curve2::Bezier { .. } => {
                let mut out = vec![self.start()];
                for span in self.beziers() {
                    sample_bezier(span, tol, 0, &mut out);
                }
                out
            }
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
            Curve2::Spline { .. } | Curve2::Bezier { .. } => self
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

/// Cubic evaluation and restriction use the same polynomial as the kernel.
pub fn bezier_at(p: [DVec2; 4], t: f64) -> DVec2 {
    let a = p[0].lerp(p[1], t);
    let b = p[1].lerp(p[2], t);
    let c = p[2].lerp(p[3], t);
    a.lerp(b, t).lerp(b.lerp(c, t), t)
}

pub fn bezier_tangent(p: [DVec2; 4], t: f64) -> DVec2 {
    3.0 * ((p[1] - p[0]) * (1.0 - t).powi(2)
        + 2.0 * (p[2] - p[1]) * (1.0 - t) * t
        + (p[3] - p[2]) * t * t)
}

pub fn bezier_part(p: [DVec2; 4], a: f64, b: f64) -> [DVec2; 4] {
    let first = bezier_at(p, a);
    let last = bezier_at(p, b);
    [
        first,
        first + bezier_tangent(p, a) * (b - a) / 3.0,
        last - bezier_tangent(p, b) * (b - a) / 3.0,
        last,
    ]
}

fn sample_bezier(p: [DVec2; 4], tol: f64, depth: usize, out: &mut Vec<DVec2>) {
    let chord = Curve2::Line { a: p[0], b: p[3] };
    let error = p[1]
        .distance(chord.closest_point(p[1]))
        .max(p[2].distance(chord.closest_point(p[2])));
    if error <= tol || depth == 20 {
        out.push(p[3]);
        return;
    }
    sample_bezier(bezier_part(p, 0.0, 0.5), tol, depth + 1, out);
    sample_bezier(bezier_part(p, 0.5, 1.0), tol, depth + 1, out);
}

impl Curve2 {
    pub fn tangent_at(&self, t: f64) -> DVec2 {
        match self {
            Self::Line { a, b } => *b - *a,
            Self::Circle { center, .. } => (self.point_at(t) - *center).perp() * TAU,
            Self::Arc { center, start, end } => {
                (self.point_at(t) - *center).perp() * arc_angles(*center, *start, *end).1
            }
            _ => {
                let spans = self.beziers();
                let u = t.clamp(0.0, 1.0) * spans.len() as f64;
                let i = (u.floor() as usize).min(spans.len() - 1);
                bezier_tangent(spans[i], u - i as f64) * spans.len() as f64
            }
        }
    }

    /// Exact cubic spans of an interpolating Catmull–Rom spline.
    pub fn beziers(&self) -> Vec<[DVec2; 4]> {
        match self {
            Self::Bezier { points } => vec![*points],
            Self::Spline { points, closed } => {
                let n = points.len();
                let get = |i: isize| {
                    if *closed {
                        points[i.rem_euclid(n as isize) as usize]
                    } else if i < 0 {
                        2.0 * points[0] - points[1]
                    } else if i as usize >= n {
                        2.0 * points[n - 1] - points[n - 2]
                    } else {
                        points[i as usize]
                    }
                };
                (0..if *closed { n } else { n - 1 })
                    .map(|i| {
                        let i = i as isize;
                        let a = get(i);
                        let b = get(i + 1);
                        [a, a + (b - get(i - 1)) / 6.0, b - (get(i + 2) - a) / 6.0, b]
                    })
                    .collect()
            }
            _ => vec![],
        }
    }
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
                        let (mut t, mut u) = (a.param_of(p), b.param_of(p));
                        for _ in 0..16 {
                            let delta = a.point_at(t) - b.point_at(u);
                            let (da, db) = (a.tangent_at(t), b.tangent_at(u));
                            let den = da.perp_dot(db);
                            if den.abs() < 1e-14 {
                                break;
                            }
                            t = (t - delta.perp_dot(db) / den).clamp(0.0, 1.0);
                            u = (u - delta.perp_dot(da) / den).clamp(0.0, 1.0);
                        }
                        if a.point_at(t).distance(b.point_at(u)) < 1e-6 {
                            out.push(a.point_at(t));
                        }
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

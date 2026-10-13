//! Clipping display primitives to regions (detail circles, crop profiles)
//! and cutting gaps out of them (break views). Coordinates are any 2D frame;
//! callers use view-plane millimetres.

use crate::document::Profile;
use crate::graphics::{P2, Prim, arc_points};

/// Largest angle between arc samples when an arc has to be cut.
const ARC_STEP: f64 = 0.05;

/// A closed region: a circle or a simple polygon (even-odd).
#[derive(Debug, Clone, PartialEq)]
pub enum Region {
    Circle { center: P2, radius: f64 },
    Polygon(Vec<P2>),
}

impl Region {
    pub fn from_profile(p: &Profile) -> Region {
        match p {
            Profile::Circle { center, radius } => Region::Circle {
                center: *center,
                radius: *radius,
            },
            Profile::Polygon { points } => Region::Polygon(points.clone()),
        }
    }

    pub fn contains(&self, p: P2) -> bool {
        match self {
            Region::Circle { center, radius } => dist(p, *center) <= *radius,
            Region::Polygon(pts) => {
                let mut inside = false;
                let n = pts.len();
                for i in 0..n {
                    let (a, b) = (pts[i], pts[(i + 1) % n]);
                    if (a[1] > p[1]) != (b[1] > p[1]) {
                        let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
                        if p[0] < x {
                            inside = !inside;
                        }
                    }
                }
                inside
            }
        }
    }

    /// Parameters in (0, 1) where segment `a`–`b` crosses the boundary.
    fn crossings(&self, a: P2, b: P2) -> Vec<f64> {
        let d = [b[0] - a[0], b[1] - a[1]];
        let mut out = Vec::new();
        match self {
            Region::Circle { center, radius } => {
                let f = [a[0] - center[0], a[1] - center[1]];
                let qa = d[0] * d[0] + d[1] * d[1];
                if qa < 1e-24 {
                    return out;
                }
                let qb = 2.0 * (f[0] * d[0] + f[1] * d[1]);
                let qc = f[0] * f[0] + f[1] * f[1] - radius * radius;
                let disc = qb * qb - 4.0 * qa * qc;
                if disc > 0.0 {
                    let s = disc.sqrt();
                    out.push((-qb - s) / (2.0 * qa));
                    out.push((-qb + s) / (2.0 * qa));
                }
            }
            Region::Polygon(pts) => {
                let n = pts.len();
                for i in 0..n {
                    let (p, q) = (pts[i], pts[(i + 1) % n]);
                    let e = [q[0] - p[0], q[1] - p[1]];
                    let den = d[0] * e[1] - d[1] * e[0];
                    if den.abs() < 1e-18 {
                        continue;
                    }
                    let w = [p[0] - a[0], p[1] - a[1]];
                    let t = (w[0] * e[1] - w[1] * e[0]) / den;
                    let u = (w[0] * d[1] - w[1] * d[0]) / den;
                    if (0.0..=1.0).contains(&u) {
                        out.push(t);
                    }
                }
            }
        }
        out.retain(|t| *t > 1e-12 && *t < 1.0 - 1e-12);
        out.sort_by(f64::total_cmp);
        out
    }

    /// Pieces of segment `a`–`b` inside (or outside) the region.
    pub fn clip_segment(&self, a: P2, b: P2, keep_inside: bool) -> Vec<[P2; 2]> {
        let mut ts = vec![0.0];
        ts.extend(self.crossings(a, b));
        ts.push(1.0);
        let at = |t: f64| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        let mut out: Vec<[P2; 2]> = Vec::new();
        for w in ts.windows(2) {
            if w[1] - w[0] < 1e-12 {
                continue;
            }
            if self.contains(at((w[0] + w[1]) / 2.0)) == keep_inside {
                // Join with the previous piece when they touch.
                if let Some(last) = out.last_mut()
                    && dist(last[1], at(w[0])) < 1e-12
                {
                    last[1] = at(w[1]);
                } else {
                    out.push([at(w[0]), at(w[1])]);
                }
            }
        }
        out
    }

    /// The boundary as a closed polyline.
    pub fn outline(&self) -> Vec<P2> {
        match self {
            Region::Circle { center, radius } => {
                arc_points(*center, *radius, 0.0, std::f64::consts::TAU, ARC_STEP)
            }
            Region::Polygon(p) => p.clone(),
        }
    }

    pub fn bounds(&self) -> [P2; 2] {
        match self {
            Region::Circle { center, radius } => [
                [center[0] - radius, center[1] - radius],
                [center[0] + radius, center[1] + radius],
            ],
            Region::Polygon(p) => {
                let mut lo = [f64::MAX; 2];
                let mut hi = [f64::MIN; 2];
                for q in p {
                    lo = [lo[0].min(q[0]), lo[1].min(q[1])];
                    hi = [hi[0].max(q[0]), hi[1].max(q[1])];
                }
                [lo, hi]
            }
        }
    }
}

pub fn dist(a: P2, b: P2) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Pieces of a polyline (open) inside or outside a region, joined.
fn clip_polyline(points: &[P2], region: &Region, keep_inside: bool) -> Vec<Vec<P2>> {
    let mut out: Vec<Vec<P2>> = Vec::new();
    for w in points.windows(2) {
        for [a, b] in region.clip_segment(w[0], w[1], keep_inside) {
            match out.last_mut() {
                Some(run) if dist(*run.last().unwrap(), a) < 1e-9 => run.push(b),
                _ => out.push(vec![a, b]),
            }
        }
    }
    out
}

fn polyline_prims(runs: Vec<Vec<P2>>) -> Vec<Prim> {
    runs.into_iter()
        .map(|r| {
            if r.len() == 2 {
                Prim::Line { a: r[0], b: r[1] }
            } else {
                Prim::Polyline {
                    points: r,
                    closed: false,
                }
            }
        })
        .collect()
}

/// The parts of a primitive inside (or outside) a region. Text is kept or
/// dropped whole by its anchor; fills are clipped against the region's
/// outline when it is convex (circles), otherwise kept by their centroid.
pub fn clip(prim: &Prim, region: &Region, keep_inside: bool) -> Vec<Prim> {
    match prim {
        Prim::Line { a, b } => region
            .clip_segment(*a, *b, keep_inside)
            .into_iter()
            .map(|[a, b]| Prim::Line { a, b })
            .collect(),
        Prim::Arc {
            center,
            radius,
            start,
            end,
        } => {
            let pts = arc_points(*center, *radius, *start, *end, ARC_STEP);
            let all_in = pts.iter().all(|p| region.contains(*p) == keep_inside);
            let crosses = pts
                .windows(2)
                .any(|w| !region.crossings(w[0], w[1]).is_empty());
            if all_in && !crosses {
                return vec![prim.clone()];
            }
            if pts.iter().all(|p| region.contains(*p) != keep_inside) && !crosses {
                return Vec::new();
            }
            polyline_prims(clip_polyline(&pts, region, keep_inside))
        }
        Prim::Polyline { points, closed } => {
            let mut pts = points.clone();
            if *closed && !pts.is_empty() {
                pts.push(pts[0]);
            }
            let runs = clip_polyline(&pts, region, keep_inside);
            if *closed && runs.len() == 1 && runs[0].len() == pts.len() {
                return vec![prim.clone()];
            }
            polyline_prims(runs)
        }
        Prim::Fill { loops } => {
            let convex = matches!(region, Region::Circle { .. }) && keep_inside;
            if convex {
                let clip_poly = region.outline();
                let loops: Vec<Vec<P2>> = loops
                    .iter()
                    .map(|l| clip_convex(l, &clip_poly))
                    .filter(|l| l.len() >= 3)
                    .collect();
                if loops.is_empty() {
                    Vec::new()
                } else {
                    vec![Prim::Fill { loops }]
                }
            } else {
                let c = centroid(loops.first().map(Vec::as_slice).unwrap_or(&[]));
                if region.contains(c) == keep_inside {
                    vec![prim.clone()]
                } else {
                    Vec::new()
                }
            }
        }
        Prim::Text { at, .. } => {
            if region.contains(*at) == keep_inside {
                vec![prim.clone()]
            } else {
                Vec::new()
            }
        }
    }
}

fn centroid(p: &[P2]) -> P2 {
    if p.is_empty() {
        return [0.0, 0.0];
    }
    let n = p.len() as f64;
    [
        p.iter().map(|q| q[0]).sum::<f64>() / n,
        p.iter().map(|q| q[1]).sum::<f64>() / n,
    ]
}

/// Sutherland–Hodgman: `subject` clipped to the convex polygon `clip`
/// (either winding).
pub fn clip_convex(subject: &[P2], clip: &[P2]) -> Vec<P2> {
    let ccw = signed_area(clip) > 0.0;
    let mut out = subject.to_vec();
    let n = clip.len();
    for i in 0..n {
        let (a, b) = (clip[i], clip[(i + 1) % n]);
        let inside = |p: P2| {
            let c = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
            if ccw { c >= 0.0 } else { c <= 0.0 }
        };
        let input = std::mem::take(&mut out);
        for j in 0..input.len() {
            let (p, q) = (input[j], input[(j + 1) % input.len()]);
            let (ip, iq) = (inside(p), inside(q));
            if ip {
                out.push(p);
            }
            if ip != iq {
                let d1 = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
                let d2 = (b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0]);
                let t = d1 / (d1 - d2);
                out.push([p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]);
            }
        }
        if out.is_empty() {
            break;
        }
    }
    out
}

pub fn signed_area(p: &[P2]) -> f64 {
    let n = p.len();
    (0..n)
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        / 2.0
}

/// One removed band of a break view, along x (`vertical` break lines) or y.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gap {
    pub vertical: bool,
    pub from: f64,
    pub to: f64,
    /// Space left between the break lines, in the same units.
    pub gap: f64,
}

impl Gap {
    fn axis(&self) -> usize {
        if self.vertical { 0 } else { 1 }
    }

    /// Where a coordinate lands once the band is removed; `None` inside it.
    pub fn shift(&self, v: f64) -> Option<f64> {
        if v <= self.from {
            Some(v)
        } else if v >= self.to {
            Some(v - (self.to - self.from) + self.gap)
        } else {
            None
        }
    }

    pub fn shift_point(&self, p: P2) -> Option<P2> {
        let k = self.axis();
        let v = self.shift(p[k])?;
        let mut q = p;
        q[k] = v;
        Some(q)
    }

    /// The point moved like everything beyond the nearer break line.
    pub fn shift_clamped(&self, p: P2) -> P2 {
        let k = self.axis();
        let mut q = p;
        q[k] = if p[k] <= self.from {
            p[k]
        } else if p[k] >= self.to {
            p[k] - (self.to - self.from) + self.gap
        } else if p[k] - self.from < self.to - p[k] {
            self.from
        } else {
            self.from + self.gap
        };
        q
    }

    fn cut_segment(&self, a: P2, b: P2) -> Vec<[P2; 2]> {
        let k = self.axis();
        let (va, vb) = (a[k], b[k]);
        let at = |v: f64| {
            if (vb - va).abs() < 1e-15 {
                return a;
            }
            let t = (v - va) / (vb - va);
            [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
        };
        let (lo, hi) = (va.min(vb), va.max(vb));
        let mut out = Vec::new();
        // The part before the band.
        if lo < self.from {
            let (p, q) = if va < vb {
                (a, if vb > self.from { at(self.from) } else { b })
            } else {
                (if va > self.from { at(self.from) } else { a }, b)
            };
            if dist(p, q) > 1e-12 {
                out.push([p, q]);
            }
        }
        // The part after it.
        if hi > self.to {
            let (p, q) = if va < vb {
                (if va < self.to { at(self.to) } else { a }, b)
            } else {
                (a, if vb < self.to { at(self.to) } else { b })
            };
            if dist(p, q) > 1e-12 {
                out.push([p, q]);
            }
        }
        out.into_iter()
            .filter_map(|[p, q]| Some([self.shift_point(p)?, self.shift_point(q)?]))
            .collect()
    }

    /// The primitive with the band cut out and the far side moved up.
    pub fn apply(&self, prim: &Prim) -> Vec<Prim> {
        match prim {
            Prim::Line { a, b } => self
                .cut_segment(*a, *b)
                .into_iter()
                .map(|[a, b]| Prim::Line { a, b })
                .collect(),
            Prim::Arc {
                center,
                radius,
                start,
                end,
            } => {
                let k = self.axis();
                let lo = center[k] - radius;
                let hi = center[k] + radius;
                if hi <= self.from {
                    return vec![prim.clone()];
                }
                if lo >= self.to {
                    let mut c = *center;
                    c[k] = self.shift(c[k]).unwrap_or(c[k]);
                    return vec![Prim::Arc {
                        center: c,
                        radius: *radius,
                        start: *start,
                        end: *end,
                    }];
                }
                let pts = arc_points(*center, *radius, *start, *end, ARC_STEP);
                self.apply_polyline(&pts)
            }
            Prim::Polyline { points, closed } => {
                let mut pts = points.clone();
                if *closed && !pts.is_empty() {
                    pts.push(pts[0]);
                }
                let k = self.axis();
                if pts.iter().all(|p| p[k] <= self.from) {
                    return vec![prim.clone()];
                }
                if pts.iter().all(|p| p[k] >= self.to) {
                    return vec![Prim::Polyline {
                        points: points
                            .iter()
                            .map(|p| self.shift_point(*p).unwrap())
                            .collect(),
                        closed: *closed,
                    }];
                }
                self.apply_polyline(&pts)
            }
            Prim::Fill { loops } => {
                // Keep each side of the band, clipped by half-planes.
                let k = self.axis();
                let mut out = Vec::new();
                for keep_low in [true, false] {
                    let parts: Vec<Vec<P2>> = loops
                        .iter()
                        .map(|l| {
                            half_plane(l, k, if keep_low { self.from } else { self.to }, keep_low)
                        })
                        .filter(|l| l.len() >= 3)
                        .map(|l| {
                            l.into_iter()
                                .map(|p| self.shift_clamped(p))
                                .collect::<Vec<_>>()
                        })
                        .collect();
                    if !parts.is_empty() {
                        out.push(Prim::Fill { loops: parts });
                    }
                }
                out
            }
            Prim::Text { at, .. } => match self.shift_point(*at) {
                Some(p) => {
                    let mut t = prim.clone();
                    if let Prim::Text { at, .. } = &mut t {
                        *at = p;
                    }
                    vec![t]
                }
                None => Vec::new(),
            },
        }
    }

    fn apply_polyline(&self, pts: &[P2]) -> Vec<Prim> {
        let mut runs: Vec<Vec<P2>> = Vec::new();
        for w in pts.windows(2) {
            for [a, b] in self.cut_segment(w[0], w[1]) {
                match runs.last_mut() {
                    Some(r) if dist(*r.last().unwrap(), a) < 1e-9 => r.push(b),
                    _ => runs.push(vec![a, b]),
                }
            }
        }
        polyline_prims(runs)
    }
}

/// A polygon clipped to `p[k] <= v` (or `>= v`).
fn half_plane(poly: &[P2], k: usize, v: f64, below: bool) -> Vec<P2> {
    let inside = |p: P2| if below { p[k] <= v } else { p[k] >= v };
    let mut out = Vec::new();
    for j in 0..poly.len() {
        let (p, q) = (poly[j], poly[(j + 1) % poly.len()]);
        if inside(p) {
            out.push(p);
        }
        if inside(p) != inside(q) {
            let t = (v - p[k]) / (q[k] - p[k]);
            out.push([p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_clipped_to_circles_and_polygons() {
        let c = Region::Circle {
            center: [0.0, 0.0],
            radius: 1.0,
        };
        let r = clip(
            &Prim::Line {
                a: [-2.0, 0.0],
                b: [2.0, 0.0],
            },
            &c,
            true,
        );
        assert_eq!(
            r,
            vec![Prim::Line {
                a: [-1.0, 0.0],
                b: [1.0, 0.0]
            }]
        );
        let outside = clip(
            &Prim::Line {
                a: [-2.0, 0.0],
                b: [2.0, 0.0],
            },
            &c,
            false,
        );
        assert_eq!(outside.len(), 2);
        // An L-shaped (concave) polygon.
        let l = Region::Polygon(vec![
            [0.0, 0.0],
            [2.0, 0.0],
            [2.0, 1.0],
            [1.0, 1.0],
            [1.0, 2.0],
            [0.0, 2.0],
        ]);
        let r = clip(
            &Prim::Line {
                a: [-1.0, 1.5],
                b: [3.0, 1.5],
            },
            &l,
            true,
        );
        assert_eq!(
            r,
            vec![Prim::Line {
                a: [0.0, 1.5],
                b: [1.0, 1.5]
            }]
        );
    }

    #[test]
    fn arcs_inside_stay_arcs() {
        let c = Region::Circle {
            center: [0.0, 0.0],
            radius: 10.0,
        };
        let arc = Prim::Arc {
            center: [1.0, 0.0],
            radius: 2.0,
            start: 0.0,
            end: std::f64::consts::TAU,
        };
        assert_eq!(clip(&arc, &c, true), vec![arc.clone()]);
        let big = Prim::Arc {
            center: [0.0, 0.0],
            radius: 12.0,
            start: 0.0,
            end: 1.0,
        };
        assert!(clip(&big, &c, true).is_empty());
    }

    #[test]
    fn breaks_remove_a_band_and_close_up() {
        let g = Gap {
            vertical: true,
            from: 10.0,
            to: 90.0,
            gap: 5.0,
        };
        let r = g.apply(&Prim::Line {
            a: [0.0, 1.0],
            b: [100.0, 1.0],
        });
        assert_eq!(
            r,
            vec![
                Prim::Line {
                    a: [0.0, 1.0],
                    b: [10.0, 1.0]
                },
                Prim::Line {
                    a: [15.0, 1.0],
                    b: [25.0, 1.0]
                }
            ]
        );
        // Lines across the band in the other direction are unaffected.
        let r = g.apply(&Prim::Line {
            a: [95.0, 0.0],
            b: [95.0, 5.0],
        });
        assert_eq!(
            r,
            vec![Prim::Line {
                a: [20.0, 0.0],
                b: [20.0, 5.0]
            }]
        );
        assert!(
            g.apply(&Prim::Line {
                a: [50.0, 0.0],
                b: [50.0, 5.0]
            })
            .is_empty()
        );
    }

    #[test]
    fn convex_clipping_keeps_the_overlap() {
        let sq = [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]];
        let other = [[1.0, 1.0], [3.0, 1.0], [3.0, 3.0], [1.0, 3.0]];
        let r = clip_convex(&sq, &other);
        assert!((signed_area(&r).abs() - 1.0).abs() < 1e-12);
    }
}

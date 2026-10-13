//! Hidden-line removal for drawing views.
//!
//! [`Shape::hidden_lines`] projects a shape orthographically onto a view plane
//! and returns its visible (and optionally hidden) edges as 2D curves in that
//! plane's coordinates. Each edge that comes from a model edge is traced back
//! to its index in the shape, so drawing annotations can attach to model
//! geometry rather than to HLR output order.

use glam::{DVec2, DVec3};
use of_kernel_sys::ffi;

use crate::{Kind, Plane, Result, Shape, v3};

/// What kind of model feature a projected edge shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeClass {
    /// An edge between faces meeting at an angle.
    Sharp,
    /// An edge between tangent faces (a "tangent edge" in drawings).
    Smooth,
    /// A seam or other edge between faces with higher continuity.
    Seam,
    /// A silhouette of a curved face; it has no model edge.
    Outline,
}

/// A projected curve in view-plane coordinates (millimetres of model space).
#[derive(Debug, Clone, PartialEq)]
pub enum Curve2d {
    Line(DVec2, DVec2),
    /// Counter-clockwise from `start` to `end` radians, `end > start`.
    Arc {
        center: DVec2,
        radius: f64,
        start: f64,
        end: f64,
    },
    Polyline(Vec<DVec2>),
}

impl Curve2d {
    /// Points along the curve, arcs split into segments of at most `max_angle`.
    pub fn points(&self, max_angle: f64) -> Vec<DVec2> {
        match self {
            Curve2d::Line(a, b) => vec![*a, *b],
            Curve2d::Arc {
                center,
                radius,
                start,
                end,
            } => {
                let n = (((end - start) / max_angle.max(1e-3)).ceil() as usize).max(1);
                (0..=n)
                    .map(|i| {
                        let a = start + (end - start) * i as f64 / n as f64;
                        *center + DVec2::new(a.cos(), a.sin()) * *radius
                    })
                    .collect()
            }
            Curve2d::Polyline(p) => p.clone(),
        }
    }

    /// The point a fraction `t` of the way along the curve.
    pub fn at(&self, t: f64) -> DVec2 {
        match self {
            Curve2d::Line(a, b) => a.lerp(*b, t),
            Curve2d::Arc {
                center,
                radius,
                start,
                end,
            } => {
                let a = start + (end - start) * t;
                *center + DVec2::new(a.cos(), a.sin()) * *radius
            }
            Curve2d::Polyline(p) => along(p, t),
        }
    }
}

fn along(p: &[DVec2], t: f64) -> DVec2 {
    let total: f64 = p.windows(2).map(|w| w[0].distance(w[1])).sum();
    let mut left = total * t.clamp(0.0, 1.0);
    for w in p.windows(2) {
        let l = w[0].distance(w[1]);
        if left <= l && l > 0.0 {
            return w[0].lerp(w[1], left / l);
        }
        left -= l;
    }
    p.last().copied().unwrap_or_default()
}

/// One edge of a hidden-line projection.
#[derive(Debug, Clone, PartialEq)]
pub struct HlrEdge {
    pub visible: bool,
    pub class: EdgeClass,
    pub curve: Curve2d,
    /// Index of the model edge this curve is (part of), in the projected
    /// shape; `None` for outlines and edges that could not be traced.
    pub source: Option<u32>,
}

/// Accuracy of a projection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HlrQuality {
    /// Analytic algorithm: exact lines and arcs, slower.
    Exact,
    /// Polygonal algorithm on a mesh: fast, for interactive previews.
    Preview,
}

impl Shape {
    /// Visible (and with `hidden`, hidden) edges of this shape seen along
    /// `-view.normal`, with `view.x_dir` pointing right. Curves are in the
    /// view plane's coordinates, relative to `view.origin`. Non-line, non-arc
    /// curves are approximated within `deflection` millimetres.
    pub fn hidden_lines(
        &self,
        view: &Plane,
        quality: HlrQuality,
        hidden: bool,
        deflection: f64,
    ) -> Result<Vec<HlrEdge>> {
        let frame = ffi::Frame {
            origin: v3(view.origin),
            x_dir: v3(view.x_dir),
            normal: v3(view.normal),
        };
        let raw = ffi::hlr(
            &self.0,
            &frame,
            quality == HlrQuality::Exact,
            hidden,
            deflection,
        )?;
        let mut out: Vec<HlrEdge> = raw
            .into_iter()
            .filter_map(|e| {
                let class = match e.category {
                    0 => EdgeClass::Sharp,
                    1 => EdgeClass::Smooth,
                    2 => EdgeClass::Seam,
                    _ => EdgeClass::Outline,
                };
                let d = &e.data;
                let curve = match e.kind {
                    0 if d.len() == 4 => {
                        Curve2d::Line(DVec2::new(d[0], d[1]), DVec2::new(d[2], d[3]))
                    }
                    1 if d.len() == 5 => Curve2d::Arc {
                        center: DVec2::new(d[0], d[1]),
                        radius: d[2],
                        start: d[3],
                        end: d[4],
                    },
                    2 if d.len() >= 4 => {
                        straighten(d.chunks_exact(2).map(|c| DVec2::new(c[0], c[1])).collect())
                    }
                    _ => return None,
                };
                Some(HlrEdge {
                    visible: e.visible,
                    class,
                    curve,
                    source: None,
                })
            })
            .collect();
        let tol = trace_sources(self, view, deflection, &mut out);
        let seams: std::collections::HashSet<u32> = ffi::seam_edges(&self.0, &v3(view.normal)).into_iter().collect();
        out.retain(|e| e.source.is_none_or(|s| !seams.contains(&s)));
        Ok(remove_covered(out, tol))
    }
}

/// A polyline whose points are all on the line through its ends (a circle
/// or curve seen edge-on) becomes that line.
fn straighten(p: Vec<DVec2>) -> Curve2d {
    // Measure against the point farthest from the start, so a closed curve
    // seen edge-on (which returns to its start) straightens too.
    let a = p[0];
    let b = p
        .iter()
        .copied()
        .max_by(|x, y| x.distance(a).total_cmp(&y.distance(a)))
        .unwrap_or(a);
    let len = a.distance(b);
    if len > 1e-9 {
        let n = (b - a).perp() / len;
        if p.iter().all(|q| (*q - a).dot(n).abs() < 1e-7 * len.max(1.0)) {
            // Keep the extreme points: an edge-on arc may overshoot its ends.
            let d = (b - a) / len;
            let ts = p.iter().map(|q| (*q - a).dot(d));
            let (lo, hi) = ts.fold((f64::MAX, f64::MIN), |(l, h), t| (l.min(t), h.max(t)));
            return Curve2d::Line(a + d * lo, a + d * hi);
        }
    }
    Curve2d::Polyline(p)
}

/// A model edge projected onto the view plane, with depth toward the viewer.
struct Projected {
    index: u32,
    points: Vec<(DVec2, f64)>,
    min: DVec2,
    max: DVec2,
}

/// Match each non-outline HLR edge to the model edge it lies on: sample it
/// at three points and pick the model edge whose projection passes through
/// all of them. When several do (a front and a back edge seen end-on), the
/// one nearest the viewer wins, which is the one a reader sees.
fn trace_sources(shape: &Shape, view: &Plane, deflection: f64, edges: &mut [HlrEdge]) -> f64 {
    let y = view.y_dir();
    let project = |p: DVec3| {
        let d = p - view.origin;
        (DVec2::new(d.dot(view.x_dir), d.dot(y)), d.dot(view.normal))
    };
    let size = shape
        .bbox()
        .map(|b| (b.max - b.min).length())
        .unwrap_or(1.0)
        .max(1e-6);
    let tol = (deflection * 2.0).max(size * 1e-6);
    let sample = (deflection * 0.25).max(size * 1e-5);
    let projected: Vec<Projected> = (0..shape.count(Kind::Edge))
        .filter_map(|i| {
            let pts = shape.edge_samples(i, sample, 0.05).ok()?;
            if pts.len() < 2 {
                return None;
            }
            let points: Vec<(DVec2, f64)> = pts.into_iter().map(project).collect();
            let (mut min, mut max) = (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN));
            for (p, _) in &points {
                min = min.min(*p);
                max = max.max(*p);
            }
            Some(Projected {
                index: i,
                points,
                min: min - DVec2::splat(tol),
                max: max + DVec2::splat(tol),
            })
        })
        .collect();
    for e in edges.iter_mut() {
        if e.class == EdgeClass::Outline {
            continue;
        }
        let probes = [e.curve.at(0.25), e.curve.at(0.5), e.curve.at(0.75)];
        let mut best: Option<(u32, f64)> = None;
        for p in &projected {
            if probes
                .iter()
                .any(|q| q.cmplt(p.min).any() || q.cmpgt(p.max).any())
            {
                continue;
            }
            let mut depth = f64::MIN;
            let mut ok = true;
            for q in &probes {
                match nearest(&p.points, *q) {
                    Some((d, z)) if d <= tol => depth = depth.max(z),
                    _ => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok && best.is_none_or(|(_, z)| depth > z + tol) {
                best = Some((p.index, depth));
            }
        }
        e.source = best.map(|(i, _)| i);
    }
    tol
}

/// Drawings never show a line twice: drop edges lying on an edge already
/// kept, visible edges first, so a hidden edge behind a visible one (the back
/// of a box) disappears. Lines only partly covered by collinear segments keep
/// their uncovered pieces.
fn remove_covered(mut edges: Vec<HlrEdge>, tol: f64) -> Vec<HlrEdge> {
    let rank = |e: &HlrEdge| {
        let class = match e.class {
            EdgeClass::Sharp => 0,
            EdgeClass::Smooth => 1,
            EdgeClass::Outline => 2,
            EdgeClass::Seam => 3,
        };
        (!e.visible as u8, class)
    };
    edges.sort_by_key(rank);
    let mut kept: Vec<HlrEdge> = Vec::with_capacity(edges.len());
    let mut segs: Vec<(DVec2, DVec2)> = Vec::new();
    for e in edges {
        let near = |q: DVec2| segs.iter().any(|(a, b)| seg_dist(q, *a, *b) <= tol);
        let pieces = match &e.curve {
            Curve2d::Line(a, b) => uncovered(*a, *b, &segs, tol)
                .into_iter()
                .map(|(p, q)| Curve2d::Line(p, q))
                .collect(),
            c => {
                let covered = (0..=8).all(|i| near(c.at(i as f64 / 8.0)));
                if covered { vec![] } else { vec![c.clone()] }
            }
        };
        for curve in pieces {
            let pts = curve.points(0.05);
            segs.extend(pts.windows(2).map(|w| (w[0], w[1])));
            kept.push(HlrEdge {
                curve,
                ..e.clone()
            });
        }
    }
    kept
}

fn seg_dist(q: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 {
        ((q - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    q.distance(a + ab * t)
}

/// Pieces of segment `a`-`b` not covered by collinear segments in `segs`.
fn uncovered(a: DVec2, b: DVec2, segs: &[(DVec2, DVec2)], tol: f64) -> Vec<(DVec2, DVec2)> {
    let ab = b - a;
    let len = ab.length();
    if len <= tol {
        return vec![];
    }
    let dir = ab / len;
    let normal = dir.perp();
    let mut cover: Vec<(f64, f64)> = segs
        .iter()
        .filter(|(p, q)| (*p - a).dot(normal).abs() <= tol && (*q - a).dot(normal).abs() <= tol)
        .map(|(p, q)| {
            let (s, t) = ((*p - a).dot(dir), (*q - a).dot(dir));
            (s.min(t) - tol, s.max(t) + tol)
        })
        .filter(|(s, t)| *t > 0.0 && *s < len)
        .collect();
    cover.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut out = Vec::new();
    let mut at = 0.0;
    for (s, t) in cover {
        if s > at + tol {
            out.push((a + dir * at, a + dir * s.min(len)));
        }
        at = f64::max(at, t);
        if at >= len {
            break;
        }
    }
    if at < len - tol {
        out.push((a + dir * at, b));
    }
    out
}

/// Distance from `q` to a sampled polyline, and the depth at the nearest point.
fn nearest(points: &[(DVec2, f64)], q: DVec2) -> Option<(f64, f64)> {
    let mut best: Option<(f64, f64)> = None;
    for w in points.windows(2) {
        let (a, za) = w[0];
        let (b, zb) = w[1];
        let ab = b - a;
        let t = if ab.length_squared() > 0.0 {
            ((q - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let d = q.distance(a + ab * t);
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, za + (zb - za) * t));
        }
    }
    best
}

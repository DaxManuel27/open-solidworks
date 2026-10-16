//! The flat pattern as 2D geometry: the outline loops of the flat's front
//! face, with bend lines and form marks in the same frame, and the sheet
//! metal properties read from them. Flat DXF, flat pattern drawing views and
//! cut list items all start here.
//!
//! Coordinates are millimetres in the flat's front plane (the fixed face's
//! plane, seen from outside the material), so the outline reads as it is
//! cut.

use std::f64::consts::TAU;

use of_geom::{CurveKind, DVec2, DVec3, Kind, SurfaceKind};
use serde::{Deserialize, Serialize};

use crate::sheet_metal::{Flattened, flattened};
use crate::{BodyId, FeatureId, State};

pub type P2 = [f64; 2];

/// One piece of a flat outline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FlatCurve {
    Line {
        a: P2,
        b: P2,
    },
    /// Counter-clockwise from `start` to `end` radians; a full circle runs
    /// from 0 to 2π.
    Arc {
        center: P2,
        radius: f64,
        start: f64,
        end: f64,
    },
    /// Any other curve, sampled.
    Polyline {
        points: Vec<P2>,
    },
}

impl FlatCurve {
    fn ends(&self) -> (DVec2, DVec2) {
        match self {
            FlatCurve::Line { a, b } => (DVec2::from(*a), DVec2::from(*b)),
            FlatCurve::Arc {
                center,
                radius,
                start,
                end,
            } => {
                let c = DVec2::from(*center);
                (
                    c + DVec2::from_angle(*start) * *radius,
                    c + DVec2::from_angle(*end) * *radius,
                )
            }
            FlatCurve::Polyline { points } => {
                (DVec2::from(points[0]), DVec2::from(*points.last().unwrap()))
            }
        }
    }

    /// Points along the curve, no more than 5° apart on arcs.
    pub fn points(&self) -> Vec<P2> {
        match self {
            FlatCurve::Line { a, b } => vec![*a, *b],
            FlatCurve::Arc {
                center,
                radius,
                start,
                end,
            } => {
                let sweep = end - start;
                let n = ((sweep / 5f64.to_radians()).ceil() as usize).max(1);
                (0..=n)
                    .map(|i| {
                        let t = start + sweep * i as f64 / n as f64;
                        (DVec2::from(*center) + DVec2::from_angle(t) * *radius).to_array()
                    })
                    .collect()
            }
            FlatCurve::Polyline { points } => points.clone(),
        }
    }
}

/// A closed loop of the outline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlatLoop {
    pub curves: Vec<FlatCurve>,
    /// Cutting length, mm.
    pub length: f64,
    /// The outside of the part; the others are cut-outs.
    pub outer: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlatBendLine {
    pub a: P2,
    pub b: P2,
    /// The flange bends toward the viewer (out of the front face).
    pub up: bool,
    /// Radians.
    pub angle: f64,
    /// Inside radius, mm.
    pub radius: f64,
    pub feature: Option<FeatureId>,
    pub index: Option<u32>,
}

/// A form undone in the flat: its punch outline and center.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlatMark {
    pub feature: FeatureId,
    pub center: P2,
    pub outline: Vec<P2>,
    pub punch_id: Option<String>,
    /// Radians, in the flat.
    pub angle: f64,
}

/// A flat pattern in 2D.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlatOutline {
    pub loops: Vec<FlatLoop>,
    pub bend_lines: Vec<FlatBendLine>,
    pub marks: Vec<FlatMark>,
    pub thickness: f64,
}

impl FlatOutline {
    /// Every outline point, for extents.
    pub fn points(&self) -> Vec<P2> {
        self.loops
            .iter()
            .flat_map(|l| l.curves.iter().flat_map(|c| c.points()))
            .collect()
    }

    /// Smallest and largest corners of the outline.
    pub fn extents(&self) -> Option<(P2, P2)> {
        let pts = self.points();
        if pts.is_empty() {
            return None;
        }
        let mut lo = [f64::MAX; 2];
        let mut hi = [f64::MIN; 2];
        for p in pts {
            lo = [lo[0].min(p[0]), lo[1].min(p[1])];
            hi = [hi[0].max(p[0]), hi[1].max(p[1])];
        }
        Some((lo, hi))
    }

    /// The smallest rectangle round the outline over all rotations: (length,
    /// width), length the longer side.
    pub fn bounding_rectangle(&self) -> (f64, f64) {
        let hull = convex_hull(self.points().into_iter().map(DVec2::from).collect());
        if hull.len() < 2 {
            return (0.0, 0.0);
        }
        let mut best = (f64::MAX, 0.0, 0.0);
        for i in 0..hull.len() {
            let d = hull[(i + 1) % hull.len()] - hull[i];
            if d.length() < 1e-12 {
                continue;
            }
            let u = d.normalize();
            let v = u.perp();
            let (mut ulo, mut uhi, mut vlo, mut vhi) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
            for p in &hull {
                let (a, b) = (p.dot(u), p.dot(v));
                (ulo, uhi, vlo, vhi) = (ulo.min(a), uhi.max(a), vlo.min(b), vhi.max(b));
            }
            let (w, h) = (uhi - ulo, vhi - vlo);
            if w * h < best.0 - 1e-12 {
                best = (w * h, w.max(h), w.min(h));
            }
        }
        (best.1, best.2)
    }
}

fn convex_hull(mut pts: Vec<DVec2>) -> Vec<DVec2> {
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup_by(|a, b| (*a - *b).length() < 1e-9);
    if pts.len() < 3 {
        return pts;
    }
    let mut hull: Vec<DVec2> = Vec::with_capacity(pts.len() * 2);
    for pass in 0..2 {
        let start = hull.len();
        let iter: Box<dyn Iterator<Item = &DVec2>> = if pass == 0 {
            Box::new(pts.iter())
        } else {
            Box::new(pts.iter().rev())
        };
        for p in iter {
            while hull.len() >= start + 2 {
                let (a, b) = (hull[hull.len() - 2], hull[hull.len() - 1]);
                if (b - a).perp_dot(*p - a) > 1e-12 {
                    break;
                }
                hull.pop();
            }
            hull.push(*p);
        }
        hull.pop();
    }
    hull
}

/// The flat pattern of `flat` in 2D.
pub fn outline(flat: &Flattened) -> Result<FlatOutline, String> {
    let shape = &flat.shape;
    let plane = &flat.plane;
    let scale = shape.bbox().map_or(1.0, |b| b.diagonal().max(1.0));
    let front: Vec<u32> = (0..shape.count(Kind::Face))
        .filter(|f| {
            shape.face_info(*f).is_ok_and(|i| {
                i.kind == SurfaceKind::Plane
                    && i.normal.dot(plane.normal) > 1.0 - 1e-9
                    && (i.point - plane.origin).dot(plane.normal).abs() < 1e-6 * scale
            })
        })
        .collect();
    if front.is_empty() {
        return Err("the flat pattern has no face on its front plane".into());
    }
    let edge_faces = shape.edge_faces();
    let local = |p: DVec3| plane.to_local(p);
    let mut curves = Vec::new();
    for (e, faces) in edge_faces.iter().enumerate() {
        // Edges between two front faces are seams, not outline.
        if faces.iter().filter(|f| front.contains(f)).count() != 1 {
            continue;
        }
        let e = e as u32;
        let info = shape.edge_info(e).map_err(|e| e.0)?;
        if info.kind == CurveKind::Degenerate || info.length < 1e-9 {
            continue;
        }
        let curve = match info.kind {
            CurveKind::Line => FlatCurve::Line {
                a: local(info.start).to_array(),
                b: local(info.end).to_array(),
            },
            CurveKind::Circle if info.axis.cross(plane.normal).length() < 1e-9 => {
                let c = local(info.center);
                if info.closed {
                    FlatCurve::Arc {
                        center: c.to_array(),
                        radius: info.radius,
                        start: 0.0,
                        end: TAU,
                    }
                } else {
                    let angle = |p: DVec3| {
                        let d = local(p) - c;
                        d.y.atan2(d.x).rem_euclid(TAU)
                    };
                    let (s, m, t) = (angle(info.start), angle(info.mid), angle(info.end));
                    // Counter-clockwise from s to t must pass m; else t to s.
                    let ccw = |from: f64, to: f64, x: f64| {
                        (x - from).rem_euclid(TAU) < (to - from).rem_euclid(TAU)
                    };
                    let (start, end) = if ccw(s, t, m) { (s, t) } else { (t, s) };
                    let end = start + (end - start).rem_euclid(TAU);
                    FlatCurve::Arc {
                        center: c.to_array(),
                        radius: info.radius,
                        start,
                        end,
                    }
                }
            }
            _ => FlatCurve::Polyline {
                points: shape
                    .edge_samples(e, 1e-3, 0.05)
                    .map_err(|e| e.0)?
                    .into_iter()
                    .map(|p| local(p).to_array())
                    .collect(),
            },
        };
        if let FlatCurve::Polyline { points } = &curve
            && points.len() < 2
        {
            continue;
        }
        curves.push((curve, info.length));
    }
    let loops = chain(curves, 1e-5 * scale)?;

    let point = |p: [f64; 3]| local(DVec3::from(p)).to_array();
    let bend_lines = flat
        .bend_lines
        .iter()
        .map(|l| FlatBendLine {
            a: point(l.a),
            b: point(l.b),
            up: l.up,
            angle: l.angle,
            radius: l.radius,
            feature: l.feature,
            index: l.index,
        })
        .collect();
    let marks = flat
        .marks
        .iter()
        .map(|m| FlatMark {
            feature: m.feature,
            center: point(m.center),
            outline: m.outline.iter().map(|p| point(*p)).collect(),
            punch_id: m.punch_id.clone(),
            angle: m.angle,
        })
        .collect();
    Ok(FlatOutline {
        loops,
        bend_lines,
        marks,
        thickness: flat.thickness,
    })
}

/// Join curves end to end into closed loops; the loop with the largest
/// extents is the outside.
fn chain(curves: Vec<(FlatCurve, f64)>, tol: f64) -> Result<Vec<FlatLoop>, String> {
    let mut left: Vec<Option<(FlatCurve, f64)>> = curves.into_iter().map(Some).collect();
    let mut loops = Vec::new();
    while let Some(first) = left.iter().position(Option::is_some) {
        let (curve, length) = left[first].take().unwrap();
        let (start, mut end) = curve.ends();
        let mut out = vec![curve];
        let mut total = length;
        while (end - start).length() > tol {
            let next = left.iter().position(|c| {
                c.as_ref().is_some_and(|(c, _)| {
                    let (a, b) = c.ends();
                    (a - end).length() <= tol || (b - end).length() <= tol
                })
            });
            let Some(next) = next else {
                return Err("the flat pattern's outline does not close".into());
            };
            let (curve, length) = left[next].take().unwrap();
            let (a, b) = curve.ends();
            end = if (a - end).length() <= tol { b } else { a };
            out.push(curve);
            total += length;
        }
        loops.push(FlatLoop {
            curves: out,
            length: total,
            outer: false,
        });
    }
    let area = |l: &FlatLoop| {
        let pts: Vec<P2> = l.curves.iter().flat_map(|c| c.points()).collect();
        let (lo, hi) = pts
            .iter()
            .fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| {
                (
                    [lo[0].min(p[0]), lo[1].min(p[1])],
                    [hi[0].max(p[0]), hi[1].max(p[1])],
                )
            });
        (hi[0] - lo[0]) * (hi[1] - lo[1])
    };
    if let Some(i) = (0..loops.len()).max_by(|a, b| area(&loops[*a]).total_cmp(&area(&loops[*b]))) {
        loops[i].outer = true;
    }
    Ok(loops)
}

/// Cut list properties of a sheet metal body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetProperties {
    /// The body, when it still exists.
    pub body: Option<BodyId>,
    pub thickness: f64,
    /// Smallest rectangle round the flat over all rotations, mm.
    pub flat_length: f64,
    pub flat_width: f64,
    /// Area of the flat blank's front face, mm².
    pub flat_area: f64,
    pub bends: usize,
    /// Cutting length of the outside, mm.
    pub outer_cut: f64,
    /// Cutting length of every cut-out, mm.
    pub inner_cut: f64,
    pub cutouts: usize,
    /// Forms pressed into the body.
    pub forms: usize,
    /// The gauge, when the body takes it from a gauge table.
    pub gauge: Option<String>,
}

/// The flat of sheet metal body `index` and its cut list properties.
pub fn sheet_properties(
    state: &State,
    index: usize,
) -> Result<(FlatOutline, SheetProperties), String> {
    let record = state
        .sheet_metal
        .get(index)
        .ok_or("no such sheet metal body")?;
    let flat = flattened(state, index)?;
    let outline = outline(&flat)?;
    let (flat_length, flat_width) = outline.bounding_rectangle();
    let volume = flat.shape.mass_props().map_err(|e| e.0)?.volume;
    let body = crate::ops::sheet::record_body(state, record).map(|b| state.bodies[b].id.clone());
    let props = SheetProperties {
        body,
        thickness: flat.thickness,
        flat_length,
        flat_width,
        flat_area: volume / flat.thickness,
        bends: record.bends.len(),
        outer_cut: outline
            .loops
            .iter()
            .filter(|l| l.outer)
            .map(|l| l.length)
            .sum(),
        inner_cut: outline
            .loops
            .iter()
            .filter(|l| !l.outer)
            .map(|l| l.length)
            .sum(),
        cutouts: outline.loops.iter().filter(|l| !l.outer).count(),
        forms: outline.marks.len(),
        gauge: record.params.gauge.as_ref().map(|g| g.gauge.clone()),
    };
    Ok((outline, props))
}

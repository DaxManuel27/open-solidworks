//! Sketch tools that create or edit several entities at once, adding the
//! relations a user would expect (a rectangle comes out with horizontal and
//! vertical relations, a slot with tangencies, and so on).

use std::f64::consts::{PI, TAU};

use glam::DVec2;

use crate::geom2::{Curve2, arc_angles, intersect, line_circle, line_line};
use crate::{ConstraintKind as K, Entity, Geometry, Id, Result, Sketch, SketchError};

/// Entities created by a tool.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Created {
    pub entities: Vec<Id>,
    pub constraints: Vec<Id>,
}

impl Created {
    fn c(&mut self, sk: &mut Sketch, k: K) {
        if let Ok(id) = sk.add_constraint(k) {
            self.constraints.push(id);
        }
    }
}

/// Rectangle from two opposite corners, with horizontal and vertical
/// relations. Entities: 4 points then 4 lines (bottom, right, top, left).
pub fn corner_rectangle(sk: &mut Sketch, p0: DVec2, p1: DVec2) -> Created {
    let (lo, hi) = (p0.min(p1), p0.max(p1));
    let corners = [lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)];
    let pts: Vec<Id> = corners.iter().map(|c| sk.add_point(*c)).collect();
    let lines: Vec<Id> = (0..4)
        .map(|i| sk.add_line(pts[i], pts[(i + 1) % 4]).unwrap())
        .collect();
    let mut out = Created {
        entities: [pts, lines.clone()].concat(),
        constraints: vec![],
    };
    out.c(sk, K::Horizontal { line: lines[0] });
    out.c(sk, K::Horizontal { line: lines[2] });
    out.c(sk, K::Vertical { line: lines[1] });
    out.c(sk, K::Vertical { line: lines[3] });
    out
}

/// Rectangle from its center and a corner, with construction diagonals
/// meeting at a center point.
pub fn center_rectangle(sk: &mut Sketch, center: DVec2, corner: DVec2) -> Created {
    let h = (corner - center).abs();
    let mut out = corner_rectangle(sk, center - h, center + h);
    let pts = out.entities[..4].to_vec();
    let c = sk.add_point(center);
    let d1 = sk.add_line(pts[0], pts[2]).unwrap();
    let d2 = sk.add_line(pts[1], pts[3]).unwrap();
    for d in [d1, d2] {
        sk.entities.get_mut(&d).unwrap().construction = true;
    }
    out.entities.extend([c, d1, d2]);
    // One midpoint relation is enough: a rectangle's diagonals bisect each other.
    out.c(sk, K::Midpoint { point: c, line: d1 });
    out
}

/// Regular polygon with `sides` sides inscribed in a construction circle.
pub fn polygon(sk: &mut Sketch, center: DVec2, vertex: DVec2, sides: usize) -> Result<Created> {
    if sides < 3 {
        return Err(SketchError::Invalid(
            "a polygon needs at least 3 sides".into(),
        ));
    }
    let r = (vertex - center).length();
    if r < 1e-9 {
        return Err(SketchError::Invalid("polygon is too small".into()));
    }
    let a0 = (vertex - center).to_angle();
    let circle = sk.add_circle_at(center, r);
    sk.entities.get_mut(&circle).unwrap().construction = true;
    let pts: Vec<Id> = (0..sides)
        .map(|i| sk.add_point(center + DVec2::from_angle(a0 + TAU * i as f64 / sides as f64) * r))
        .collect();
    let lines: Vec<Id> = (0..sides)
        .map(|i| sk.add_line(pts[i], pts[(i + 1) % sides]).unwrap())
        .collect();
    let mut out = Created {
        entities: vec![circle],
        constraints: vec![],
    };
    out.entities.extend(&pts);
    out.entities.extend(&lines);
    for (i, &p) in pts.iter().enumerate() {
        out.c(
            sk,
            K::PointOnCurve {
                point: p,
                curve: circle,
            },
        );
        if i > 0 {
            out.c(
                sk,
                K::Equal {
                    a: lines[0],
                    b: lines[i],
                },
            );
        }
    }
    Ok(out)
}

/// Straight slot between two arc centers, `width` wide.
pub fn straight_slot(sk: &mut Sketch, c0: DVec2, c1: DVec2, width: f64) -> Result<Created> {
    let axis = c1 - c0;
    if axis.length() < 1e-9 || width <= 0.0 {
        return Err(SketchError::Invalid(
            "slot needs a length and a width".into(),
        ));
    }
    let n = axis.normalize().perp() * (width / 2.0);
    let (a, b) = (sk.add_point(c0), sk.add_point(c1));
    let p = [c0 - n, c1 - n, c1 + n, c0 + n].map(|q| sk.add_point(q));
    let bottom = sk.add_line(p[0], p[1]).unwrap();
    let top = sk.add_line(p[2], p[3]).unwrap();
    let arc1 = sk.add_arc(b, p[1], p[2]).unwrap();
    let arc0 = sk.add_arc(a, p[3], p[0]).unwrap();
    let center_line = sk.add_line(a, b).unwrap();
    sk.entities.get_mut(&center_line).unwrap().construction = true;
    let mut out = Created {
        entities: vec![a, b],
        constraints: vec![],
    };
    out.entities.extend(p);
    out.entities.extend([bottom, top, arc1, arc0, center_line]);
    for line in [bottom, top] {
        for arc in [arc0, arc1] {
            out.c(
                sk,
                K::Tangent {
                    a: line,
                    b: arc,
                    inside: false,
                },
            );
        }
    }
    out.c(sk, K::Equal { a: arc0, b: arc1 });
    Ok(out)
}

/// Arc through three points: from `start` to `end`, passing `via`.
pub fn three_point_arc(sk: &mut Sketch, start: Id, end: Id, via: DVec2) -> Result<Id> {
    let (s, e) = (sk.point(start)?, sk.point(end)?);
    let center = circumcenter(s, via, e)
        .ok_or_else(|| SketchError::Invalid("the three points are in a line".into()))?;
    let c = sk.add_point(center);
    // Arcs run counter-clockwise; flip the ends when `via` is on the other side.
    if (via - s).perp_dot(e - s) > 0.0 {
        sk.add_arc(c, start, end)
    } else {
        sk.add_arc(c, end, start)
    }
}

/// Arc from `start` tangent to `tangent` (a direction), ending at `end`.
pub fn tangent_arc(sk: &mut Sketch, start: Id, tangent: DVec2, end: Id) -> Result<Id> {
    let (s, e) = (sk.point(start)?, sk.point(end)?);
    let n = tangent.normalize_or_zero().perp();
    // Center lies on the normal at `s` and is equidistant from s and e.
    let d = e - s;
    let den = 2.0 * n.dot(d);
    if den.abs() < 1e-12 {
        return Err(SketchError::Invalid(
            "the end point is in line with the tangent".into(),
        ));
    }
    let t = d.length_squared() / den;
    let c = sk.add_point(s + n * t);
    if t > 0.0 {
        sk.add_arc(c, start, end)
    } else {
        sk.add_arc(c, end, start)
    }
}

/// Circle center through three points, if they aren't collinear.
pub fn circumcenter(a: DVec2, b: DVec2, c: DVec2) -> Option<DVec2> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-12 {
        return None;
    }
    let (a2, b2, c2) = (a.length_squared(), b.length_squared(), c.length_squared());
    Some(DVec2::new(
        (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d,
        (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d,
    ))
}

/// Points where `id` meets other curves, as normalised parameters along it,
/// with the curve hit.
fn cuts(sk: &Sketch, id: Id, curve: &Curve2) -> Vec<(f64, Id)> {
    let mut out = Vec::new();
    for other in sk.curve_ids().filter(|o| *o != id).collect::<Vec<_>>() {
        let oc = sk.curve(other).unwrap();
        for p in intersect(curve, &oc) {
            out.push((curve.param_of(p), other));
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// Point id at `p` for the end of a trimmed curve. Reuses `old` when no
/// other curve shares it, so relations on it survive.
fn end_point(sk: &mut Sketch, old: Id, p: DVec2, owner: Id) -> Id {
    let shared = sk
        .entities
        .iter()
        .any(|(id, e)| *id != owner && e.point_refs().contains(&old));
    if shared {
        sk.add_point(p)
    } else {
        sk.set_point(old, p).unwrap();
        old
    }
}

/// Power trim: remove the part of `id` between the intersections either side
/// of `pick`. Removes the whole entity if nothing crosses it.
pub fn trim(sk: &mut Sketch, id: Id, pick: DVec2) -> Result<Created> {
    let curve = sk.curve(id)?;
    let geo = sk.entity(id)?.geo.clone();
    let t = curve.param_of(pick);
    let hits = cuts(sk, id, &curve);
    let eps = 1e-7;
    let inner: Vec<(f64, Id)> = hits
        .into_iter()
        .filter(|(u, _)| *u > eps && *u < 1.0 - eps)
        .collect();
    let lo = inner.iter().rev().find(|(u, _)| *u < t).copied();
    let hi = inner.iter().find(|(u, _)| *u > t).copied();
    let mut out = Created::default();

    match geo {
        Geometry::Circle { center, .. } => {
            // Periodic: keep the arc from the cut after the pick round to the cut before it.
            let all: Vec<(f64, Id)> = cuts(sk, id, &curve);
            if all.len() < 2 {
                sk.delete(&[id]);
                return Ok(out);
            }
            let after = all
                .iter()
                .find(|(u, _)| *u > t)
                .or(all.first())
                .copied()
                .unwrap();
            let before = all
                .iter()
                .rev()
                .find(|(u, _)| *u < t)
                .or(all.last())
                .copied()
                .unwrap();
            let s = sk.add_point(curve.point_at(after.0));
            let e = sk.add_point(curve.point_at(before.0));
            let arc = sk.add_arc(center, s, e)?;
            retarget(sk, id, arc);
            sk.delete(&[id]);
            out.entities.push(arc);
            out.c(
                sk,
                K::PointOnCurve {
                    point: s,
                    curve: after.1,
                },
            );
            out.c(
                sk,
                K::PointOnCurve {
                    point: e,
                    curve: before.1,
                },
            );
        }
        Geometry::Line { .. } | Geometry::Arc { .. } => {
            let (start, end) = match geo {
                Geometry::Line { a, b } => (a, b),
                Geometry::Arc { start, end, .. } => (start, end),
                _ => unreachable!(),
            };
            match (lo, hi) {
                (None, None) => sk.delete(&[id]),
                (Some((u, other)), None) => {
                    let p = end_point(sk, end, curve.point_at(u), id);
                    set_end(sk, id, false, p);
                    out.c(
                        sk,
                        K::PointOnCurve {
                            point: p,
                            curve: other,
                        },
                    );
                }
                (None, Some((u, other))) => {
                    let p = end_point(sk, start, curve.point_at(u), id);
                    set_end(sk, id, true, p);
                    out.c(
                        sk,
                        K::PointOnCurve {
                            point: p,
                            curve: other,
                        },
                    );
                }
                (Some((u0, o0)), Some((u1, o1))) => {
                    let p0 = sk.add_point(curve.point_at(u0));
                    let p1 = sk.add_point(curve.point_at(u1));
                    // Second piece from the far cut to the original end.
                    let piece = match geo {
                        Geometry::Line { .. } => sk.add_line(p1, end)?,
                        Geometry::Arc { center, .. } => sk.add_arc(center, p1, end)?,
                        _ => unreachable!(),
                    };
                    set_end(sk, id, false, p0);
                    out.entities.push(piece);
                    out.c(
                        sk,
                        K::PointOnCurve {
                            point: p0,
                            curve: o0,
                        },
                    );
                    out.c(
                        sk,
                        K::PointOnCurve {
                            point: p1,
                            curve: o1,
                        },
                    );
                    if matches!(geo, Geometry::Line { .. }) {
                        out.c(sk, K::Collinear { a: id, b: piece });
                    }
                }
            }
        }
        _ => {
            return Err(SketchError::Invalid(
                "trim works on lines, arcs and circles".into(),
            ));
        }
    }
    Ok(out)
}

/// Point a curve's start (`at_start`) or end at a different point.
fn set_end(sk: &mut Sketch, id: Id, at_start: bool, p: Id) {
    let old = match &mut sk.entities.get_mut(&id).unwrap().geo {
        Geometry::Line { a, b } => std::mem::replace(if at_start { a } else { b }, p),
        Geometry::Arc { start, end, .. } => {
            std::mem::replace(if at_start { start } else { end }, p)
        }
        _ => return,
    };
    if old != p && !sk.entities.values().any(|e| e.point_refs().contains(&old)) {
        sk.delete(&[old]);
    }
}

/// Move relations from one entity to another (used when a circle becomes an arc).
fn retarget(sk: &mut Sketch, from: Id, to: Id) {
    let ids: Vec<Id> = sk.constraints.keys().copied().collect();
    for cid in ids {
        let c = sk.constraints.get_mut(&cid).unwrap();
        if let Some(k) = retarget_kind(&c.kind, from, to) {
            c.kind = k;
        }
    }
}

fn retarget_kind(k: &K, from: Id, to: Id) -> Option<K> {
    let r = |x: Id| if x == from { to } else { x };
    if !k.refs().contains(&from) {
        return None;
    }
    Some(match k.clone() {
        K::Radius { curve, value } => K::Radius {
            curve: r(curve),
            value,
        },
        K::Diameter { curve, value } => K::Diameter {
            curve: r(curve),
            value,
        },
        K::Equal { a, b } => K::Equal { a: r(a), b: r(b) },
        K::Concentric { a, b } => K::Concentric { a: r(a), b: r(b) },
        K::Tangent { a, b, inside } => K::Tangent {
            a: r(a),
            b: r(b),
            inside,
        },
        K::PointOnCurve { point, curve } => K::PointOnCurve {
            point,
            curve: r(curve),
        },
        K::Distance { a, b, value } => K::Distance {
            a: r(a),
            b: r(b),
            value,
        },
        _ => return None,
    })
}

/// Extend a line from the end nearest `pick` to the next curve it meets.
pub fn extend(sk: &mut Sketch, id: Id, pick: DVec2) -> Result<Created> {
    let Geometry::Line { a, b } = sk.entity(id)?.geo else {
        return Err(SketchError::Invalid("extend works on lines".into()));
    };
    let (pa, pb) = (sk.point(a)?, sk.point(b)?);
    let from_b = (pick - pb).length() <= (pick - pa).length();
    let (fixed, moving) = if from_b { (pa, pb) } else { (pb, pa) };
    let dir = (moving - fixed).normalize();
    let mut best: Option<(f64, DVec2, Id)> = None;
    for other in sk.curve_ids().filter(|o| *o != id).collect::<Vec<_>>() {
        let oc = sk.curve(other)?;
        let hits: Vec<DVec2> = match &oc {
            Curve2::Line { a: r, b: s } => line_line(moving, moving + dir, *r, *s, true)
                .filter(|p| {
                    let seg = Curve2::Line { a: *r, b: *s };
                    (seg.closest_point(*p) - *p).length() < 1e-7
                })
                .into_iter()
                .collect(),
            Curve2::Circle { center, radius } => {
                line_circle(moving, moving + dir, *center, *radius)
                    .into_iter()
                    .map(|h| h.1)
                    .collect()
            }
            Curve2::Arc { center, start, .. } => {
                line_circle(moving, moving + dir, *center, (*start - *center).length())
                    .into_iter()
                    .map(|h| h.1)
                    .filter(|p| (oc.closest_point(*p) - *p).length() < 1e-7)
                    .collect()
            }
            Curve2::Spline { .. } => vec![],
        };
        for p in hits {
            let d = (p - moving).dot(dir);
            if d > 1e-7 && best.is_none_or(|b| d < b.0) {
                best = Some((d, p, other));
            }
        }
    }
    let Some((_, p, other)) = best else {
        return Err(SketchError::Invalid("nothing to extend to".into()));
    };
    let pid = if from_b { b } else { a };
    let p = end_point(sk, pid, p, id);
    set_end(sk, id, !from_b, p);
    let mut out = Created::default();
    out.c(
        sk,
        K::PointOnCurve {
            point: p,
            curve: other,
        },
    );
    Ok(out)
}

/// Offset a chain of lines and arcs (or a single circle) by `distance` to its
/// left (negative for right). Joints between offset curves are re-intersected
/// so the new chain stays connected.
pub fn offset(sk: &mut Sketch, ids: &[Id], distance: f64) -> Result<Created> {
    let mut out = Created::default();
    if ids.is_empty() || distance == 0.0 {
        return Ok(out);
    }
    if ids.len() == 1
        && let Geometry::Circle { center, radius } = sk.entity(ids[0])?.geo
    {
        let r = radius - distance;
        if r <= 0.0 {
            return Err(SketchError::Invalid(
                "offset is larger than the circle".into(),
            ));
        }
        let c = sk.add_circle(center, r)?;
        out.entities.push(c);
        out.c(sk, K::Concentric { a: ids[0], b: c });
        return Ok(out);
    }
    let chain = order_chain(sk, ids)?;
    let n = chain.len();
    let closed = n > 1 && (chain[0].start() - chain[n - 1].end()).length() < 1e-6;
    let offs: Vec<Oriented> = chain
        .iter()
        .map(|c| c.offset(distance))
        .collect::<Result<_>>()?;
    let mut joints: Vec<DVec2> = (0..n)
        .map(|i| match (i, closed) {
            (0, false) => offs[0].start(),
            (0, true) => join(&offs[n - 1].c, &offs[0].c, offs[0].start()),
            _ => join(&offs[i - 1].c, &offs[i].c, offs[i].start()),
        })
        .collect();
    if !closed {
        joints.push(offs[n - 1].end());
    }
    let mut pts: Vec<Id> = joints.iter().map(|p| sk.add_point(*p)).collect();
    if closed {
        pts.push(pts[0]);
    }
    for (i, o) in offs.iter().enumerate() {
        let (s, e) = (pts[i], pts[i + 1]);
        let id = match o.c {
            Curve2::Line { .. } => sk.add_line(s, e)?,
            Curve2::Arc { center, .. } => {
                let cid = sk.add_point(center);
                if o.rev {
                    sk.add_arc(cid, e, s)?
                } else {
                    sk.add_arc(cid, s, e)?
                }
            }
            _ => {
                return Err(SketchError::Invalid(
                    "offset works on lines, arcs and circles".into(),
                ));
            }
        };
        out.entities.push(id);
    }
    Ok(out)
}

/// A curve with a direction of travel; `rev` means it runs end to start.
#[derive(Debug, Clone)]
struct Oriented {
    c: Curve2,
    rev: bool,
}

impl Oriented {
    fn start(&self) -> DVec2 {
        if self.rev {
            self.c.end()
        } else {
            self.c.start()
        }
    }

    fn end(&self) -> DVec2 {
        if self.rev {
            self.c.start()
        } else {
            self.c.end()
        }
    }

    /// Offset to the left of the direction of travel.
    fn offset(&self, d: f64) -> Result<Oriented> {
        let c = match &self.c {
            Curve2::Line { a, b } => {
                let n = (self.end() - self.start()).normalize().perp() * d;
                Curve2::Line {
                    a: *a + n,
                    b: *b + n,
                }
            }
            Curve2::Arc { center, start, end } => {
                let r = (*start - *center).length();
                // Left of a counter-clockwise arc is toward its center.
                let nr = if self.rev { r + d } else { r - d };
                if nr <= 0.0 {
                    return Err(SketchError::Invalid(
                        "offset is larger than an arc's radius".into(),
                    ));
                }
                let k = nr / r;
                Curve2::Arc {
                    center: *center,
                    start: *center + (*start - *center) * k,
                    end: *center + (*end - *center) * k,
                }
            }
            _ => {
                return Err(SketchError::Invalid(
                    "offset works on lines and arcs".into(),
                ));
            }
        };
        Ok(Oriented { c, rev: self.rev })
    }
}

/// Order curves into a connected chain, noting which run backwards.
fn order_chain(sk: &Sketch, ids: &[Id]) -> Result<Vec<Oriented>> {
    let mut rest: Vec<Curve2> = ids.iter().map(|id| sk.curve(*id)).collect::<Result<_>>()?;
    let near = |a: DVec2, b: DVec2| (a - b).length() < 1e-6;
    let mut chain = vec![Oriented {
        c: rest.remove(0),
        rev: false,
    }];
    while !rest.is_empty() {
        let (end, start) = (chain[chain.len() - 1].end(), chain[0].start());
        let Some(i) = rest.iter().position(|c| {
            near(c.start(), end)
                || near(c.end(), end)
                || near(c.start(), start)
                || near(c.end(), start)
        }) else {
            return Err(SketchError::Invalid("offset needs connected curves".into()));
        };
        let c = rest.remove(i);
        if near(c.start(), end) {
            chain.push(Oriented { c, rev: false });
        } else if near(c.end(), end) {
            chain.push(Oriented { c, rev: true });
        } else if near(c.end(), start) {
            chain.insert(0, Oriented { c, rev: false });
        } else {
            chain.insert(0, Oriented { c, rev: true });
        }
    }
    Ok(chain)
}

/// Where two consecutive offset curves meet: their intersection nearest the
/// original joint, or the start of the second if they don't meet.
fn join(prev: &Curve2, next: &Curve2, near: DVec2) -> DVec2 {
    let extended = |c: &Curve2| -> Curve2 {
        match c {
            Curve2::Line { a, b } => {
                let d = (*b - *a) * 1e4;
                Curve2::Line {
                    a: *a - d,
                    b: *b + d,
                }
            }
            Curve2::Arc { center, start, .. } => Curve2::Circle {
                center: *center,
                radius: (*start - *center).length(),
            },
            c => c.clone(),
        }
    };
    let hits = intersect(&extended(prev), &extended(next));
    hits.into_iter()
        .min_by(|a, b| (*a - near).length().total_cmp(&(*b - near).length()))
        .unwrap_or(next.start())
}

/// Mirror entities about a line, adding symmetric relations between
/// matching points.
pub fn mirror(sk: &mut Sketch, ids: &[Id], about: Id) -> Result<Created> {
    let (p, q) = sk.line(about)?;
    let axis = (q - p).normalize();
    let reflect = |x: DVec2| {
        let v = x - p;
        p + axis * (2.0 * v.dot(axis)) - v
    };
    let mut out = Created::default();
    let mut map: std::collections::HashMap<Id, Id> = std::collections::HashMap::new();
    let mut point_for = |sk: &mut Sketch, out: &mut Created, id: Id| -> Id {
        if let Some(m) = map.get(&id) {
            return *m;
        }
        let pos = sk.point(id).unwrap();
        // Points on the mirror line map to themselves.
        let m = if (reflect(pos) - pos).length() < 1e-9 {
            id
        } else {
            sk.add_point(reflect(pos))
        };
        if m != id {
            out.c(sk, K::Symmetric { a: id, b: m, about });
        }
        map.insert(id, m);
        m
    };
    for &id in ids {
        if id == about {
            continue;
        }
        let e = sk.entity(id)?.clone();
        let geo = match e.geo.clone() {
            Geometry::Point { .. } => {
                point_for(sk, &mut out, id);
                continue;
            }
            Geometry::Line { a, b } => Geometry::Line {
                a: point_for(sk, &mut out, a),
                b: point_for(sk, &mut out, b),
            },
            Geometry::Circle { center, radius } => Geometry::Circle {
                center: point_for(sk, &mut out, center),
                radius,
            },
            // Mirroring reverses direction, so start and end swap.
            Geometry::Arc { center, start, end } => Geometry::Arc {
                center: point_for(sk, &mut out, center),
                start: point_for(sk, &mut out, end),
                end: point_for(sk, &mut out, start),
            },
            Geometry::Spline { points, closed } => Geometry::Spline {
                points: points.iter().map(|p| point_for(sk, &mut out, *p)).collect(),
                closed,
            },
        };
        let new = sk.add_entity(Entity {
            geo,
            construction: e.construction,
            external: None,
        })?;
        out.entities.push(new);
        if let Geometry::Circle { .. } | Geometry::Arc { .. } = e.geo {
            out.c(sk, K::Equal { a: id, b: new });
        }
    }
    Ok(out)
}

/// Round the corner where two lines meet at `corner` with an arc of `radius`.
pub fn sketch_fillet(sk: &mut Sketch, corner: Id, radius: f64) -> Result<Created> {
    let lines: Vec<Id> = sk
        .entities
        .iter()
        .filter(|(_, e)| matches!(e.geo, Geometry::Line { a, b } if a == corner || b == corner))
        .map(|(id, _)| *id)
        .collect();
    if lines.len() != 2 {
        return Err(SketchError::Invalid(
            "select a corner where exactly two lines meet".into(),
        ));
    }
    let c = sk.point(corner)?;
    let far = |sk: &Sketch, l: Id| -> DVec2 {
        let (a, b) = sk.line(l).unwrap();
        if (a - c).length() < (b - c).length() {
            b
        } else {
            a
        }
    };
    let (u, v) = (
        (far(sk, lines[0]) - c).normalize(),
        (far(sk, lines[1]) - c).normalize(),
    );
    let theta = u.angle_to(v).abs();
    if theta < 1e-6 || (PI - theta) < 1e-6 {
        return Err(SketchError::Invalid("the lines are parallel".into()));
    }
    let t = radius / (theta / 2.0).tan();
    for l in [lines[0], lines[1]] {
        if t >= (far(sk, l) - c).length() {
            return Err(SketchError::Invalid(
                "the fillet radius is too large".into(),
            ));
        }
    }
    let bis = (u + v).normalize();
    let center = c + bis * (radius / (theta / 2.0).sin());
    let t0 = sk.add_point(c + u * t);
    let t1 = sk.add_point(c + v * t);
    for (l, p) in [(lines[0], t0), (lines[1], t1)] {
        if let Geometry::Line { a, b } = &mut sk.entities.get_mut(&l).unwrap().geo {
            if *a == corner {
                *a = p;
            } else {
                *b = p;
            }
        }
    }
    let cid = sk.add_point(center);
    // Arcs run counter-clockwise; pick the order that gives the short way round.
    let (_, sweep) = arc_angles(center, sk.point(t0)?, sk.point(t1)?);
    let arc = if sweep < PI {
        sk.add_arc(cid, t0, t1)?
    } else {
        sk.add_arc(cid, t1, t0)?
    };
    let mut out = Created {
        entities: vec![arc],
        constraints: vec![],
    };
    if !sk
        .entities
        .values()
        .any(|e| e.point_refs().contains(&corner))
    {
        sk.delete(&[corner]);
    }
    out.c(
        sk,
        K::Tangent {
            a: lines[0],
            b: arc,
            inside: false,
        },
    );
    out.c(
        sk,
        K::Tangent {
            a: lines[1],
            b: arc,
            inside: false,
        },
    );
    out.c(
        sk,
        K::Radius {
            curve: arc,
            value: radius,
        },
    );
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SolveStatus;

    #[test]
    fn rectangle_slot_polygon_solve() {
        let mut sk = Sketch::new();
        corner_rectangle(&mut sk, DVec2::new(10.0, 10.0), DVec2::ZERO);
        center_rectangle(&mut sk, DVec2::new(50.0, 0.0), DVec2::new(55.0, 3.0));
        straight_slot(&mut sk, DVec2::new(0.0, 40.0), DVec2::new(20.0, 40.0), 6.0).unwrap();
        polygon(&mut sk, DVec2::new(-40.0, 0.0), DVec2::new(-30.0, 0.0), 6).unwrap();
        let r = sk.solve();
        assert_eq!(r.status, SolveStatus::Solved, "{r:?}");
        assert!(r.redundant.is_empty(), "{r:?}");
        let c = sk.contours().unwrap();
        assert_eq!(c.regions.len(), 4);
    }

    #[test]
    fn three_point_arc_passes_through_via() {
        let mut sk = Sketch::new();
        let a = sk.add_point(DVec2::new(-1.0, 0.0));
        let b = sk.add_point(DVec2::new(1.0, 0.0));
        let arc = three_point_arc(&mut sk, a, b, DVec2::new(0.0, -1.0)).unwrap();
        let curve = sk.curve(arc).unwrap();
        assert!((curve.point_at(0.5) - DVec2::new(0.0, -1.0)).length() < 1e-9);
    }

    #[test]
    fn trim_line_between_crossings() {
        let mut sk = Sketch::new();
        let (h, _, _) = sk.add_line_at(DVec2::new(0.0, 0.0), DVec2::new(30.0, 0.0));
        sk.add_line_at(DVec2::new(10.0, -5.0), DVec2::new(10.0, 5.0));
        sk.add_line_at(DVec2::new(20.0, -5.0), DVec2::new(20.0, 5.0));
        let out = trim(&mut sk, h, DVec2::new(15.0, 0.0)).unwrap();
        assert_eq!(out.entities.len(), 1);
        let (a, b) = sk.line(h).unwrap();
        assert!((b - DVec2::new(10.0, 0.0)).length() < 1e-9 && a == DVec2::ZERO);
        let (c, d) = sk.line(out.entities[0]).unwrap();
        assert!(
            (c - DVec2::new(20.0, 0.0)).length() < 1e-9
                && (d - DVec2::new(30.0, 0.0)).length() < 1e-9
        );
        assert_eq!(sk.solve().status, SolveStatus::Solved);
    }

    #[test]
    fn trim_circle_to_arc() {
        let mut sk = Sketch::new();
        let c = sk.add_circle_at(DVec2::ZERO, 5.0);
        sk.add_line_at(DVec2::new(-10.0, 0.0), DVec2::new(10.0, 0.0));
        let out = trim(&mut sk, c, DVec2::new(0.0, -5.0)).unwrap();
        let arc = sk.curve(out.entities[0]).unwrap();
        // The lower half went; the upper half remains.
        assert!(
            (arc.point_at(0.5) - DVec2::new(0.0, 5.0)).length() < 1e-9,
            "{arc:?}"
        );
    }

    #[test]
    fn extend_line_to_circle() {
        let mut sk = Sketch::new();
        let (l, _, _) = sk.add_line_at(DVec2::ZERO, DVec2::new(1.0, 0.0));
        sk.add_circle_at(DVec2::new(10.0, 0.0), 2.0);
        extend(&mut sk, l, DVec2::new(1.0, 0.0)).unwrap();
        let (_, b) = sk.line(l).unwrap();
        assert!((b - DVec2::new(8.0, 0.0)).length() < 1e-9);
    }

    #[test]
    fn offset_closed_rectangle_inward() {
        let mut sk = Sketch::new();
        let r = corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(10.0, 10.0));
        let out = offset(&mut sk, &r.entities[4..8], 1.0).unwrap();
        assert_eq!(out.entities.len(), 4);
        let c = sk.contours().unwrap();
        assert_eq!(c.loops.len(), 2);
        let inner: f64 = out
            .entities
            .iter()
            .map(|e| sk.curve(*e).unwrap().length())
            .sum();
        assert!((inner - 32.0).abs() < 1e-9, "{inner}");
    }

    #[test]
    fn mirror_and_fillet() {
        let mut sk = Sketch::new();
        let (axis, _, _) = sk.add_line_at(DVec2::new(0.0, -10.0), DVec2::new(0.0, 10.0));
        sk.entities.get_mut(&axis).unwrap().construction = true;
        let (l, _, _) = sk.add_line_at(DVec2::new(2.0, 0.0), DVec2::new(5.0, 3.0));
        let out = mirror(&mut sk, &[l], axis).unwrap();
        let (a, b) = sk.line(out.entities[0]).unwrap();
        assert!(
            (a - DVec2::new(-2.0, 0.0)).length() < 1e-9
                && (b - DVec2::new(-5.0, 3.0)).length() < 1e-9
        );
        assert_eq!(sk.solve().status, SolveStatus::Solved);

        let mut sk = Sketch::new();
        let r = corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(10.0, 10.0));
        let out = sketch_fillet(&mut sk, r.entities[2], 2.0).unwrap();
        let arc = sk.curve(out.entities[0]).unwrap();
        let mid = arc.point_at(0.5);
        let expected = DVec2::new(8.0, 8.0) + DVec2::splat(std::f64::consts::FRAC_1_SQRT_2 * 2.0);
        assert!((mid - expected).length() < 1e-9, "{mid}");
        assert_eq!(sk.solve().status, SolveStatus::Solved);
        assert_eq!(sk.contours().unwrap().regions.len(), 1);
    }
}

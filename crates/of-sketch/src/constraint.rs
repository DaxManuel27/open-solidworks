//! Relations and dimensions, and the equations they impose.

use glam::DVec2;
use serde::{Deserialize, Serialize};

use crate::{Geometry, Id, Result, Sketch, SketchError};

/// A relation or dimension between sketch entities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Constraint {
    #[serde(flatten)]
    pub kind: ConstraintKind,
    /// Driven (reference) dimensions show a value but don't constrain.
    #[serde(default = "yes")]
    pub driving: bool,
}

fn yes() -> bool {
    true
}

/// Lengths are in millimetres and angles in radians. Where a dimension
/// takes a point, a circle or arc may be given and its center is used.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConstraintKind {
    /// Two points at the same place.
    Coincident {
        a: Id,
        b: Id,
    },
    /// A point lying on a line (extended), circle or arc.
    PointOnCurve {
        point: Id,
        curve: Id,
    },
    Midpoint {
        point: Id,
        line: Id,
    },
    Horizontal {
        line: Id,
    },
    Vertical {
        line: Id,
    },
    HorizontalPoints {
        a: Id,
        b: Id,
    },
    VerticalPoints {
        a: Id,
        b: Id,
    },
    Parallel {
        a: Id,
        b: Id,
    },
    Perpendicular {
        a: Id,
        b: Id,
    },
    Collinear {
        a: Id,
        b: Id,
    },
    /// Equal length (lines) or equal radius (circles and arcs).
    Equal {
        a: Id,
        b: Id,
    },
    /// Line–circle or circle–circle tangency. `inside` is for a circle
    /// tangent inside another; it's worked out when the relation is added.
    Tangent {
        a: Id,
        b: Id,
        #[serde(default)]
        inside: bool,
    },
    Concentric {
        a: Id,
        b: Id,
    },
    /// Points `a` and `b` mirrored about line `about`.
    Symmetric {
        a: Id,
        b: Id,
        about: Id,
    },
    /// Hold an entity where it is. `at` is filled in when added.
    Fix {
        entity: Id,
        #[serde(default)]
        at: Vec<f64>,
    },
    /// Point–point, point–line, or line–line (parallel) distance.
    Distance {
        a: Id,
        b: Id,
        value: f64,
    },
    /// Signed horizontal distance, `b.x - a.x`.
    HorizontalDistance {
        a: Id,
        b: Id,
        value: f64,
    },
    /// Signed vertical distance, `b.y - a.y`.
    VerticalDistance {
        a: Id,
        b: Id,
        value: f64,
    },
    Length {
        line: Id,
        value: f64,
    },
    Radius {
        curve: Id,
        value: f64,
    },
    Diameter {
        curve: Id,
        value: f64,
    },
    /// Signed angle from line `a`'s direction to line `b`'s.
    Angle {
        a: Id,
        b: Id,
        value: f64,
    },
}

/// Read access to geometry, either from a sketch or from solver parameters.
pub(crate) trait Geo {
    fn pt(&self, id: Id) -> DVec2;
    fn radius(&self, id: Id) -> f64;
    fn sketch(&self) -> &Sketch;

    fn line(&self, id: Id) -> (DVec2, DVec2) {
        match self.sketch().entities[&id].geo {
            Geometry::Line { a, b } => (self.pt(a), self.pt(b)),
            _ => (DVec2::ZERO, DVec2::X),
        }
    }

    /// A point's position, or a circle's or arc's center.
    fn locus(&self, id: Id) -> DVec2 {
        match self.sketch().entities[&id].geo {
            Geometry::Circle { center, .. } | Geometry::Arc { center, .. } => self.pt(center),
            _ => self.pt(id),
        }
    }
}

fn cross(a: DVec2, b: DVec2) -> f64 {
    a.perp_dot(b)
}

fn wrap_angle(a: f64) -> f64 {
    let t = std::f64::consts::TAU;
    let mut a = a % t;
    if a > std::f64::consts::PI {
        a -= t;
    } else if a <= -std::f64::consts::PI {
        a += t;
    }
    a
}

/// Signed distance from `p` to the line through `a`, `b`.
fn point_line(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let d = b - a;
    let len = d.length().max(1e-12);
    cross(d, p - a) / len
}

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Point,
    Line,
    Round,
    Spline,
}

fn shape_of(sk: &Sketch, id: Id) -> Result<Shape> {
    Ok(match sk.entity(id)?.geo {
        Geometry::Point { .. } => Shape::Point,
        Geometry::Line { .. } => Shape::Line,
        Geometry::Circle { .. } | Geometry::Arc { .. } => Shape::Round,
        Geometry::Spline { .. } => Shape::Spline,
    })
}

impl ConstraintKind {
    /// Entities this constraint refers to.
    pub fn refs(&self) -> Vec<Id> {
        use ConstraintKind::*;
        match self {
            Coincident { a, b }
            | HorizontalPoints { a, b }
            | VerticalPoints { a, b }
            | Parallel { a, b }
            | Perpendicular { a, b }
            | Collinear { a, b }
            | Equal { a, b }
            | Tangent { a, b, .. }
            | Concentric { a, b }
            | Distance { a, b, .. }
            | HorizontalDistance { a, b, .. }
            | VerticalDistance { a, b, .. }
            | Angle { a, b, .. } => vec![*a, *b],
            PointOnCurve { point, curve } => vec![*point, *curve],
            Midpoint { point, line } => vec![*point, *line],
            Horizontal { line } | Vertical { line } | Length { line, .. } => vec![*line],
            Symmetric { a, b, about } => vec![*a, *b, *about],
            Fix { entity, .. } => vec![*entity],
            Radius { curve, .. } | Diameter { curve, .. } => vec![*curve],
        }
    }

    /// The dimension value, if this is a dimension.
    pub fn value(&self) -> Option<f64> {
        use ConstraintKind::*;
        match self {
            Distance { value, .. }
            | HorizontalDistance { value, .. }
            | VerticalDistance { value, .. }
            | Length { value, .. }
            | Radius { value, .. }
            | Diameter { value, .. }
            | Angle { value, .. } => Some(*value),
            _ => None,
        }
    }

    fn value_mut(&mut self) -> Option<&mut f64> {
        use ConstraintKind::*;
        match self {
            Distance { value, .. }
            | HorizontalDistance { value, .. }
            | VerticalDistance { value, .. }
            | Length { value, .. }
            | Radius { value, .. }
            | Diameter { value, .. }
            | Angle { value, .. } => Some(value),
            _ => None,
        }
    }

    /// Set a dimension's value. Signed dimensions keep their current sign,
    /// so typing a positive number never flips geometry over.
    pub fn set_value(&mut self, v: f64) -> std::result::Result<(), String> {
        let signed = matches!(
            self,
            Self::HorizontalDistance { .. } | Self::VerticalDistance { .. } | Self::Angle { .. }
        );
        let unsigned_nonzero = matches!(self, Self::Radius { .. } | Self::Diameter { .. });
        let Some(slot) = self.value_mut() else {
            return Err("this relation has no value".into());
        };
        if !v.is_finite() || (!signed && v < 0.0) || (unsigned_nonzero && v == 0.0) {
            return Err(format!("{v} is not a valid value for this dimension"));
        }
        *slot = if signed && *slot < 0.0 { -v.abs() } else { v };
        Ok(())
    }

    /// Number of equations this constraint contributes.
    pub(crate) fn equation_count(&self, sk: &Sketch) -> usize {
        use ConstraintKind::*;
        match self {
            Coincident { .. } | Midpoint { .. } | Concentric { .. } | Symmetric { .. } => 2,
            Collinear { .. } => 2,
            Fix { at, .. } => at.len(),
            PointOnCurve { curve, .. } => match sk.entities[curve].geo {
                Geometry::Spline { .. } => 0,
                _ => 1,
            },
            _ => 1,
        }
    }

    /// Append this constraint's residuals (zero when satisfied).
    pub(crate) fn residuals(&self, g: &impl Geo, out: &mut Vec<f64>) {
        use ConstraintKind::*;
        let sk = g.sketch();
        match self {
            Coincident { a, b } => {
                let d = g.pt(*a) - g.pt(*b);
                out.extend([d.x, d.y]);
            }
            PointOnCurve { point, curve } => {
                let p = g.pt(*point);
                match sk.entities[curve].geo {
                    Geometry::Line { .. } => {
                        let (a, b) = g.line(*curve);
                        out.push(point_line(p, a, b));
                    }
                    Geometry::Circle { .. } | Geometry::Arc { .. } => {
                        out.push((p - g.locus(*curve)).length() - g.radius(*curve));
                    }
                    _ => {}
                }
            }
            Midpoint { point, line } => {
                let (a, b) = g.line(*line);
                let d = g.pt(*point) - (a + b) * 0.5;
                out.extend([d.x, d.y]);
            }
            Horizontal { line } => {
                let (a, b) = g.line(*line);
                out.push(b.y - a.y);
            }
            Vertical { line } => {
                let (a, b) = g.line(*line);
                out.push(b.x - a.x);
            }
            HorizontalPoints { a, b } => out.push(g.locus(*b).y - g.locus(*a).y),
            VerticalPoints { a, b } => out.push(g.locus(*b).x - g.locus(*a).x),
            Parallel { a, b } => {
                let (d1, d2) = (dir(g.line(*a)), dir(g.line(*b)));
                out.push(cross(d1, d2));
            }
            Perpendicular { a, b } => {
                let (d1, d2) = (dir(g.line(*a)), dir(g.line(*b)));
                out.push(d1.dot(d2));
            }
            Collinear { a, b } => {
                let (p, q) = g.line(*a);
                let (r, s) = g.line(*b);
                out.extend([point_line(r, p, q), point_line(s, p, q)]);
            }
            Equal { a, b } => match shape_of(sk, *a) {
                Ok(Shape::Line) => {
                    let (p, q) = g.line(*a);
                    let (r, s) = g.line(*b);
                    out.push((q - p).length() - (s - r).length());
                }
                _ => out.push(g.radius(*a) - g.radius(*b)),
            },
            Tangent { a, b, inside } => {
                let (sa, sb) = (shape_of(sk, *a).ok(), shape_of(sk, *b).ok());
                // At a shared endpoint the distance form has no gradient, so
                // require the curves' directions there to match instead.
                if let Some((pa, pb)) = shared_end(sk, *a, *b) {
                    let p = g.pt(pa);
                    let q = g.pt(pb);
                    let normal_of = |id: Id, at: DVec2| -> DVec2 {
                        if shape_of(sk, id).ok() == Some(Shape::Line) {
                            dir(g.line(id)).perp()
                        } else {
                            (at - g.locus(id)).normalize_or(DVec2::X)
                        }
                    };
                    out.push(cross(normal_of(*a, p), normal_of(*b, q)));
                    return;
                }
                match (sa, sb) {
                    (Some(Shape::Line), _) | (_, Some(Shape::Line)) => {
                        let (line, round) = if sa == Some(Shape::Line) {
                            (*a, *b)
                        } else {
                            (*b, *a)
                        };
                        let (p, q) = g.line(line);
                        out.push(point_line(g.locus(round), p, q).abs() - g.radius(round));
                    }
                    _ => {
                        let d = (g.locus(*a) - g.locus(*b)).length();
                        let (ra, rb) = (g.radius(*a), g.radius(*b));
                        out.push(if *inside {
                            d - (ra - rb).abs()
                        } else {
                            d - (ra + rb)
                        });
                    }
                }
            }
            Concentric { a, b } => {
                let d = g.locus(*a) - g.locus(*b);
                out.extend([d.x, d.y]);
            }
            Symmetric { a, b, about } => {
                let (p, q) = g.line(*about);
                let (pa, pb) = (g.pt(*a), g.pt(*b));
                let m = (pa + pb) * 0.5;
                out.push(point_line(m, p, q));
                out.push(dir((p, q)).dot(pb - pa));
            }
            Fix { entity, at } => {
                let now = fix_values(g, *entity);
                out.extend(now.iter().zip(at).map(|(n, a)| n - a));
            }
            Distance { a, b, value } => out.push(distance(g, *a, *b) - value),
            HorizontalDistance { a, b, value } => out.push(g.locus(*b).x - g.locus(*a).x - value),
            VerticalDistance { a, b, value } => out.push(g.locus(*b).y - g.locus(*a).y - value),
            Length { line, value } => {
                let (p, q) = g.line(*line);
                out.push((q - p).length() - value);
            }
            Radius { curve, value } => out.push(g.radius(*curve) - value),
            Diameter { curve, value } => out.push(2.0 * g.radius(*curve) - value),
            Angle { a, b, value } => {
                let (d1, d2) = (dir(g.line(*a)), dir(g.line(*b)));
                out.push(wrap_angle(cross(d1, d2).atan2(d1.dot(d2)) - value));
            }
        }
    }
}

/// Endpoints of two curves that are the same point or coincident.
fn shared_end(sk: &Sketch, a: Id, b: Id) -> Option<(Id, Id)> {
    let ends = |id: Id| -> Vec<Id> {
        match sk.entities.get(&id).map(|e| &e.geo) {
            Some(Geometry::Line { a, b }) => vec![*a, *b],
            Some(Geometry::Arc { start, end, .. }) => vec![*start, *end],
            _ => vec![],
        }
    };
    let coincident = |p: Id, q: Id| {
        p == q
            || sk.constraints.values().any(|c| {
                matches!(c.kind, ConstraintKind::Coincident { a, b } if (a == p && b == q) || (a == q && b == p))
            })
    };
    for p in ends(a) {
        for q in ends(b) {
            if coincident(p, q) {
                return Some((p, q));
            }
        }
    }
    None
}

fn dir((a, b): (DVec2, DVec2)) -> DVec2 {
    (b - a).normalize_or(DVec2::X)
}

fn distance(g: &impl Geo, a: Id, b: Id) -> f64 {
    let sk = g.sketch();
    match (shape_of(sk, a).ok(), shape_of(sk, b).ok()) {
        (Some(Shape::Line), Some(Shape::Line)) => {
            let (p, q) = g.line(a);
            point_line(g.line(b).0, p, q).abs()
        }
        (Some(Shape::Line), _) => {
            let (p, q) = g.line(a);
            point_line(g.locus(b), p, q).abs()
        }
        (_, Some(Shape::Line)) => {
            let (p, q) = g.line(b);
            point_line(g.locus(a), p, q).abs()
        }
        _ => (g.locus(a) - g.locus(b)).length(),
    }
}

fn fix_values(g: &impl Geo, id: Id) -> Vec<f64> {
    let e = &g.sketch().entities[&id];
    let mut v = Vec::new();
    match &e.geo {
        Geometry::Point { .. } => {
            let p = g.pt(id);
            v.extend([p.x, p.y]);
        }
        Geometry::Circle { center, .. } => {
            let c = g.pt(*center);
            v.extend([c.x, c.y, g.radius(id)]);
        }
        _ => {
            for p in e.point_refs() {
                let q = g.pt(p);
                v.extend([q.x, q.y]);
            }
        }
    }
    v
}

/// Geometry read straight from the sketch.
struct SketchGeo<'a>(&'a Sketch);

impl Geo for SketchGeo<'_> {
    fn pt(&self, id: Id) -> DVec2 {
        self.0.point(id).unwrap_or_default()
    }
    fn radius(&self, id: Id) -> f64 {
        self.0.circle(id).map(|c| c.1).unwrap_or(0.0)
    }
    fn sketch(&self) -> &Sketch {
        self.0
    }
}

impl Constraint {
    pub fn new(kind: ConstraintKind) -> Constraint {
        Constraint {
            kind,
            driving: true,
        }
    }

    /// Check the referenced entities exist and have the right kinds.
    pub(crate) fn validate(&self, sk: &Sketch) -> Result<()> {
        use ConstraintKind::*;
        let refs = self.kind.refs();
        let mut shapes = Vec::new();
        for r in &refs {
            shapes.push(shape_of(sk, *r)?);
        }
        if refs.len() >= 2 && refs[0] == refs[1] {
            return Err(SketchError::Invalid(
                "a relation needs two different entities".into(),
            ));
        }
        let want = |i: usize, ok: &[Shape], what: &'static str| -> Result<()> {
            if ok.contains(&shapes[i]) {
                Ok(())
            } else {
                Err(SketchError::WrongKind(refs[i], what))
            }
        };
        let (pt, ln, rd) = (Shape::Point, Shape::Line, Shape::Round);
        match &self.kind {
            Coincident { .. } => {
                want(0, &[pt], "point")?;
                want(1, &[pt], "point")
            }
            PointOnCurve { .. } => {
                want(0, &[pt], "point")?;
                want(1, &[ln, rd], "line, circle or arc")
            }
            Midpoint { .. } | Symmetric { .. } => {
                want(0, &[pt], "point")?;
                want(
                    1,
                    &[if matches!(self.kind, Midpoint { .. }) {
                        ln
                    } else {
                        pt
                    }],
                    "line or point",
                )?;
                if refs.len() == 3 {
                    want(2, &[ln], "line")
                } else {
                    Ok(())
                }
            }
            Horizontal { .. } | Vertical { .. } | Length { .. } => want(0, &[ln], "line"),
            Parallel { .. } | Perpendicular { .. } | Collinear { .. } | Angle { .. } => {
                want(0, &[ln], "line")?;
                want(1, &[ln], "line")
            }
            Equal { .. } => {
                want(0, &[ln, rd], "line, circle or arc")?;
                want(1, &[shapes[0]], "entity of the same kind")
            }
            Tangent { .. } => {
                want(0, &[ln, rd], "line, circle or arc")?;
                want(1, &[ln, rd], "line, circle or arc")?;
                if shapes[0] == ln && shapes[1] == ln {
                    return Err(SketchError::Invalid("two lines can't be tangent".into()));
                }
                Ok(())
            }
            Concentric { .. } | Radius { .. } | Diameter { .. } => {
                want(0, &[rd], "circle or arc")?;
                if refs.len() == 2 {
                    want(1, &[rd], "circle or arc")
                } else {
                    Ok(())
                }
            }
            HorizontalPoints { .. }
            | VerticalPoints { .. }
            | HorizontalDistance { .. }
            | VerticalDistance { .. } => {
                want(0, &[pt, rd], "point")?;
                want(1, &[pt, rd], "point")
            }
            Distance { .. } => {
                want(0, &[pt, ln, rd], "point or line")?;
                want(1, &[pt, ln, rd], "point or line")
            }
            Fix { .. } => Ok(()),
        }
    }

    /// Fill in values that come from the current geometry: fix positions,
    /// inside/outside tangency, and the sign of signed dimensions.
    pub(crate) fn prepare(&mut self, sk: &Sketch) {
        let g = SketchGeo(sk);
        let measured = self.measured(sk);
        match &mut self.kind {
            ConstraintKind::Fix { entity, at } => *at = fix_values(&g, *entity),
            ConstraintKind::Tangent { a, b, inside } => {
                if let (Ok(Shape::Round), Ok(Shape::Round)) = (shape_of(sk, *a), shape_of(sk, *b)) {
                    let d = (g.locus(*a) - g.locus(*b)).length();
                    let (ra, rb) = (g.radius(*a), g.radius(*b));
                    *inside = (d - (ra - rb).abs()).abs() < (d - (ra + rb)).abs();
                }
            }
            ConstraintKind::HorizontalDistance { value, .. }
            | ConstraintKind::VerticalDistance { value, .. }
            | ConstraintKind::Angle { value, .. } => {
                if let Some(m) = measured
                    && m < 0.0
                {
                    *value = -value.abs();
                }
            }
            _ => {}
        }
    }

    /// The value this dimension has in the current geometry.
    pub fn measured(&self, sk: &Sketch) -> Option<f64> {
        let mut kind = self.kind.clone();
        let slot = kind.value_mut()?;
        *slot = 0.0;
        let mut out = Vec::new();
        kind.residuals(&SketchGeo(sk), &mut out);
        out.first().copied()
    }

    /// Set this dimension's value to what the geometry currently measures.
    pub(crate) fn measure_from(&mut self, sk: &Sketch) {
        self.prepare(sk);
        if let Some(m) = self.measured(sk)
            && let Some(slot) = self.kind.value_mut()
        {
            *slot = m;
        }
    }

    /// How far the current geometry is from satisfying this constraint.
    pub fn error(&self, sk: &Sketch) -> f64 {
        let mut out = Vec::new();
        self.kind.residuals(&SketchGeo(sk), &mut out);
        out.iter().map(|r| r * r).sum::<f64>().sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angle_wraps() {
        assert!((wrap_angle(3.0 * std::f64::consts::PI) - std::f64::consts::PI).abs() < 1e-12);
        assert!((wrap_angle(-0.5) + 0.5).abs() < 1e-12);
    }

    #[test]
    fn signed_values_keep_sign() {
        let mut k = ConstraintKind::Angle {
            a: 1,
            b: 2,
            value: -0.3,
        };
        k.set_value(0.5).unwrap();
        assert_eq!(k.value(), Some(-0.5));
        let mut r = ConstraintKind::Radius {
            curve: 1,
            value: 2.0,
        };
        assert!(r.set_value(-1.0).is_err());
        assert!(r.set_value(0.0).is_err());
    }

    #[test]
    fn serializes_flat() {
        let c = Constraint::new(ConstraintKind::Distance {
            a: 1,
            b: 2,
            value: 5.0,
        });
        let s = serde_json::to_string(&c).unwrap();
        assert_eq!(
            s,
            r#"{"type":"distance","a":1,"b":2,"value":5.0,"driving":true}"#
        );
    }
}

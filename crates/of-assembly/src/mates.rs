//! Mates as solver terms. Values that depend on where things are when a mate
//! is made (closest alignment, the side a face is on, reference vectors,
//! coupler phases) are captured once and stored with the mate.

use std::f64::consts::{PI, TAU};
use std::rc::Rc;

use of_mate::{Body, Curve, DVec3, Dir, Line, Measure, Motion, Plane, Pose, Pt, Target, Term};

use crate::cache::Part;
use crate::document::{
    Alignment, Captured, CompRef, Limit, MateKind, TravelConstraint, WidthConstraint,
};
use crate::geometry::{self, Geo};

/// A reference resolved for solving.
#[derive(Clone)]
pub struct Located {
    /// Solver body the geometry moves with; `None` is the assembly frame.
    pub body: Body,
    /// Geometry in the body's frame.
    pub geo: Geo,
    /// The body's current world pose.
    pub pose: Pose,
    /// The part the entity belongs to and its frame inside the body.
    pub part: Option<Rc<Part>>,
    pub part_to_body: Pose,
}

impl Located {
    pub fn world(&self) -> Geo {
        self.geo.transformed(&self.pose)
    }
}

/// What mates need from the assembly.
pub trait Context {
    fn locate(&mut self, r: &CompRef) -> Result<Located, String>;
    /// The solver body and world pose of a component path.
    fn body(&mut self, path: &[String]) -> Result<(Body, Pose), String>;
}

fn unsupported(mate: &str, a: &Geo, b: &Geo) -> String {
    format!("a {mate} mate can't use a {} and a {}", a.name(), b.name())
}

/// Captured values, filled in the first time and reused afterwards.
struct Capture {
    c: Captured,
    fresh: bool,
}

impl Capture {
    fn sign(&mut self, alignment: Alignment, world: impl FnOnce() -> f64) -> f64 {
        match alignment {
            Alignment::Aligned => 1.0,
            Alignment::AntiAligned => -1.0,
            Alignment::Closest => {
                if self.fresh || self.c.sign.is_none() {
                    let d = world();
                    self.c.sign = Some(if d >= 0.0 { 1.0 } else { -1.0 });
                }
                self.c.sign.unwrap()
            }
        }
    }

    fn side(&mut self, world: impl FnOnce() -> f64) -> f64 {
        if self.fresh || self.c.side.is_none() {
            self.c.side = Some(if world() >= 0.0 { 1.0 } else { -1.0 });
        }
        self.c.side.unwrap()
    }

    fn vectors(&mut self, n: usize, world: impl FnOnce() -> Vec<DVec3>) -> Vec<DVec3> {
        if self.fresh || self.c.vectors.len() != n {
            self.c.vectors = world().iter().map(|v| v.to_array()).collect();
        }
        self.c.vectors.iter().map(|v| DVec3::from(*v)).collect()
    }
}

// Solver primitives from located geometry, in the body frame.
fn pt(l: &Located, p: DVec3) -> Pt {
    Pt::new(l.body, p)
}

fn dir(l: &Located, d: DVec3) -> Dir {
    Dir::new(l.body, d)
}

fn plane_of(l: &Located) -> Option<Plane> {
    match l.geo {
        Geo::Plane { origin, normal, .. } => Some(Plane::new(l.body, origin, normal)),
        _ => None,
    }
}

fn line_of(l: &Located) -> Option<Line> {
    l.geo.axis().map(|(o, d)| Line::new(l.body, o, d))
}

/// A center-like point: points, sphere and circle centers.
fn center_of(l: &Located) -> Option<Pt> {
    match l.geo {
        Geo::Point(p) | Geo::Sphere { center: p, .. } | Geo::Circle { center: p, .. } => {
            Some(pt(l, p))
        }
        _ => None,
    }
}

fn world_dir(l: &Located) -> DVec3 {
    l.world().direction().unwrap_or(DVec3::Z)
}

fn eq(measure: Measure, value: f64) -> Term {
    Term::Value {
        measure,
        target: Target::Equal(value),
    }
}

fn target(value: f64, limit: Option<Limit>, scale: f64) -> Target {
    match limit {
        Some(l) => {
            let (a, b) = (l.min * scale, l.max * scale);
            Target::Range {
                min: a.min(b),
                max: a.max(b),
            }
        }
        None => Target::Equal(value * scale),
    }
}

/// A unit vector perpendicular to `d`.
fn perpendicular(d: DVec3) -> DVec3 {
    let d = d.normalize_or_zero();
    let seed = if d.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    (seed - d * seed.dot(d)).normalize_or_zero()
}

/// Terms for a mate, and the captured values they use. `captured` is
/// `None` when the mate is new: values are taken from current positions.
pub fn build(
    kind: &MateKind,
    captured: Option<&Captured>,
    ctx: &mut dyn Context,
) -> Result<(Vec<Term>, Captured), String> {
    let mut cap = Capture {
        c: captured.cloned().unwrap_or_default(),
        fresh: captured.is_none(),
    };
    let terms = match kind {
        MateKind::Coincident { a, b, alignment } => {
            let (a, b) = (ctx.locate(a)?, ctx.locate(b)?);
            coincident(&a, &b, *alignment, &mut cap)?
        }
        MateKind::Parallel { a, b, alignment } => {
            let (a, b) = (ctx.locate(a)?, ctx.locate(b)?);
            parallel(&a, &b, *alignment, &mut cap)?
        }
        MateKind::Perpendicular { a, b } => {
            let (a, b) = (ctx.locate(a)?, ctx.locate(b)?);
            perpendicular_mate(&a, &b)?
        }
        MateKind::Tangent { a, b, alignment } => {
            let (a, b) = (ctx.locate(a)?, ctx.locate(b)?);
            tangent(&a, &b, *alignment, &mut cap)?
        }
        MateKind::Concentric {
            a,
            b,
            alignment,
            lock_rotation,
        } => {
            let (a, b) = (ctx.locate(a)?, ctx.locate(b)?);
            concentric(&a, &b, *alignment, *lock_rotation, &mut cap)?
        }
        MateKind::Lock { a, b } => {
            let (ba, pa) = ctx.body(a)?;
            let (bb, pb) = ctx.body(b)?;
            if ba == bb {
                return Err("lock two different components".into());
            }
            if cap.fresh || cap.c.relative.is_none() {
                cap.c.relative = Some(pa.inverse().then(&pb).into());
            }
            vec![Term::Lock {
                a: ba,
                b: bb,
                relative: cap.c.relative.unwrap().into(),
            }]
        }
        MateKind::Distance {
            a,
            b,
            distance,
            flip,
            limit,
        } => {
            if *distance < 0.0 {
                return Err("the distance can't be negative".into());
            }
            let (a, b) = (ctx.locate(a)?, ctx.locate(b)?);
            distance_mate(&a, &b, *distance, *flip, *limit, &mut cap)?
        }
        MateKind::Angle {
            a,
            b,
            angle,
            flip,
            limit,
        } => {
            let (a, b) = (ctx.locate(a)?, ctx.locate(b)?);
            let (da, db) = (
                a.geo
                    .direction()
                    .ok_or("an angle mate needs planes, edges or axes")?,
                b.geo
                    .direction()
                    .ok_or("an angle mate needs planes, edges or axes")?,
            );
            if !(0.0..=PI).contains(angle) {
                return Err("the angle must be between 0 and 180 degrees".into());
            }
            let value = if *flip { PI - angle } else { *angle };
            let t = match limit {
                Some(l) => target(0.0, Some(*l), 1.0),
                None => Target::Equal(value),
            };
            vec![Term::Value {
                measure: Measure::Angle {
                    a: dir(&a, da),
                    b: dir(&b, db),
                },
                target: t,
            }]
        }
        MateKind::ProfileCenter {
            a,
            b,
            offset,
            flip,
            lock_rotation,
        } => {
            let (a, b) = (ctx.locate(a)?, ctx.locate(b)?);
            profile_center(&a, &b, *offset, *flip, *lock_rotation, &mut cap)?
        }
        MateKind::Symmetric { a, b, plane } => {
            let (a, b, s) = (ctx.locate(a)?, ctx.locate(b)?, ctx.locate(plane)?);
            symmetric(&a, &b, &s, &mut cap)?
        }
        MateKind::Width {
            width,
            tab,
            constraint,
        } => {
            let w = [ctx.locate(&width[0])?, ctx.locate(&width[1])?];
            let t = [ctx.locate(&tab[0])?, ctx.locate(&tab[1])?];
            width_mate(&w, &t, *constraint, &mut cap)?
        }
        MateKind::Path {
            point,
            path,
            constraint,
            follow,
            roll,
        } => {
            let p = ctx.locate(point)?;
            let pieces = path
                .iter()
                .map(|r| ctx.locate(r))
                .collect::<Result<Vec<_>, _>>()?;
            let follow = follow.as_ref().map(|r| ctx.locate(r)).transpose()?;
            let roll = match roll {
                Some(r) => Some((ctx.locate(&r.axis)?, ctx.locate(&r.up)?)),
                None => None,
            };
            path_mate(&p, &pieces, *constraint, follow.as_ref(), roll.as_ref())?
        }
        MateKind::LinearCoupler {
            a,
            b,
            ratio_a,
            ratio_b,
            reverse,
        } => {
            if *ratio_a == 0.0 || !ratio_a.is_finite() || !ratio_b.is_finite() {
                return Err("enter nonzero ratio values".into());
            }
            let (a, b) = (ctx.locate(a)?, ctx.locate(b)?);
            let k = if *reverse { -1.0 } else { 1.0 };
            let ta = translation(&a, &mut cap, 0)?;
            let tb = translation(&b, &mut cap, 2)?;
            coupler(vec![(ta, *ratio_b), (tb, -k * ratio_a)], &mut cap)
        }
        MateKind::Cam { cam, follower } => {
            let (c, f) = (ctx.locate(cam)?, ctx.locate(follower)?);
            cam_mate(&c, &f, cam)?
        }
        MateKind::Slot {
            slot,
            pin,
            constraint,
        } => {
            let (s, p) = (ctx.locate(slot)?, ctx.locate(pin)?);
            slot_mate(&s, &p, slot, *constraint)?
        }
        MateKind::Hinge { axis, faces, limit } => {
            let (a, b) = (ctx.locate(&axis[0])?, ctx.locate(&axis[1])?);
            let mut terms = concentric(&a, &b, Alignment::Closest, false, &mut cap)?;
            let (fa, fb) = (ctx.locate(&faces[0])?, ctx.locate(&faces[1])?);
            let mut faces_cap = Capture {
                c: Captured {
                    sign: cap.c.side,
                    ..Default::default()
                },
                fresh: cap.fresh,
            };
            terms.extend(coincident(&fa, &fb, Alignment::Closest, &mut faces_cap)?);
            cap.c.side = faces_cap.c.sign;
            if let Some(l) = limit {
                let (la, lb) = (ctx.locate(&l.faces[0])?, ctx.locate(&l.faces[1])?);
                let (da, db) = (
                    la.geo.direction().ok_or("hinge limits need planar faces")?,
                    lb.geo.direction().ok_or("hinge limits need planar faces")?,
                );
                terms.push(Term::Value {
                    measure: Measure::Angle {
                        a: dir(&la, da),
                        b: dir(&lb, db),
                    },
                    target: target(
                        0.0,
                        Some(Limit {
                            min: l.min,
                            max: l.max,
                        }),
                        1.0,
                    ),
                });
            }
            terms
        }
        MateKind::Gear {
            a,
            b,
            ratio_a,
            ratio_b,
            reverse,
        } => {
            if *ratio_a <= 0.0 || *ratio_b <= 0.0 {
                return Err("gear ratios must be positive".into());
            }
            let (a, b) = (ctx.locate(a)?, ctx.locate(b)?);
            let ra = ground_rotation(&a, &mut cap, 0)?;
            let rb = ground_rotation(&b, &mut cap, 2)?;
            let k = if *reverse { -1.0 } else { 1.0 };
            coupler(vec![(ra, *ratio_a), (rb, k * ratio_b)], &mut cap)
        }
        MateKind::RackPinion {
            rack,
            pinion,
            distance_per_revolution,
            reverse,
        } => {
            if *distance_per_revolution <= 0.0 {
                return Err("the distance per revolution must be positive".into());
            }
            let (r, p) = (ctx.locate(rack)?, ctx.locate(pinion)?);
            let tr = translation(&r, &mut cap, 0)?;
            let rp = ground_rotation(&p, &mut cap, 2)?;
            let k = distance_per_revolution / TAU * if *reverse { 1.0 } else { -1.0 };
            coupler(vec![(tr, 1.0), (rp, k)], &mut cap)
        }
        MateKind::Screw {
            a,
            b,
            distance_per_revolution,
            reverse,
        } => {
            if *distance_per_revolution <= 0.0 {
                return Err("the distance per revolution must be positive".into());
            }
            let (a, b) = (ctx.locate(a)?, ctx.locate(b)?);
            let mut terms = concentric(&a, &b, Alignment::Closest, false, &mut cap)?;
            let (oa, da) = a.geo.axis().ok_or("a screw mate needs axes")?;
            let (ob, db) = b.geo.axis().ok_or("a screw mate needs axes")?;
            let v = cap.vectors(2, || {
                let ua = perpendicular(a.pose.vector(da));
                vec![
                    a.pose.inverse().vector(ua),
                    b.pose
                        .inverse()
                        .vector(ua - b.pose.vector(db) * ua.dot(b.pose.vector(db))),
                ]
            });
            let slide = Motion::Translation {
                point: pt(&b, ob),
                origin: pt(&a, oa),
                dir: dir(&a, da),
            };
            let turn = Motion::Rotation {
                vector: dir(&b, v[1]),
                reference: dir(&a, v[0]),
                axis: dir(&a, da),
            };
            let k = distance_per_revolution / TAU * if *reverse { 1.0 } else { -1.0 };
            terms.extend(coupler(vec![(slide, 1.0), (turn, k)], &mut cap));
            terms
        }
        MateKind::UniversalJoint { a, b, joint_point } => {
            let (a, b) = (ctx.locate(a)?, ctx.locate(b)?);
            let joint = joint_point.as_ref().map(|r| ctx.locate(r)).transpose()?;
            universal(&a, &b, joint.as_ref(), &mut cap)?
        }
    };
    Ok((terms, cap.c))
}

fn coincident(
    a: &Located,
    b: &Located,
    alignment: Alignment,
    cap: &mut Capture,
) -> Result<Vec<Term>, String> {
    let (wa, wb) = (a.world(), b.world());
    let s = |cap: &mut Capture| cap.sign(alignment, || world_dir(a).dot(world_dir(b)));
    Ok(match (&a.geo, &b.geo) {
        (
            Geo::Plane { normal: na, .. },
            Geo::Plane {
                origin: ob,
                normal: nb,
                ..
            },
        ) => vec![
            Term::Parallel {
                a: dir(a, *na),
                b: dir(b, *nb),
                sign: Some(s(cap)),
            },
            eq(
                Measure::PlaneDistance {
                    point: pt(b, *ob),
                    plane: plane_of(a).unwrap(),
                },
                0.0,
            ),
        ],
        (Geo::Plane { .. }, Geo::Point(p)) => vec![eq(
            Measure::PlaneDistance {
                point: pt(b, *p),
                plane: plane_of(a).unwrap(),
            },
            0.0,
        )],
        (Geo::Point(_), Geo::Plane { .. }) => return coincident(b, a, alignment, cap),
        (Geo::Plane { normal, .. }, Geo::Line { origin, dir: d, .. }) => vec![
            eq(
                Measure::Dot {
                    a: dir(a, *normal),
                    b: dir(b, *d),
                },
                0.0,
            ),
            eq(
                Measure::PlaneDistance {
                    point: pt(b, *origin),
                    plane: plane_of(a).unwrap(),
                },
                0.0,
            ),
        ],
        (Geo::Line { .. }, Geo::Plane { .. }) => return coincident(b, a, alignment, cap),
        (
            Geo::Plane { normal: na, .. },
            Geo::Circle {
                center, normal: nb, ..
            },
        ) => vec![
            Term::Parallel {
                a: dir(a, *na),
                b: dir(b, *nb),
                sign: Some(s(cap)),
            },
            eq(
                Measure::PlaneDistance {
                    point: pt(b, *center),
                    plane: plane_of(a).unwrap(),
                },
                0.0,
            ),
        ],
        (Geo::Circle { .. }, Geo::Plane { .. }) => return coincident(b, a, alignment, cap),
        (Geo::Point(p), Geo::Point(q)) => vec![Term::Points {
            a: pt(a, *p),
            b: pt(b, *q),
        }],
        (Geo::Point(p), Geo::Line { .. }) => vec![Term::PointOnLine {
            point: pt(a, *p),
            line: line_of(b).unwrap(),
        }],
        (Geo::Line { .. }, Geo::Point(_)) => return coincident(b, a, alignment, cap),
        (Geo::Point(p), Geo::Cylinder { radius, .. }) => vec![eq(
            Measure::LineDistance {
                point: pt(a, *p),
                line: line_of(b).unwrap(),
            },
            *radius,
        )],
        (Geo::Cylinder { .. }, Geo::Point(_)) => return coincident(b, a, alignment, cap),
        (Geo::Point(p), Geo::Sphere { center, radius }) => vec![eq(
            Measure::PointDistance {
                a: pt(a, *p),
                b: pt(b, *center),
            },
            *radius,
        )],
        (Geo::Sphere { .. }, Geo::Point(_)) => return coincident(b, a, alignment, cap),
        (
            Geo::Point(p),
            Geo::Circle {
                center,
                normal,
                radius,
            },
        ) => vec![
            eq(
                Measure::PointDistance {
                    a: pt(a, *p),
                    b: pt(b, *center),
                },
                *radius,
            ),
            eq(
                Measure::PlaneDistance {
                    point: pt(a, *p),
                    plane: Plane::new(b.body, *center, *normal),
                },
                0.0,
            ),
        ],
        (Geo::Circle { .. }, Geo::Point(_)) => return coincident(b, a, alignment, cap),
        (
            Geo::Line { dir: da, .. },
            Geo::Line {
                origin, dir: db, ..
            },
        ) => vec![
            Term::Parallel {
                a: dir(a, *da),
                b: dir(b, *db),
                sign: match alignment {
                    Alignment::Closest => None,
                    _ => Some(s(cap)),
                },
            },
            Term::PointOnLine {
                point: pt(b, *origin),
                line: line_of(a).unwrap(),
            },
        ],
        (
            Geo::Circle {
                center: ca,
                normal: na,
                ..
            },
            Geo::Circle {
                center: cb,
                normal: nb,
                ..
            },
        ) => vec![
            Term::Points {
                a: pt(a, *ca),
                b: pt(b, *cb),
            },
            Term::Parallel {
                a: dir(a, *na),
                b: dir(b, *nb),
                sign: Some(s(cap)),
            },
        ],
        (Geo::Point(p), Geo::Curve { points, closed }) => vec![eq(
            Measure::CurveDistance {
                point: pt(a, *p),
                curve: Curve {
                    body: b.body,
                    points: points.clone(),
                    closed: *closed,
                },
                normal: None,
            },
            0.0,
        )],
        (Geo::Curve { .. }, Geo::Point(_)) => return coincident(b, a, alignment, cap),
        _ => return Err(unsupported("coincident", &wa, &wb)),
    })
}

fn parallel(
    a: &Located,
    b: &Located,
    alignment: Alignment,
    cap: &mut Capture,
) -> Result<Vec<Term>, String> {
    let (Some(da), Some(db)) = (a.geo.direction(), b.geo.direction()) else {
        return Err(unsupported("parallel", &a.geo, &b.geo));
    };
    let plane_a = matches!(a.geo, Geo::Plane { .. });
    let plane_b = matches!(b.geo, Geo::Plane { .. });
    Ok(if plane_a != plane_b {
        // A line parallel to a plane is perpendicular to its normal.
        vec![eq(
            Measure::Dot {
                a: dir(a, da),
                b: dir(b, db),
            },
            0.0,
        )]
    } else {
        let s = cap.sign(alignment, || world_dir(a).dot(world_dir(b)));
        vec![Term::Parallel {
            a: dir(a, da),
            b: dir(b, db),
            sign: Some(s),
        }]
    })
}

fn perpendicular_mate(a: &Located, b: &Located) -> Result<Vec<Term>, String> {
    let (Some(da), Some(db)) = (a.geo.direction(), b.geo.direction()) else {
        return Err(unsupported("perpendicular", &a.geo, &b.geo));
    };
    let plane_a = matches!(a.geo, Geo::Plane { .. });
    let plane_b = matches!(b.geo, Geo::Plane { .. });
    Ok(if plane_a != plane_b {
        // A line perpendicular to a plane is parallel to its normal.
        vec![Term::Parallel {
            a: dir(a, da),
            b: dir(b, db),
            sign: None,
        }]
    } else {
        vec![eq(
            Measure::Dot {
                a: dir(a, da),
                b: dir(b, db),
            },
            0.0,
        )]
    })
}

fn tangent(
    a: &Located,
    b: &Located,
    alignment: Alignment,
    cap: &mut Capture,
) -> Result<Vec<Term>, String> {
    let (wa, wb) = (a.world(), b.world());
    let side = |cap: &mut Capture, plane: &Located, point: DVec3| {
        let (o, n) = match plane.world() {
            Geo::Plane { origin, normal, .. } => (origin, normal),
            _ => unreachable!(),
        };
        match alignment {
            Alignment::Aligned => 1.0,
            Alignment::AntiAligned => -1.0,
            Alignment::Closest => cap.side(|| (point - o).dot(n)),
        }
    };
    let internal = |cap: &mut Capture, d: f64, ra: f64, rb: f64| {
        if cap.fresh || cap.c.internal.is_none() {
            cap.c.internal = Some((d - (ra - rb).abs()).abs() < (d - (ra + rb)).abs());
        }
        if cap.c.internal.unwrap() {
            (ra - rb).abs()
        } else {
            ra + rb
        }
    };
    Ok(match (&a.geo, &b.geo) {
        (
            Geo::Plane { normal, .. },
            Geo::Cylinder {
                origin,
                dir: d,
                radius,
            },
        ) => {
            let wo = match &wb {
                Geo::Cylinder { origin, .. } => *origin,
                _ => unreachable!(),
            };
            let s = side(cap, a, wo);
            vec![
                eq(
                    Measure::Dot {
                        a: dir(a, *normal),
                        b: dir(b, *d),
                    },
                    0.0,
                ),
                eq(
                    Measure::PlaneDistance {
                        point: pt(b, *origin),
                        plane: plane_of(a).unwrap(),
                    },
                    s * radius,
                ),
            ]
        }
        (Geo::Plane { .. }, Geo::Sphere { center, radius }) => {
            let s = side(cap, a, wb.point());
            vec![eq(
                Measure::PlaneDistance {
                    point: pt(b, *center),
                    plane: plane_of(a).unwrap(),
                },
                s * radius,
            )]
        }
        (Geo::Cylinder { .. } | Geo::Sphere { .. }, Geo::Plane { .. }) => {
            return tangent(b, a, alignment, cap);
        }
        (
            Geo::Cylinder {
                dir: da,
                radius: ra,
                ..
            },
            Geo::Cylinder {
                origin,
                dir: db,
                radius: rb,
            },
        ) => {
            let (wao, wad) = wa.axis().unwrap();
            let d = (wb.point() - wao)
                .reject_from_normalized(wad.normalize())
                .length();
            let r = internal(cap, d, *ra, *rb);
            vec![
                Term::Parallel {
                    a: dir(a, *da),
                    b: dir(b, *db),
                    sign: None,
                },
                eq(
                    Measure::LineDistance {
                        point: pt(b, *origin),
                        line: line_of(a).unwrap(),
                    },
                    r,
                ),
            ]
        }
        (
            Geo::Sphere {
                center: ca,
                radius: ra,
            },
            Geo::Sphere {
                center: cb,
                radius: rb,
            },
        ) => {
            let d = wa.point().distance(wb.point());
            let r = internal(cap, d, *ra, *rb);
            vec![eq(
                Measure::PointDistance {
                    a: pt(a, *ca),
                    b: pt(b, *cb),
                },
                r,
            )]
        }
        (Geo::Cylinder { radius: ra, .. }, Geo::Sphere { center, radius: rb }) => {
            let (wao, wad) = wa.axis().unwrap();
            let d = (wb.point() - wao)
                .reject_from_normalized(wad.normalize())
                .length();
            let r = internal(cap, d, *ra, *rb);
            vec![eq(
                Measure::LineDistance {
                    point: pt(b, *center),
                    line: line_of(a).unwrap(),
                },
                r,
            )]
        }
        (Geo::Sphere { .. }, Geo::Cylinder { .. }) => return tangent(b, a, alignment, cap),
        _ => return Err(unsupported("tangent", &wa, &wb)),
    })
}

fn concentric(
    a: &Located,
    b: &Located,
    alignment: Alignment,
    lock_rotation: bool,
    cap: &mut Capture,
) -> Result<Vec<Term>, String> {
    let mut terms = match (line_of(a), line_of(b), center_of(a), center_of(b)) {
        (Some(la), Some(lb), _, _) => {
            let sign = match alignment {
                Alignment::Closest => None,
                _ => Some(cap.sign(alignment, || world_dir(a).dot(world_dir(b)))),
            };
            vec![
                Term::Parallel {
                    a: la.direction(),
                    b: lb.direction(),
                    sign,
                },
                Term::PointOnLine {
                    point: lb.point(),
                    line: la,
                },
            ]
        }
        (Some(la), None, _, Some(cb)) => vec![Term::PointOnLine {
            point: cb,
            line: la,
        }],
        (None, Some(lb), Some(ca), _) => vec![Term::PointOnLine {
            point: ca,
            line: lb,
        }],
        (None, None, Some(ca), Some(cb)) => vec![Term::Points { a: ca, b: cb }],
        _ => return Err(unsupported("concentric", &a.world(), &b.world())),
    };
    if lock_rotation {
        let ((_, da), (_, db)) = (
            a.geo.axis().ok_or("lock rotation needs two axes")?,
            b.geo.axis().ok_or("lock rotation needs two axes")?,
        );
        let v = cap.vectors(2, || {
            let wa = a.pose.vector(da);
            let u = perpendicular(wa);
            let wb = b.pose.vector(db);
            vec![
                a.pose.inverse().vector(u),
                b.pose.inverse().vector(u - wb * u.dot(wb)),
            ]
        });
        terms.push(eq(
            Measure::SignedAngle {
                a: dir(a, v[0]),
                b: dir(b, v[1]),
                axis: dir(a, da),
            },
            0.0,
        ));
    }
    Ok(terms)
}

fn distance_mate(
    a: &Located,
    b: &Located,
    distance: f64,
    flip: bool,
    limit: Option<Limit>,
    cap: &mut Capture,
) -> Result<Vec<Term>, String> {
    let (wa, wb) = (a.world(), b.world());
    let flip = if flip { -1.0 } else { 1.0 };
    let unsigned = |measure: Measure| Term::Value {
        measure,
        target: target(distance, limit, 1.0),
    };
    Ok(match (&a.geo, &b.geo) {
        (
            Geo::Plane { normal: na, .. },
            Geo::Plane {
                origin, normal: nb, ..
            },
        ) => {
            let s = cap.sign(Alignment::Closest, || world_dir(a).dot(world_dir(b)));
            let (o, n) = (wa.point(), world_dir(a));
            let side = cap.side(|| (wb.point() - o).dot(n)) * flip;
            vec![
                Term::Parallel {
                    a: dir(a, *na),
                    b: dir(b, *nb),
                    sign: Some(s),
                },
                Term::Value {
                    measure: Measure::PlaneDistance {
                        point: pt(b, *origin),
                        plane: plane_of(a).unwrap(),
                    },
                    target: target(distance, limit, side),
                },
            ]
        }
        (Geo::Plane { normal, .. }, _) => {
            let p = b.geo.point();
            let (o, n) = (wa.point(), world_dir(a));
            let side = cap.side(|| (wb.point() - o).dot(n)) * flip;
            let mut terms = vec![Term::Value {
                measure: Measure::PlaneDistance {
                    point: pt(b, p),
                    plane: plane_of(a).unwrap(),
                },
                target: target(distance, limit, side),
            }];
            if let Geo::Cylinder { dir: d, .. } | Geo::Line { dir: d, .. } = &b.geo {
                terms.push(eq(
                    Measure::Dot {
                        a: dir(a, *normal),
                        b: dir(b, *d),
                    },
                    0.0,
                ));
            }
            terms
        }
        (_, Geo::Plane { .. }) => return distance_mate(b, a, distance, flip < 0.0, limit, cap),
        (Geo::Line { .. } | Geo::Cylinder { .. }, Geo::Line { .. } | Geo::Cylinder { .. }) => {
            let (la, lb) = (line_of(a).unwrap(), line_of(b).unwrap());
            vec![
                Term::Parallel {
                    a: la.direction(),
                    b: lb.direction(),
                    sign: None,
                },
                unsigned(Measure::LineDistance {
                    point: lb.point(),
                    line: la,
                }),
            ]
        }
        (Geo::Line { .. } | Geo::Cylinder { .. }, _) => vec![unsigned(Measure::LineDistance {
            point: pt(b, b.geo.point()),
            line: line_of(a).unwrap(),
        })],
        (_, Geo::Line { .. } | Geo::Cylinder { .. }) => vec![unsigned(Measure::LineDistance {
            point: pt(a, a.geo.point()),
            line: line_of(b).unwrap(),
        })],
        (Geo::Point(_) | Geo::Sphere { .. } | Geo::Circle { .. }, _) => {
            vec![unsigned(Measure::PointDistance {
                a: pt(a, a.geo.point()),
                b: pt(b, b.geo.point()),
            })]
        }
        _ => return Err(unsupported("distance", &wa, &wb)),
    })
}

fn profile_center(
    a: &Located,
    b: &Located,
    offset: f64,
    flip: bool,
    lock_rotation: bool,
    cap: &mut Capture,
) -> Result<Vec<Term>, String> {
    let (
        Geo::Plane {
            normal: na,
            center: ca,
            x: xa,
            ..
        },
        Geo::Plane {
            normal: nb,
            center: cb,
            x: xb,
            ..
        },
    ) = (&a.geo, &b.geo)
    else {
        return Err("a profile center mate needs two flat faces".into());
    };
    let mut terms = vec![
        Term::Parallel {
            a: dir(a, *na),
            b: dir(b, *nb),
            sign: Some(-1.0),
        },
        Term::PointOnLine {
            point: pt(b, *cb),
            line: Line::new(a.body, *ca, *na),
        },
        eq(
            Measure::PlaneDistance {
                point: pt(b, *cb),
                plane: Plane::new(a.body, *ca, *na),
            },
            if flip { -offset } else { offset },
        ),
    ];
    if lock_rotation {
        if cap.fresh || cap.c.angle.is_none() {
            let (wxa, wxb, wn) = (a.pose.vector(*xa), b.pose.vector(*xb), a.pose.vector(*na));
            cap.c.angle = Some(of_mate::measure_value(
                &[],
                &Measure::SignedAngle {
                    a: Dir::new(None, wxa),
                    b: Dir::new(None, wxb),
                    axis: Dir::new(None, wn),
                },
            ));
        }
        let angle = cap.c.angle.unwrap();
        terms.push(eq(
            Measure::SignedAngle {
                a: dir(a, *xa),
                b: dir(b, *xb),
                axis: dir(a, *na),
            },
            angle,
        ));
    }
    Ok(terms)
}

fn symmetric(
    a: &Located,
    b: &Located,
    s: &Located,
    cap: &mut Capture,
) -> Result<Vec<Term>, String> {
    let mirror = plane_of(s).ok_or("select a plane or flat face to be symmetric about")?;
    let reflect = |v: DVec3, n: DVec3| v - n * (2.0 * v.dot(n));
    let wn = world_dir(s).normalize_or_zero();
    Ok(match (&a.geo, &b.geo) {
        (
            Geo::Plane {
                origin, normal: na, ..
            },
            Geo::Plane { normal: nb, .. },
        ) => {
            let sign = cap.sign(Alignment::Closest, || {
                reflect(world_dir(a), wn).dot(world_dir(b))
            });
            vec![
                Term::MirrorParallel {
                    a: dir(a, *na),
                    b: dir(b, *nb),
                    plane: mirror,
                    sign,
                },
                eq(
                    Measure::MirrorPlaneDistance {
                        point: pt(a, *origin),
                        mirror,
                        plane: plane_of(b).unwrap(),
                    },
                    0.0,
                ),
            ]
        }
        (Geo::Line { .. } | Geo::Cylinder { .. }, Geo::Line { .. } | Geo::Cylinder { .. }) => {
            let ((oa, da), (ob, db)) = (a.geo.axis().unwrap(), b.geo.axis().unwrap());
            let sign = cap.sign(Alignment::Closest, || {
                reflect(world_dir(a), wn).dot(world_dir(b))
            });
            // The reflected axis point lies on both planes through B's axis.
            let e1 = perpendicular(db);
            let e2 = db.cross(e1).normalize_or_zero();
            vec![
                Term::MirrorParallel {
                    a: dir(a, da),
                    b: dir(b, db),
                    plane: mirror,
                    sign,
                },
                eq(
                    Measure::MirrorPlaneDistance {
                        point: pt(a, oa),
                        mirror,
                        plane: Plane::new(b.body, ob, e1),
                    },
                    0.0,
                ),
                eq(
                    Measure::MirrorPlaneDistance {
                        point: pt(a, oa),
                        mirror,
                        plane: Plane::new(b.body, ob, e2),
                    },
                    0.0,
                ),
            ]
        }
        _ => match (center_of(a), center_of(b)) {
            (Some(pa), Some(pb)) => vec![Term::MirrorPoints {
                a: pa,
                b: pb,
                plane: mirror,
            }],
            _ => return Err(unsupported("symmetric", &a.world(), &b.world())),
        },
    })
}

fn width_mate(
    w: &[Located; 2],
    t: &[Located; 2],
    constraint: WidthConstraint,
    cap: &mut Capture,
) -> Result<Vec<Term>, String> {
    let planes = [
        plane_of(&w[0]),
        plane_of(&w[1]),
        plane_of(&t[0]),
        plane_of(&t[1]),
    ];
    let [Some(w1), Some(w2), Some(t1), Some(t2)] = planes else {
        return Err("a width mate needs four flat faces".into());
    };
    let d = |p: Plane, from: Plane| Measure::PlaneDistance {
        point: p.point(),
        plane: from,
    };
    let mut terms = vec![
        Term::Parallel {
            a: w1.direction(),
            b: t1.direction(),
            sign: None,
        },
        Term::Parallel {
            a: w1.direction(),
            b: w2.direction(),
            sign: None,
        },
    ];
    // Width and tab thickness, measured once in their own components.
    let world = |l: &Located| l.world();
    let v = cap.vectors(1, || {
        let (
            Geo::Plane {
                origin: o1,
                normal: n1,
                ..
            },
            Geo::Plane { origin: o2, .. },
        ) = (world(&w[0]), world(&w[1]))
        else {
            unreachable!()
        };
        let (Geo::Plane { origin: q1, .. }, Geo::Plane { origin: q2, .. }) =
            (world(&t[0]), world(&t[1]))
        else {
            unreachable!()
        };
        vec![DVec3::new(
            (o2 - o1).dot(n1),
            ((q2 - q1).dot(n1)).abs(),
            0.0,
        )]
    });
    let (width, tab) = (v[0].x, v[0].y);
    let (lo, hi) = (width.min(0.0), width.max(0.0));
    match constraint {
        WidthConstraint::Centered => terms.push(eq(
            Measure::Sum(vec![(d(t1, w1), 1.0), (d(t2, w1), 1.0), (d(w2, w1), -1.0)]),
            0.0,
        )),
        WidthConstraint::Free => {
            for p in [t1, t2] {
                terms.push(Term::Value {
                    measure: d(p, w1),
                    target: Target::Range { min: lo, max: hi },
                });
            }
        }
        WidthConstraint::Dimension { distance, flip } => {
            let s = width.signum();
            terms.push(if flip {
                eq(d(t2, w1), width - s * distance)
            } else {
                eq(d(t1, w1), s * distance)
            });
        }
        WidthConstraint::Percent { percent } => {
            if !(0.0..=1.0).contains(&percent) {
                return Err("the percentage must be between 0 and 100".into());
            }
            let s = width.signum();
            let travel = width.abs() - tab;
            let center = s * (tab / 2.0 + percent * travel);
            terms.push(eq(
                Measure::Sum(vec![(d(t1, w1), 1.0), (d(t2, w1), 1.0)]),
                2.0 * center,
            ));
        }
    }
    Ok(terms)
}

/// A polyline in one body from located edges and curves.
fn curve_of(pieces: &[Located]) -> Result<Curve, String> {
    let body = pieces.first().ok_or("select the path")?.body;
    if pieces.iter().any(|p| p.body != body) {
        return Err("the path must be in one component".into());
    }
    let mut polylines = Vec::new();
    for p in pieces {
        polylines.push(match &p.geo {
            Geo::Curve { points, .. } => points.clone(),
            Geo::Line {
                ends: Some((a, b)), ..
            } => vec![*a, *b],
            Geo::Circle {
                center,
                normal,
                radius,
            } => {
                let u = perpendicular(*normal);
                let v = normal.normalize().cross(u);
                (0..=96)
                    .map(|i| {
                        let t = TAU * i as f64 / 96.0;
                        *center + (u * t.cos() + v * t.sin()) * *radius
                    })
                    .collect()
            }
            other => return Err(format!("a path can't use a {}", other.name())),
        });
    }
    let (points, closed) = geometry::chain(polylines)?;
    Ok(Curve {
        body,
        points,
        closed,
    })
}

fn curve_length(c: &Curve) -> f64 {
    let n = c.points.len();
    let open: f64 = c.points.windows(2).map(|w| w[0].distance(w[1])).sum();
    if c.closed && n > 2 {
        open + c.points[n - 1].distance(c.points[0])
    } else {
        open
    }
}

fn path_mate(
    p: &Located,
    pieces: &[Located],
    constraint: TravelConstraint,
    follow: Option<&Located>,
    roll: Option<&(Located, Located)>,
) -> Result<Vec<Term>, String> {
    let curve = curve_of(pieces)?;
    let point = pt(p, p.geo.point());
    let length = curve_length(&curve);
    let mut terms = vec![match constraint {
        TravelConstraint::Free | TravelConstraint::Center => eq(
            Measure::CurveDistance {
                point,
                curve: curve.clone(),
                normal: None,
            },
            0.0,
        ),
        TravelConstraint::Distance { distance, flip } => Term::PointAtParameter {
            point,
            curve: curve.clone(),
            at: if flip { length - distance } else { distance },
        },
        TravelConstraint::Percent { percent, flip } => Term::PointAtParameter {
            point,
            curve: curve.clone(),
            at: length * if flip { 1.0 - percent } else { percent },
        },
    }];
    if let Some(f) = follow {
        let d = f
            .geo
            .direction()
            .ok_or("pick an edge or axis of the moving component to follow the path")?;
        terms.push(Term::CurveTangent {
            dir: dir(f, d),
            point,
            curve: curve.clone(),
        });
    }
    if let Some((axis, up)) = roll {
        let (da, du) = (
            axis.geo.direction().ok_or("pick an axis to roll")?,
            up.geo.direction().ok_or("pick an up direction")?,
        );
        terms.push(Term::CurveUp {
            axis: dir(axis, da),
            up: dir(up, du),
            point,
            curve,
        });
    }
    Ok(terms)
}

/// A translation motion of `l`'s component along its direction, measured
/// from where it was when the mate was made. Uses captured vectors
/// `slot`..`slot + 2` (world origin and direction).
fn translation(l: &Located, cap: &mut Capture, slot: usize) -> Result<Motion, String> {
    let d = l
        .geo
        .direction()
        .ok_or("select an edge, axis or face for the direction")?;
    let p = l.geo.point();
    if cap.fresh || cap.c.vectors.len() < slot + 2 {
        cap.c.vectors.resize(slot + 2, [0.0; 3]);
        cap.c.vectors[slot] = l.pose.point(p).to_array();
        cap.c.vectors[slot + 1] = l.pose.vector(d).normalize_or_zero().to_array();
    }
    Ok(Motion::Translation {
        point: pt(l, p),
        origin: Pt::new(None, DVec3::from(cap.c.vectors[slot])),
        dir: Dir::new(None, DVec3::from(cap.c.vectors[slot + 1])),
    })
}

/// Rotation of `l`'s component about its own axis relative to the assembly.
/// Uses captured vectors `slot` (local spoke) and `slot + 1` (world reference).
fn ground_rotation(l: &Located, cap: &mut Capture, slot: usize) -> Result<Motion, String> {
    let (_, d) = l
        .geo
        .axis()
        .ok_or("select a cylindrical face, circular edge or axis")?;
    if cap.fresh || cap.c.vectors.len() < slot + 2 {
        cap.c.vectors.resize(slot + 2, [0.0; 3]);
        let u = perpendicular(d);
        cap.c.vectors[slot] = u.to_array();
        cap.c.vectors[slot + 1] = l.pose.vector(u).to_array();
    }
    Ok(Motion::Rotation {
        vector: dir(l, DVec3::from(cap.c.vectors[slot])),
        reference: Dir::new(None, DVec3::from(cap.c.vectors[slot + 1])),
        axis: dir(l, d),
    })
}

/// A coupler term with phase and starting values captured on creation.
fn coupler(motions: Vec<(Motion, f64)>, cap: &mut Capture) -> Vec<Term> {
    if cap.fresh || cap.c.motions.len() != motions.len() || cap.c.phase.is_none() {
        // Motion values need poses; they are evaluated in the world frame
        // because every motion here references world-located geometry.
        cap.c.motions = motions.iter().map(|_| 0.0).collect();
        cap.c.phase = None;
    }
    let start = cap.c.motions.clone();
    vec![Term::Coupler {
        motions,
        start,
        phase: cap.c.phase.unwrap_or(f64::NAN),
    }]
}

fn cam_mate(cam: &Located, follower: &Located, r: &CompRef) -> Result<Vec<Term>, String> {
    let part = cam
        .part
        .as_ref()
        .ok_or("select a face on the cam component")?;
    let state = part.state();
    let face = geometry::face_index(state, &r.entity)?;
    let shape = part.shape().ok_or("the cam has no body")?;
    let info = shape.face_info(face).map_err(|e| e.0)?;
    // The profile is the boundary of a flat face across the cam.
    let profile_face = if info.kind == of_model::geom::SurfaceKind::Plane {
        face
    } else {
        geometry::neighbours(shape, face)
            .into_iter()
            .find(|f| {
                shape
                    .face_info(*f)
                    .is_ok_and(|i| i.kind == of_model::geom::SurfaceKind::Plane)
            })
            .ok_or("the cam face needs a flat face beside it to define its profile")?
    };
    let (points, normal) = geometry::face_loop(state, profile_face)?;
    let to_body = cam.part_to_body;
    let curve = Curve {
        body: cam.body,
        points: points.iter().map(|p| to_body.point(*p)).collect(),
        closed: true,
    };
    let normal = Some(Dir::new(cam.body, to_body.vector(normal)));
    Ok(match &follower.geo {
        Geo::Point(p) => vec![eq(
            Measure::CurveDistance {
                point: pt(follower, *p),
                curve,
                normal,
            },
            0.0,
        )],
        Geo::Cylinder { origin, radius, .. }
        | Geo::Circle {
            center: origin,
            radius,
            ..
        } => vec![eq(
            Measure::CurveDistance {
                point: pt(follower, *origin),
                curve,
                normal,
            },
            *radius,
        )],
        other => return Err(format!("a cam follower can't be a {}", other.name())),
    })
}

fn slot_mate(
    slot: &Located,
    pin: &Located,
    r: &CompRef,
    constraint: TravelConstraint,
) -> Result<Vec<Term>, String> {
    let part = slot.part.as_ref().ok_or("select a face of the slot")?;
    let state = part.state();
    let shape = part.shape().ok_or("the slot has no body")?;
    let face = geometry::face_index(state, &r.entity)?;
    use of_model::geom::SurfaceKind as S;
    let cylinder = |f: u32| shape.face_info(f).ok().filter(|i| i.kind == S::Cylinder);
    let end = match cylinder(face) {
        Some(_) => face,
        None => geometry::neighbours(shape, face)
            .into_iter()
            .find(|f| cylinder(*f).is_some())
            .ok_or("select a face of a slot with rounded ends")?,
    };
    let e = cylinder(end).unwrap();
    let d = e.axis_dir.normalize();
    let size = shape.bbox().map_or(1.0, |b| b.diagonal());
    // The other end: same radius, parallel axis, nearest distinct axis.
    let other = (0..shape.count(of_model::geom::Kind::Face))
        .filter_map(|f| cylinder(f).map(|i| (f, i)))
        .filter(|(f, i)| {
            *f != end
                && (i.radius - e.radius).abs() < 1e-6 * size
                && i.axis_dir.normalize().cross(d).length() < 1e-6
        })
        .map(|(_, i)| {
            let off = (i.axis_origin - e.axis_origin).reject_from_normalized(d);
            (off.length(), i)
        })
        .filter(|(l, _)| *l > 1e-6 * size)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .ok_or("could not find the other end of the slot")?;
    let a = e.axis_origin;
    let b = a + (other.1.axis_origin - a).reject_from_normalized(d);
    let to_body = slot.part_to_body;
    let (a, b, d) = (to_body.point(a), to_body.point(b), to_body.vector(d));
    let length = a.distance(b);
    let u = (b - a) / length;
    let (_, pd) = pin
        .geo
        .axis()
        .ok_or("the pin must be an axis, cylinder or circle")?;
    let po = pin.geo.point();
    let along = Measure::Along {
        point: pt(pin, po),
        origin: Pt::new(slot.body, a),
        dir: Dir::new(slot.body, u),
    };
    let at = |v: f64| eq(along.clone(), v);
    Ok(vec![
        Term::Parallel {
            a: Dir::new(slot.body, d),
            b: dir(pin, pd),
            sign: None,
        },
        eq(
            Measure::PlaneDistance {
                point: pt(pin, po),
                plane: Plane::new(slot.body, a, d.cross(u)),
            },
            0.0,
        ),
        match constraint {
            TravelConstraint::Free => Term::Value {
                measure: along,
                target: Target::Range {
                    min: 0.0,
                    max: length,
                },
            },
            TravelConstraint::Center => at(length / 2.0),
            TravelConstraint::Distance { distance, flip } => {
                at(if flip { length - distance } else { distance })
            }
            TravelConstraint::Percent { percent, flip } => {
                at(length * if flip { 1.0 - percent } else { percent })
            }
        },
    ])
}

fn universal(
    a: &Located,
    b: &Located,
    joint: Option<&Located>,
    cap: &mut Capture,
) -> Result<Vec<Term>, String> {
    let ((oa, da), (ob, db)) = (
        a.geo
            .axis()
            .ok_or("a universal joint needs two shaft axes")?,
        b.geo
            .axis()
            .ok_or("a universal joint needs two shaft axes")?,
    );
    let v = cap.vectors(4, || {
        let (wo_a, wd_a) = (a.pose.point(oa), a.pose.vector(da).normalize());
        let (wo_b, wd_b) = (b.pose.point(ob), b.pose.vector(db).normalize());
        let point = match joint {
            Some(j) => j.world().point(),
            None => {
                // Closest point between the two axes.
                let w = wo_a - wo_b;
                let (aa, bb, cc) = (wd_a.dot(wd_a), wd_a.dot(wd_b), wd_b.dot(wd_b));
                let (dd, ee) = (wd_a.dot(w), wd_b.dot(w));
                let den = aa * cc - bb * bb;
                let s = if den.abs() < 1e-12 {
                    0.0
                } else {
                    (bb * ee - cc * dd) / den
                };
                wo_a + wd_a * s
            }
        };
        let cross_a = {
            let c = wd_a.cross(wd_b);
            if c.length() < 1e-9 {
                perpendicular(wd_a)
            } else {
                c.normalize()
            }
        };
        let cross_b = wd_b.cross(cross_a).normalize_or_zero();
        vec![
            a.pose.inverse().point(point),
            b.pose.inverse().point(point),
            a.pose.inverse().vector(cross_a),
            b.pose.inverse().vector(cross_b),
        ]
    });
    if a.world()
        .direction()
        .unwrap()
        .cross(b.world().direction().unwrap())
        .length()
        < 1e-9
        && cap.fresh
    {
        return Err("a universal joint needs shafts at an angle, not parallel".into());
    }
    Ok(vec![
        Term::Points {
            a: pt(a, v[0]),
            b: pt(b, v[1]),
        },
        eq(
            Measure::Dot {
                a: dir(a, v[2]),
                b: dir(b, v[3]),
            },
            0.0,
        ),
    ])
}

/// Fill in coupler phases and starting values from current poses. Called
/// with all solver poses after [`build`] for new mates.
pub fn settle_couplers(terms: &mut [Term], captured: &mut Captured, poses: &[Pose]) {
    for t in terms.iter_mut() {
        if let Term::Coupler {
            motions,
            start,
            phase,
        } = t
            && phase.is_nan()
        {
            let values: Vec<f64> = motions
                .iter()
                .map(|(m, _)| of_mate::motion_value(poses, m))
                .collect();
            *start = values.clone();
            *phase = motions.iter().zip(&values).map(|((_, r), v)| r * v).sum();
            captured.motions = values;
            captured.phase = Some(*phase);
        }
    }
}

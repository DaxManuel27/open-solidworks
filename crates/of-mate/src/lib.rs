//! Assembly mate solving.
//!
//! Components are rigid bodies with a [`Pose`] (translation plus unit
//! quaternion). Mates are lists of [`Term`]s over geometry fixed in a body's
//! local frame or in the assembly frame. [`solve`] moves the floating bodies
//! until every term holds, using damped least squares like the sketch solver,
//! and reports degrees of freedom, failing mates and the minimal set of mates
//! in conflict. Geometry extraction and file handling live in `of-assembly`.

mod eval;
mod solve;
mod sparse;

pub use glam::{DAffine3, DMat3, DQuat, DVec3};
pub use solve::{Drag, Report, solve};

/// A rigid placement: rotate, then translate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub translation: DVec3,
    pub rotation: DQuat,
}

impl Default for Pose {
    fn default() -> Self {
        Pose::IDENTITY
    }
}

impl Pose {
    pub const IDENTITY: Pose = Pose {
        translation: DVec3::ZERO,
        rotation: DQuat::IDENTITY,
    };

    pub fn new(translation: DVec3, rotation: DQuat) -> Pose {
        Pose {
            translation,
            rotation: rotation.normalize(),
        }
    }

    pub fn from_translation(translation: DVec3) -> Pose {
        Pose::new(translation, DQuat::IDENTITY)
    }

    /// A local point in the parent frame.
    pub fn point(&self, p: DVec3) -> DVec3 {
        self.rotation * p + self.translation
    }

    /// A local direction in the parent frame.
    pub fn vector(&self, v: DVec3) -> DVec3 {
        self.rotation * v
    }

    pub fn inverse(&self) -> Pose {
        let r = self.rotation.inverse();
        Pose {
            translation: -(r * self.translation),
            rotation: r,
        }
    }

    /// `self ∘ other`: apply `other`, then `self`.
    pub fn then(&self, other: &Pose) -> Pose {
        Pose::new(
            self.rotation * other.translation + self.translation,
            self.rotation * other.rotation,
        )
    }

    pub fn affine(&self) -> DAffine3 {
        DAffine3::from_rotation_translation(self.rotation, self.translation)
    }

    /// The rigid part of an affine transform; scale and shear are dropped.
    pub fn from_affine(a: &DAffine3) -> Pose {
        let (_, rotation, translation) = a.to_scale_rotation_translation();
        Pose::new(translation, rotation)
    }

    pub fn is_finite(&self) -> bool {
        self.translation.is_finite() && self.rotation.is_finite()
    }

    /// Move by a small step: `dt` in the parent frame, `dw` a rotation vector.
    pub(crate) fn nudged(&self, dt: DVec3, dw: DVec3) -> Pose {
        Pose::new(
            self.translation + dt,
            DQuat::from_scaled_axis(dw) * self.rotation,
        )
    }
}

/// Which body geometry belongs to: a solver body index, or `None` for the
/// assembly frame (reference planes, fixed geometry).
pub type Body = Option<usize>;

/// A point fixed in a body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pt {
    pub body: Body,
    pub at: DVec3,
}

/// A direction fixed in a body. Need not be unit length; it is normalized.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dir {
    pub body: Body,
    pub dir: DVec3,
}

/// An infinite line fixed in a body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
    pub body: Body,
    pub origin: DVec3,
    pub dir: DVec3,
}

/// A plane fixed in a body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    pub body: Body,
    pub origin: DVec3,
    pub normal: DVec3,
}

/// A polyline fixed in a body: paths, cam profiles, slot center lines.
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    pub body: Body,
    pub points: Vec<DVec3>,
    pub closed: bool,
}

impl Pt {
    pub fn new(body: Body, at: DVec3) -> Pt {
        Pt { body, at }
    }
}

impl Dir {
    pub fn new(body: Body, dir: DVec3) -> Dir {
        Dir { body, dir }
    }
}

impl Line {
    pub fn new(body: Body, origin: DVec3, dir: DVec3) -> Line {
        Line { body, origin, dir }
    }
    pub fn point(&self) -> Pt {
        Pt::new(self.body, self.origin)
    }
    pub fn direction(&self) -> Dir {
        Dir::new(self.body, self.dir)
    }
}

impl Plane {
    pub fn new(body: Body, origin: DVec3, normal: DVec3) -> Plane {
        Plane {
            body,
            origin,
            normal,
        }
    }
    pub fn point(&self) -> Pt {
        Pt::new(self.body, self.origin)
    }
    pub fn direction(&self) -> Dir {
        Dir::new(self.body, self.normal)
    }
}

/// A scalar the solver can drive to a value or keep inside a range.
#[derive(Debug, Clone, PartialEq)]
pub enum Measure {
    /// Signed distance of a point from a plane, along the plane normal.
    PlaneDistance { point: Pt, plane: Plane },
    /// Distance between two points.
    PointDistance { a: Pt, b: Pt },
    /// Perpendicular distance from a point to a line.
    LineDistance { point: Pt, line: Line },
    /// Angle between two directions, 0..π.
    Angle { a: Dir, b: Dir },
    /// Signed angle from `a` to `b` about `axis` (both projected), −π..π.
    SignedAngle { a: Dir, b: Dir, axis: Dir },
    /// Dot product of two unit directions.
    Dot { a: Dir, b: Dir },
    /// Position of a point along a direction from an origin.
    Along { point: Pt, origin: Pt, dir: Dir },
    /// Distance from a point to a curve. With `normal`, the point and curve
    /// are projected onto planes perpendicular to it first (cam profiles).
    CurveDistance {
        point: Pt,
        curve: Curve,
        normal: Option<Dir>,
    },
    /// Arc length along a curve to the point nearest `point`.
    CurveParameter { point: Pt, curve: Curve },
    /// Signed distance from `plane` of `point` reflected in `mirror`.
    MirrorPlaneDistance {
        point: Pt,
        mirror: Plane,
        plane: Plane,
    },
    /// A weighted sum of measures.
    Sum(Vec<(Measure, f64)>),
}

/// The value a [`Measure`] must take.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    Equal(f64),
    /// Inclusive range; the term is inactive inside it.
    Range {
        min: f64,
        max: f64,
    },
}

/// One motion of a coupler: a rotation about an axis or a translation.
#[derive(Debug, Clone, PartialEq)]
pub enum Motion {
    /// Angle of `vector` (fixed in the rotating body) from `reference`
    /// (fixed in another body or the assembly) about `axis`.
    Rotation {
        vector: Dir,
        reference: Dir,
        axis: Dir,
    },
    /// Position of `point` along `dir` from `origin`.
    Translation { point: Pt, origin: Pt, dir: Dir },
}

/// One constraint equation group.
#[derive(Debug, Clone, PartialEq)]
pub enum Term {
    /// Two points coincide (3 rows).
    Points { a: Pt, b: Pt },
    /// A point lies on a line (3 rows).
    PointOnLine { point: Pt, line: Line },
    /// Two directions are parallel. With a sign, `a = sign · b`; without,
    /// either sense satisfies it (3 rows).
    Parallel { a: Dir, b: Dir, sign: Option<f64> },
    /// A scalar measure takes a value or stays in a range (1 row).
    Value { measure: Measure, target: Target },
    /// `b`'s pose relative to `a`'s stays `relative` (6 rows).
    Lock { a: Body, b: Body, relative: Pose },
    /// A point lies at a given arc length along a curve (3 rows).
    PointAtParameter { point: Pt, curve: Curve, at: f64 },
    /// `b` is `a` reflected in `plane` (3 rows).
    MirrorPoints { a: Pt, b: Pt, plane: Plane },
    /// `b = sign · reflect(a)` in `plane` (3 rows).
    MirrorParallel {
        a: Dir,
        b: Dir,
        plane: Plane,
        sign: f64,
    },
    /// `dir` is parallel to the curve tangent nearest `point` (3 rows).
    CurveTangent { dir: Dir, point: Pt, curve: Curve },
    /// `axis` points along `up` with the curve tangent nearest `point`
    /// removed: roll control along a path (3 rows).
    CurveUp {
        axis: Dir,
        up: Dir,
        point: Pt,
        curve: Curve,
    },
    /// `Σ ratio · motion = phase`, angles unwrapped from `start` (1 row).
    /// `start` holds each motion's value before this solve.
    Coupler {
        motions: Vec<(Motion, f64)>,
        start: Vec<f64>,
        phase: f64,
    },
}

/// A mate: an ID reported back in diagnostics and its terms.
#[derive(Debug, Clone, PartialEq)]
pub struct Mate {
    pub id: u32,
    pub terms: Vec<Term>,
}

impl Term {
    /// Bodies whose poses this term reads.
    pub fn bodies(&self) -> Vec<usize> {
        let mut out = Vec::new();
        let mut push = |b: Body| {
            if let Some(b) = b {
                out.push(b);
            }
        };
        match self {
            Term::Points { a, b } => {
                push(a.body);
                push(b.body);
            }
            Term::PointOnLine { point, line } => {
                push(point.body);
                push(line.body);
            }
            Term::Parallel { a, b, .. } => {
                push(a.body);
                push(b.body);
            }
            Term::Value { measure, .. } => measure.bodies(&mut push),
            Term::Lock { a, b, .. } => {
                push(*a);
                push(*b);
            }
            Term::PointAtParameter { point, curve, .. } => {
                push(point.body);
                push(curve.body);
            }
            Term::Coupler { motions, .. } => {
                for (m, _) in motions {
                    m.bodies(&mut push);
                }
            }
            Term::MirrorPoints { a, b, plane } => {
                push(a.body);
                push(b.body);
                push(plane.body);
            }
            Term::MirrorParallel { a, b, plane, .. } => {
                push(a.body);
                push(b.body);
                push(plane.body);
            }
            Term::CurveTangent { dir, point, curve } => {
                push(dir.body);
                push(point.body);
                push(curve.body);
            }
            Term::CurveUp {
                axis,
                up,
                point,
                curve,
            } => {
                push(axis.body);
                push(up.body);
                push(point.body);
                push(curve.body);
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

impl Measure {
    fn bodies(&self, push: &mut impl FnMut(Body)) {
        match self {
            Measure::PlaneDistance { point, plane } => {
                push(point.body);
                push(plane.body);
            }
            Measure::PointDistance { a, b } => {
                push(a.body);
                push(b.body);
            }
            Measure::LineDistance { point, line } => {
                push(point.body);
                push(line.body);
            }
            Measure::Angle { a, b } | Measure::Dot { a, b } => {
                push(a.body);
                push(b.body);
            }
            Measure::SignedAngle { a, b, axis } => {
                push(a.body);
                push(b.body);
                push(axis.body);
            }
            Measure::Along { point, origin, dir } => {
                push(point.body);
                push(origin.body);
                push(dir.body);
            }
            Measure::CurveDistance {
                point,
                curve,
                normal,
            } => {
                push(point.body);
                push(curve.body);
                if let Some(n) = normal {
                    push(n.body);
                }
            }
            Measure::CurveParameter { point, curve } => {
                push(point.body);
                push(curve.body);
            }
            Measure::MirrorPlaneDistance {
                point,
                mirror,
                plane,
            } => {
                push(point.body);
                push(mirror.body);
                push(plane.body);
            }
            Measure::Sum(parts) => {
                for (m, _) in parts {
                    m.bodies(push);
                }
            }
        }
    }
}

impl Motion {
    fn bodies(&self, push: &mut impl FnMut(Body)) {
        match self {
            Motion::Rotation {
                vector,
                reference,
                axis,
            } => {
                push(vector.body);
                push(reference.body);
                push(axis.body);
            }
            Motion::Translation { point, origin, dir } => {
                push(point.body);
                push(origin.body);
                push(dir.body);
            }
        }
    }
}

/// Everything to solve: current poses, which bodies are fixed, and mates.
#[derive(Debug, Clone, Default)]
pub struct System {
    pub poses: Vec<Pose>,
    pub fixed: Vec<bool>,
    pub mates: Vec<Mate>,
}

/// Evaluate a motion's current value, for capturing coupler phases when a
/// mate is created. Rotations are in radians, −π..π.
pub fn motion_value(poses: &[Pose], motion: &Motion) -> f64 {
    eval::motion_raw(&eval::Poses::new(poses), motion)
}

/// Evaluate a measure on the given poses.
pub fn measure_value(poses: &[Pose], measure: &Measure) -> f64 {
    eval::measure(&eval::Poses::new(poses), measure)
}

/// Wrap an angle into −π..π.
pub fn wrap(a: f64) -> f64 {
    let t = std::f64::consts::TAU;
    let w = (a + std::f64::consts::PI).rem_euclid(t) - std::f64::consts::PI;
    if w <= -std::f64::consts::PI { w + t } else { w }
}

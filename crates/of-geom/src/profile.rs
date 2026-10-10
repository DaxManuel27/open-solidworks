use glam::DVec2;
use of_kernel_sys::ffi;
use serde::{Deserialize, Serialize};

use crate::{Plane, Result, Shape};

/// A 2D curve in a sketch plane's coordinates. Arcs run counter-clockwise
/// from `start` to `end`.
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

impl Curve2 {
    pub(crate) fn to_ffi(&self) -> ffi::Curve {
        let (kind, data) = match self {
            Curve2::Line { a, b } => (0, vec![a.x, a.y, b.x, b.y]),
            Curve2::Arc {
                center: c,
                start: s,
                end: e,
            } => (1, vec![c.x, c.y, s.x, s.y, e.x, e.y]),
            Curve2::Circle { center: c, radius } => (2, vec![c.x, c.y, *radius]),
            Curve2::Spline { points, closed } => (
                if *closed { 4 } else { 3 },
                points.iter().flat_map(|p| [p.x, p.y]).collect(),
            ),
        };
        ffi::Curve { kind, data }
    }

    /// Start point (the center for closed curves without endpoints).
    pub fn start(&self) -> DVec2 {
        match self {
            Curve2::Line { a, .. } => *a,
            Curve2::Arc { start, .. } => *start,
            Curve2::Circle { center, radius } => *center + DVec2::new(*radius, 0.0),
            Curve2::Spline { points, .. } => points[0],
        }
    }

    pub fn end(&self) -> DVec2 {
        match self {
            Curve2::Line { b, .. } => *b,
            Curve2::Arc { end, .. } => *end,
            Curve2::Circle { .. } => self.start(),
            Curve2::Spline { points, closed } => {
                if *closed {
                    points[0]
                } else {
                    *points.last().unwrap()
                }
            }
        }
    }
}

/// One planar face: an outer loop and any number of holes. Each loop is a
/// list of curves joined end to end.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileFace {
    pub outer: Vec<Curve2>,
    pub holes: Vec<Vec<Curve2>>,
}

/// Closed regions of a sketch, ready to extrude or revolve.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub plane: Plane,
    pub faces: Vec<ProfileFace>,
}

impl Profile {
    /// Build the faces in the kernel: a single face, or a compound of faces.
    pub fn to_shape(&self) -> Result<Shape> {
        let mut curves = Vec::new();
        let mut loop_sizes = Vec::new();
        let mut face_loops = Vec::new();
        for f in &self.faces {
            for l in std::iter::once(&f.outer).chain(&f.holes) {
                curves.extend(l.iter().map(Curve2::to_ffi));
                loop_sizes.push(l.len() as u32);
            }
            face_loops.push(1 + f.holes.len() as u32);
        }
        let s = ffi::make_faces(&self.plane.to_ffi(), &curves, &loop_sizes, &face_loops)?;
        Ok(Shape(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DVec3, Kind};

    fn rect(w: f64, h: f64) -> Vec<Curve2> {
        let p = [
            DVec2::ZERO,
            DVec2::new(w, 0.0),
            DVec2::new(w, h),
            DVec2::new(0.0, h),
        ];
        (0..4)
            .map(|i| Curve2::Line {
                a: p[i],
                b: p[(i + 1) % 4],
            })
            .collect()
    }

    #[test]
    fn revolve_rectangle_into_tube() {
        // A 2 x 10 rectangle 3 mm from the Y axis, revolved a full turn.
        let outer: Vec<Curve2> = rect(2.0, 10.0)
            .into_iter()
            .map(|c| match c {
                Curve2::Line { a, b } => Curve2::Line {
                    a: a + DVec2::new(3.0, 0.0),
                    b: b + DVec2::new(3.0, 0.0),
                },
                c => c,
            })
            .collect();
        let profile = Profile {
            plane: Plane::front(),
            faces: vec![ProfileFace {
                outer,
                holes: vec![],
            }],
        };
        let face = profile.to_shape().unwrap();
        let tube = face
            .revolve(DVec3::ZERO, DVec3::Y, std::f64::consts::TAU)
            .unwrap()
            .shape;
        let v = tube.mass_props().unwrap().volume;
        let expected = std::f64::consts::PI * (25.0 - 9.0) * 10.0;
        assert!((v - expected).abs() < 1e-6, "{v}");
    }

    #[test]
    fn arcs_and_two_faces() {
        // A slot (two lines, two arcs) and a separate circle.
        let slot = vec![
            Curve2::Line {
                a: DVec2::new(0.0, -1.0),
                b: DVec2::new(4.0, -1.0),
            },
            Curve2::Arc {
                center: DVec2::new(4.0, 0.0),
                start: DVec2::new(4.0, -1.0),
                end: DVec2::new(4.0, 1.0),
            },
            Curve2::Line {
                a: DVec2::new(4.0, 1.0),
                b: DVec2::new(0.0, 1.0),
            },
            Curve2::Arc {
                center: DVec2::ZERO,
                start: DVec2::new(0.0, 1.0),
                end: DVec2::new(0.0, -1.0),
            },
        ];
        let circle = vec![Curve2::Circle {
            center: DVec2::new(20.0, 0.0),
            radius: 1.0,
        }];
        let profile = Profile {
            plane: Plane::top(),
            faces: vec![
                ProfileFace {
                    outer: slot,
                    holes: vec![],
                },
                ProfileFace {
                    outer: circle,
                    holes: vec![],
                },
            ],
        };
        let faces = profile.to_shape().unwrap();
        assert_eq!(faces.count(Kind::Face), 2);
        let solid = faces.prism(DVec3::Y).unwrap().shape;
        let v = solid.mass_props().unwrap().volume;
        let expected = 8.0 + std::f64::consts::PI + std::f64::consts::PI;
        assert!((v - expected).abs() < 1e-6, "{v}");
    }
}

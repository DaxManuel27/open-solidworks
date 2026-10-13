//! View orientation: how each view looks at its model, and how views derived
//! from a parent fold out around it.
//!
//! A [`Frame`] is a right-handed orthonormal basis: `x` points right on
//! paper, `y` up, and `toward` from the model at the viewer. A model point
//! `p` lands on paper at `origin + scale * (p·x, p·y)`.

use of_model::geom::{DVec2, DVec3, Plane};

use crate::document::{Orientation, Placement, Projection};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub x: DVec3,
    pub y: DVec3,
    pub toward: DVec3,
}

impl Frame {
    /// A frame looking from `toward` with `up` as near paper-up as possible.
    pub fn looking(toward: DVec3, up: DVec3) -> Frame {
        let t = toward.normalize_or(DVec3::Z);
        let mut y = up - t * up.dot(t);
        if y.length_squared() < 1e-18 {
            y = t.any_orthonormal_vector();
        }
        let y = y.normalize();
        Frame {
            x: y.cross(t),
            y,
            toward: t,
        }
    }

    /// Paper coordinates (unscaled, relative to the model origin) of a point.
    pub fn project(&self, p: DVec3) -> DVec2 {
        DVec2::new(p.dot(self.x), p.dot(self.y))
    }

    /// Depth toward the viewer.
    pub fn depth(&self, p: DVec3) -> f64 {
        p.dot(self.toward)
    }

    /// A model-space direction from paper coordinates.
    pub fn unproject(&self, v: DVec2) -> DVec3 {
        self.x * v.x + self.y * v.y
    }

    /// The view plane through the model origin, for hidden-line removal.
    pub fn plane(&self) -> Plane {
        Plane::new(DVec3::ZERO, self.x, self.toward)
    }

    /// This frame turned about the view direction by `angle` (radians,
    /// counter-clockwise on paper).
    pub fn rotated(&self, angle: f64) -> Frame {
        let (s, c) = angle.sin_cos();
        Frame {
            x: self.x * c + self.y * s,
            y: self.y * c - self.x * s,
            toward: self.toward,
        }
    }

    /// Whether two frames look the same way with the same paper axes.
    pub fn same(&self, o: &Frame) -> bool {
        self.x.abs_diff_eq(o.x, 1e-9) && self.y.abs_diff_eq(o.y, 1e-9)
    }
}

/// Frame of a named orientation. The model's Y axis is up; the front view
/// looks along -Z.
pub fn orientation(o: &Orientation) -> Frame {
    let up = DVec3::Y;
    match o {
        Orientation::Front => Frame::looking(DVec3::Z, up),
        Orientation::Back => Frame::looking(DVec3::NEG_Z, up),
        Orientation::Left => Frame::looking(DVec3::NEG_X, up),
        Orientation::Right => Frame::looking(DVec3::X, up),
        Orientation::Top => Frame::looking(DVec3::Y, DVec3::NEG_Z),
        Orientation::Bottom => Frame::looking(DVec3::NEG_Y, DVec3::Z),
        // Equal foreshortening of all three axes.
        Orientation::Isometric => Frame::looking(DVec3::new(1.0, 1.0, 1.0), up),
        // X and Z equally foreshortened, Y less (about 20.7° elevation).
        Orientation::Dimetric => Frame::looking(DVec3::new(1.0, 0.534, 1.0), up),
        // All three axes foreshortened differently.
        Orientation::Trimetric => Frame::looking(DVec3::new(1.0, 0.65, 0.75), up),
        Orientation::Custom { toward, up } => {
            Frame::looking(DVec3::from(*toward), DVec3::from(*up))
        }
    }
}

/// Paper direction (unit, from the parent) in which a placement lies.
pub fn placement_dir(p: Placement) -> DVec2 {
    let (x, y) = match p {
        Placement::Left => (-1.0, 0.0),
        Placement::Right => (1.0, 0.0),
        Placement::Above => (0.0, 1.0),
        Placement::Below => (0.0, -1.0),
        Placement::AboveLeft => (-1.0, 1.0),
        Placement::AboveRight => (1.0, 1.0),
        Placement::BelowLeft => (-1.0, -1.0),
        Placement::BelowRight => (1.0, -1.0),
    };
    DVec2::new(x, y).normalize()
}

/// The frame of a view projected from `parent` and placed on the sheet in
/// direction `p`. In third-angle projection the view placed to the right
/// shows the right side; in first-angle it shows the left side. Diagonal
/// placements give a pictorial view from that corner.
pub fn projected(parent: &Frame, p: Placement, projection: Projection) -> Frame {
    let d = placement_dir(p);
    let s = match projection {
        Projection::Third => 1.0,
        Projection::First => -1.0,
    };
    let (x, y, t) = (parent.x, parent.y, parent.toward);
    if d.x != 0.0 && d.y != 0.0 {
        let toward = t + (x * d.x.signum() + y * d.y.signum()) * s;
        return Frame::looking(toward, y);
    }
    if d.x != 0.0 {
        // Folded about the parent's vertical axis.
        let toward = x * d.x * s;
        Frame::looking(toward, y)
    } else {
        // Folded about the parent's horizontal axis: x stays.
        let toward = y * d.y * s;
        let up = -t * d.y * s;
        Frame::looking(toward, up)
    }
}

/// The frame of an auxiliary or section view of `parent`, folded out about
/// a line on the parent toward `side` (a unit paper direction perpendicular
/// to the line, where the view is placed). The view's paper axes are turned
/// so the fold line keeps its direction on paper.
pub fn folded(parent: &Frame, side: DVec2, projection: Projection) -> Frame {
    // Unrotated, this is the "above" projection with the parent turned so
    // `side` is up.
    let angle = side.y.atan2(side.x) - std::f64::consts::FRAC_PI_2;
    let turned = parent.rotated(angle);
    projected(&turned, Placement::Above, projection).rotated(-angle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: DVec3, b: DVec3) -> bool {
        a.abs_diff_eq(b, 1e-9)
    }

    #[test]
    fn standard_views_follow_y_up() {
        let f = orientation(&Orientation::Front);
        assert!(close(f.x, DVec3::X) && close(f.y, DVec3::Y) && close(f.toward, DVec3::Z));
        let t = orientation(&Orientation::Top);
        assert!(close(t.x, DVec3::X) && close(t.y, DVec3::NEG_Z));
        let r = orientation(&Orientation::Right);
        assert!(close(r.x, DVec3::NEG_Z) && close(r.y, DVec3::Y));
        for o in [
            Orientation::Isometric,
            Orientation::Dimetric,
            Orientation::Trimetric,
        ] {
            let f = orientation(&o);
            assert!((f.x.cross(f.y) - f.toward).length() < 1e-9);
            assert!(f.y.y > 0.0, "{o:?} keeps Y up");
        }
    }

    #[test]
    fn third_angle_places_the_right_view_on_the_right() {
        let front = orientation(&Orientation::Front);
        let right = projected(&front, Placement::Right, Projection::Third);
        assert!(right.same(&orientation(&Orientation::Right)));
        let top = projected(&front, Placement::Above, Projection::Third);
        assert!(top.same(&orientation(&Orientation::Top)));
        let bottom = projected(&front, Placement::Below, Projection::Third);
        assert!(bottom.same(&orientation(&Orientation::Bottom)));
    }

    #[test]
    fn first_angle_mirrors_the_layout() {
        let front = orientation(&Orientation::Front);
        let on_right = projected(&front, Placement::Right, Projection::First);
        assert!(on_right.same(&orientation(&Orientation::Left)));
        let below = projected(&front, Placement::Below, Projection::First);
        assert!(below.same(&orientation(&Orientation::Top)));
    }

    #[test]
    fn projected_views_line_up_with_their_parent() {
        // A point keeps its paper height in a side view and its paper x in
        // a view above or below.
        let front = orientation(&Orientation::Front);
        let p = DVec3::new(3.0, 5.0, -2.0);
        for proj in [Projection::First, Projection::Third] {
            for pl in [Placement::Left, Placement::Right] {
                let f = projected(&front, pl, proj);
                assert!((f.project(p).y - front.project(p).y).abs() < 1e-9);
            }
            for pl in [Placement::Above, Placement::Below] {
                let f = projected(&front, pl, proj);
                assert!((f.project(p).x - front.project(p).x).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn folding_across_a_horizontal_line_is_a_projection_above() {
        let front = orientation(&Orientation::Front);
        let f = folded(&front, DVec2::Y, Projection::Third);
        assert!(f.same(&projected(&front, Placement::Above, Projection::Third)));
        // Across a 45° edge: points along the edge keep their spacing.
        let along = DVec2::new(1.0, 1.0).normalize();
        let side = along.perp();
        let f = folded(&front, side, Projection::Third);
        let e = front.unproject(along);
        assert!((f.project(e * 10.0) - along * 10.0).length() < 1e-9);
        assert!(f.toward.dot(front.toward).abs() < 1e-9);
    }
}

//! Surface bodies: knitting, solids from closed shells, offsets, thickening,
//! deleting and patching faces, and filled patches.

use glam::DVec3;
use of_kernel_sys::ffi;

use crate::{OpResult, Plane, Result, Shape, convert_history};

/// Continuity of a filled patch with the face beside one of its edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Continuity {
    Contact,
    Tangent,
    Curvature,
}

/// One edge of a filled patch's boundary.
pub struct FillEdge<'a> {
    pub edge: &'a Shape,
    /// The face beside the edge, for tangent and curvature continuity.
    pub support: Option<&'a Shape>,
    pub continuity: Continuity,
}

/// Faces knitted into shells, and how many edges were left open.
pub struct Knitted {
    pub result: OpResult,
    pub free_edges: u32,
}

impl Shape {
    /// Sew the faces of this shape into shells, closing gaps up to
    /// `tolerance`.
    pub fn sew(&self, tolerance: f64) -> Result<Knitted> {
        let mut h = Vec::new();
        let mut free_edges = 0;
        let s = ffi::sew(&self.0, tolerance, &mut h, &mut free_edges)?;
        Ok(Knitted {
            result: OpResult {
                shape: Shape(s),
                history: convert_history(h),
            },
            free_edges,
        })
    }

    /// A solid from each closed shell of this shape. Fails if any shell is open.
    pub fn solidify(&self) -> Result<OpResult> {
        let mut h = Vec::new();
        let s = ffi::solidify(&self.0, &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    /// Move every face `distance` along its normal; 0 makes a copy.
    pub fn offset_shape(&self, distance: f64) -> Result<OpResult> {
        let mut h = Vec::new();
        let s = ffi::offset_shape(&self.0, distance, &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    /// Thicken a surface into a solid, along its normal when `thickness` is
    /// positive and against it when negative.
    pub fn thicken(&self, thickness: f64) -> Result<OpResult> {
        let mut h = Vec::new();
        let s = ffi::thicken(&self.0, thickness, &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    /// Remove faces and extend their neighbours to close the gap.
    pub fn defeature(&self, faces: &[u32]) -> Result<OpResult> {
        let mut h = Vec::new();
        let s = ffi::defeature(&self.0, faces, &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    /// Remove faces, leaving the rest as open shells.
    pub fn remove_faces(&self, faces: &[u32]) -> Result<OpResult> {
        let mut h = Vec::new();
        let s = ffi::remove_faces(&self.0, faces, &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    /// A patch bounded by edges, passing through `constraints` (edges or
    /// null). Returns the patch and its largest distance from the boundary.
    pub fn fill(boundary: &[FillEdge], constraints: &[&Shape]) -> Result<(Shape, f64)> {
        let edges = Shape::compound(boundary.iter().map(|b| b.edge));
        let supports = Shape::compound(boundary.iter().filter_map(|b| b.support));
        let support_of = boundary
            .iter()
            .map(|b| match b.support {
                Some(face) if b.continuity != Continuity::Contact => {
                    Ok(face.indices_in(&supports, crate::Kind::Face)?[0] as i32)
                }
                _ => Ok(-1),
            })
            .collect::<Result<Vec<i32>>>()?;
        let orders: Vec<u8> = boundary
            .iter()
            .map(|b| match b.continuity {
                Continuity::Contact => 0,
                Continuity::Tangent => 1,
                Continuity::Curvature => 2,
            })
            .collect();
        let constraints = Shape::compound(constraints.iter().copied());
        let mut error = 0.0;
        let s = ffi::fill(
            &edges.0,
            &supports.0,
            &support_of,
            &orders,
            &constraints.0,
            &mut error,
        )?;
        Ok((Shape(s), error))
    }

    /// Sweep an open or closed profile along a path into a surface.
    /// Orientation: 0 follows the path, 1 keeps the profile's direction, 2
    /// keeps `plane`'s normal.
    pub fn sweep_surface(
        profile: &Shape,
        path: &Shape,
        plane: &Plane,
        orientation: u8,
    ) -> Result<OpResult> {
        let mut h = Vec::new();
        let s = ffi::sweep_surface(&profile.0, &path.0, &plane.to_ffi(), orientation, &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    /// A surface through wires in order.
    pub fn loft_surface(profiles: &Shape, ruled: bool, preserve: bool) -> Result<OpResult> {
        let mut h = Vec::new();
        let s = ffi::loft_surface(&profiles.0, ruled, preserve, &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    /// Each shell, with bare faces as shells of their own.
    pub fn shells(&self) -> Vec<Shape> {
        let Ok(c) = ffi::shells_of(&self.0) else {
            return Vec::new();
        };
        Shape(c).children()
    }

    /// The direct children of a compound, solid or shell.
    pub fn children(&self) -> Vec<Shape> {
        (0..ffi::child_count(&self.0))
            .filter_map(|i| ffi::child(&self.0, i).ok().map(Shape))
            .collect()
    }

    /// Edges on only one face: the open boundary of a surface.
    pub fn free_edges(&self) -> Result<Shape> {
        Ok(Shape(ffi::free_edges(&self.0)?))
    }

    /// Cut the solids with a surface, removing the pieces on the side its
    /// normal points to (the other side with `flip`).
    pub fn cut_with_surface(&self, surface: &Shape, flip: bool) -> Result<OpResult> {
        let mut h = Vec::new();
        let s = ffi::cut_with_surface(&self.0, &surface.0, flip, &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    /// A surface through sections (wires, in order) and direction-2 curves
    /// crossing them.
    pub fn boundary_surface(sections: &Shape, guides: &Shape) -> Result<OpResult> {
        let mut h = Vec::new();
        let s = ffi::boundary_surface(&sections.0, &guides.0, &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    /// A planar face bounded by a closed loop of edges.
    pub fn planar_face(edges: &Shape) -> Result<Shape> {
        Ok(Shape(ffi::planar_face(&edges.0)?))
    }

    /// The face's surface extended `length` past its boundary, untrimmed.
    pub fn extend_face(&self, length: f64) -> Result<Shape> {
        Ok(Shape(ffi::extend_face(&self.0, length)?))
    }

    /// The closed regions the faces of this shape enclose, each a solid.
    pub fn cells(&self) -> Result<Vec<Shape>> {
        Ok(Shape(ffi::cells(&self.0)?).solids())
    }

    /// Whether a point is inside (1), outside (-1) or on (0) this solid.
    pub fn classify(&self, point: DVec3) -> Result<i8> {
        Ok(ffi::classify(&self.0, &crate::v3(point))?)
    }

    /// A point strictly inside this solid.
    pub fn inner_point(&self) -> Result<DVec3> {
        Ok(crate::dv3(ffi::inner_point(&self.0)?))
    }

    /// Faces of this shape lying on `target`, within `tolerance`.
    pub fn faces_on(&self, target: &Shape, tolerance: f64) -> Result<Vec<u32>> {
        Ok(ffi::faces_on(&self.0, &target.0, tolerance)?)
    }

    /// Merge neighbouring faces and edges that lie on one surface.
    pub fn unify(&self) -> Result<OpResult> {
        let mut h = Vec::new();
        let s = ffi::unify(&self.0, &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    /// Remove the holes (inner loops) of faces.
    pub fn remove_holes(&self, faces: &[u32]) -> Result<OpResult> {
        let mut h = Vec::new();
        let s = ffi::remove_holes(&self.0, faces, &mut h)?;
        Ok(OpResult {
            shape: Shape(s),
            history: convert_history(h),
        })
    }

    /// Whether this shape has faces outside any solid.
    pub fn has_open_faces(&self) -> bool {
        let in_solids: u32 = self
            .solids()
            .iter()
            .map(|s| s.count(crate::Kind::Face))
            .sum();
        self.count(crate::Kind::Face) > in_solids
    }

    /// The surface area of all faces.
    pub fn area(&self) -> f64 {
        self.mass_props().map(|m| m.area).unwrap_or(0.0)
    }

    /// The centroid of the shape's bounding box.
    pub fn bbox_center(&self) -> Option<DVec3> {
        self.bbox().map(|b| b.center())
    }
}

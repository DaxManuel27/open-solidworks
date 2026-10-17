//! Analysis queries: draft and undercut for moulding, deviation between a
//! mesh and the model, and projected area. They never change the model.

use of_geom::{DVec3, Kind, Shape};
use of_mesh::bvh::Bvh;
use serde::{Deserialize, Serialize};

use crate::build::State;
use crate::meshes::{deflection, tessellate};

/// How a face drafts against the pull direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftClass {
    /// Draft toward the pull direction of at least the angle everywhere.
    Positive,
    /// Draft away from it of at least the angle everywhere.
    Negative,
    /// Less than the angle everywhere.
    RequiresDraft,
    /// Positive in places and negative in others.
    Straddle,
}

/// One face's draft.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaceDraft {
    /// Index in [`State::everything`].
    pub face: u32,
    pub class: DraftClass,
    /// The smallest and largest draft on the face, radians: the angle
    /// between the face and the pull direction, positive facing along it.
    pub min: f64,
    pub max: f64,
    /// Parts of the face have less draft than the angle.
    pub steep: bool,
}

fn faces_mesh(shape: &Shape) -> Result<of_mesh::TriMesh, String> {
    tessellate(shape, deflection(shape, 0.002))
}

/// Draft of every face of the solids against `pull`, with `angle` the
/// required draft (radians).
pub fn draft(state: &State, pull: DVec3, angle: f64) -> Result<Vec<FaceDraft>, String> {
    let body = state.body.as_ref().ok_or("there's no solid to analyse")?;
    let pull = pull.normalize();
    let mesh = faces_mesh(&body.shape)?;
    let n = body.shape.count(Kind::Face) as usize;
    let mut lo = vec![f64::INFINITY; n];
    let mut hi = vec![f64::NEG_INFINITY; n];
    for t in 0..mesh.triangles.len() {
        if mesh.triangle_area(t) < 1e-12 {
            continue;
        }
        let d = mesh.normal(t).dot(pull).clamp(-1.0, 1.0).asin();
        let f = mesh.groups[t] as usize;
        lo[f] = lo[f].min(d);
        hi[f] = hi[f].max(d);
    }
    let eps = 1e-6;
    Ok((0..n)
        .filter(|f| lo[*f].is_finite())
        .map(|f| {
            let (a, b) = (lo[f], hi[f]);
            let pos = b >= angle - eps;
            let neg = a <= -angle + eps;
            let class = match (pos, neg) {
                (true, true) => DraftClass::Straddle,
                (true, false) if a >= angle - eps => DraftClass::Positive,
                (false, true) if b <= -angle + eps => DraftClass::Negative,
                (true, false) => DraftClass::Positive,
                (false, true) => DraftClass::Negative,
                (false, false) => DraftClass::RequiresDraft,
            };
            let steep = match class {
                DraftClass::Positive => a < angle - eps,
                DraftClass::Negative => b > -angle + eps,
                _ => false,
            };
            FaceDraft {
                face: f as u32,
                class,
                min: a,
                max: b,
                steep,
            }
        })
        .collect())
}

/// Whether a face can leave the mould.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UndercutClass {
    None,
    /// Faces facing along the pull that something blocks from above.
    Direction1,
    /// Faces facing against the pull that something blocks from below.
    Direction2,
    /// Faces blocked both ways.
    Occluded,
    /// Partly undercut.
    Straddle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaceUndercut {
    pub face: u32,
    pub class: UndercutClass,
}

/// Undercut faces of the solids for a mould opening along `pull`.
pub fn undercut(state: &State, pull: DVec3) -> Result<Vec<FaceUndercut>, String> {
    let body = state.body.as_ref().ok_or("there's no solid to analyse")?;
    let pull = pull.normalize();
    let mesh = faces_mesh(&body.shape)?;
    let bvh = Bvh::new(&mesh);
    let size = mesh.bounds().diagonal().max(1e-9);
    let eps = size * 1e-6;
    let n = body.shape.count(Kind::Face) as usize;
    let mut seen: Vec<[bool; 5]> = vec![[false; 5]; n];
    for t in 0..mesh.triangles.len() {
        if mesh.triangle_area(t) < 1e-12 {
            continue;
        }
        let [a, b, c] = mesh.corners(t);
        let normal = mesh.normal(t);
        let start = (a + b + c) / 3.0 + normal * eps;
        let blocked = |dir: DVec3| bvh.ray(start, dir, eps).is_some();
        let along = normal.dot(pull);
        let class = if along > 1e-6 {
            if blocked(pull) {
                UndercutClass::Direction1
            } else {
                UndercutClass::None
            }
        } else if along < -1e-6 {
            if blocked(-pull) {
                UndercutClass::Direction2
            } else {
                UndercutClass::None
            }
        } else if blocked(pull) && blocked(-pull) {
            UndercutClass::Occluded
        } else {
            UndercutClass::None
        };
        let k = match class {
            UndercutClass::None => 0,
            UndercutClass::Direction1 => 1,
            UndercutClass::Direction2 => 2,
            UndercutClass::Occluded => 3,
            UndercutClass::Straddle => 4,
        };
        seen[mesh.groups[t] as usize][k] = true;
    }
    Ok(seen
        .iter()
        .enumerate()
        .filter(|(_, s)| s.iter().any(|x| *x))
        .map(|(f, s)| {
            let kinds: Vec<usize> = (0..4).filter(|k| s[*k]).collect();
            let class = match kinds.as_slice() {
                [0] => UndercutClass::None,
                [1] => UndercutClass::Direction1,
                [2] => UndercutClass::Direction2,
                [3] => UndercutClass::Occluded,
                _ => UndercutClass::Straddle,
            };
            FaceUndercut {
                face: f as u32,
                class,
            }
        })
        .collect())
}

/// Distances from a mesh's vertices to the model's surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Deviation {
    pub distances: Vec<f64>,
    pub mean: f64,
    pub rms: f64,
    pub max: f64,
    /// The share of vertices within each of `limits`.
    pub within: Vec<(f64, f64)>,
}

/// How far each vertex of `mesh` lies from `shape`'s faces.
pub fn deviation(
    mesh: &of_mesh::TriMesh,
    shape: &Shape,
    limits: &[f64],
) -> Result<Deviation, String> {
    let model = tessellate(shape, deflection(shape, 0.0005))?;
    let bvh = Bvh::new(&model);
    let distances: Vec<f64> = mesh
        .positions
        .iter()
        .map(|p| bvh.closest(*p).map_or(f64::INFINITY, |c| c.distance))
        .collect();
    let n = distances.len().max(1) as f64;
    let mean = distances.iter().sum::<f64>() / n;
    let rms = (distances.iter().map(|d| d * d).sum::<f64>() / n).sqrt();
    let max = distances.iter().copied().fold(0.0, f64::max);
    let within = limits
        .iter()
        .map(|l| {
            (
                *l,
                distances.iter().filter(|d| **d <= *l).count() as f64 / n,
            )
        })
        .collect();
    Ok(Deviation {
        distances,
        mean,
        rms,
        max,
        within,
    })
}

/// The area of faces seen along `normal`: with `hidden`, every face's
/// projection counts (overlaps add up); without, only the outline's area
/// counts. Returns the area and its possible error.
pub fn projected_area(
    state: &State,
    faces: &[u32],
    normal: DVec3,
    hidden: bool,
) -> Result<(f64, f64), String> {
    let all = state.everything().ok_or("there's no body")?;
    let picked = faces
        .iter()
        .map(|f| all.shape.sub_shape(Kind::Face, *f).map_err(|e| e.0))
        .collect::<Result<Vec<_>, _>>()?;
    let shape = Shape::compound(&picked);
    let mesh = tessellate(&shape, deflection(&shape, 0.001))?;
    let n = normal.normalize();
    if hidden {
        let area = (0..mesh.triangles.len())
            .map(|t| {
                let [a, b, c] = mesh.corners(t);
                0.5 * (b - a).cross(c - a).dot(n).abs()
            })
            .sum();
        return Ok((area, 0.0));
    }
    Ok(of_mesh::projected_area(&mesh, n, 1500))
}

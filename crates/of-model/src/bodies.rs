//! Body identity and multibody operations. Aggregate geometry is a derived cache
//! for the Phase 1 operations and whole-part measurement/export.

mod split;

use std::collections::{BTreeSet, HashSet};

use of_geom::{BoolOp, DAffine3, DMat3, DVec3, Kind, Shape};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::naming::{Inputs, Naming};
use crate::{Body, FeatureId, Ref, State, resolve};

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct BodyId(pub String);

impl BodyId {
    pub fn creator(&self) -> Option<FeatureId> {
        self.0.strip_prefix('b')?.split(':').next()?.parse().ok()
    }
}

#[derive(Debug, Clone)]
pub struct SolidBody {
    pub id: BodyId,
    pub geometry: Body,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BodyDisplay {
    pub name: Option<String>,
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Combine {
    /// The first selected body is the target for subtraction.
    #[schemars(length(min = 2))]
    pub bodies: Vec<BodyId>,
    pub operation: BoolOp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MoveBody {
    #[schemars(length(min = 1))]
    pub bodies: Vec<BodyId>,
    /// XYZ, mm.
    #[serde(default)]
    #[schemars(with = "[f64; 3]")]
    pub translation: DVec3,
    /// Euler XYZ angles in radians, applied X then Y then Z about `origin`.
    #[serde(default)]
    #[schemars(with = "[f64; 3]")]
    pub rotation: DVec3,
    /// XYZ, mm.
    #[serde(default)]
    #[schemars(with = "[f64; 3]")]
    pub origin: DVec3,
    #[serde(default)]
    pub copy: bool,
    /// With `copy`: how many copies, each moved again from the one before.
    #[serde(default = "one_copy", skip_serializing_if = "is_one")]
    pub copies: u32,
    /// Move along an edge, axis or plane normal instead of `translation`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub along: Option<Along>,
    /// Move so the first point or vertex lands on the second, instead of
    /// `translation`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_point: Option<[Ref; 2]>,
    /// Rotate about an edge or axis instead of `rotation`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<About>,
    /// Then place the bodies by mates between their geometry and other
    /// geometry, as in an assembly.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mates: Vec<BodyMate>,
}

fn one_copy() -> u32 {
    1
}

impl Default for MoveBody {
    fn default() -> Self {
        MoveBody {
            bodies: Vec::new(),
            translation: DVec3::ZERO,
            rotation: DVec3::ZERO,
            origin: DVec3::ZERO,
            copy: false,
            copies: 1,
            along: None,
            to_point: None,
            about: None,
            mates: Vec::new(),
        }
    }
}

fn is_one(n: &u32) -> bool {
    *n == 1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Along {
    pub reference: Ref,
    /// mm.
    pub distance: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct About {
    pub axis: Ref,
    /// Radians.
    pub angle: f64,
}

/// How a mate relates the moved geometry to the fixed geometry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BodyMateKind {
    Coincident,
    Parallel,
    Perpendicular,
    Tangent,
    Concentric,
    Distance { distance: f64 },
    Angle { angle: f64 },
}

/// A mate between geometry on the moved bodies and other geometry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BodyMate {
    pub kind: BodyMateKind,
    /// A face, edge, vertex or reference on a moved body.
    pub moving: Ref,
    /// A face, edge, vertex or reference that stays put.
    pub fixed: Ref,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KeepSide {
    #[default]
    Both,
    Positive,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SplitBody {
    #[schemars(length(min = 1))]
    pub bodies: Vec<BodyId>,
    /// A plane or planar face.
    #[serde(default = "default_split_plane")]
    pub plane: Ref,
    #[serde(default)]
    pub keep: KeepSide,
    /// General split mode: planes, bounded faces or whole tool bodies. Empty
    /// preserves the legacy one-piece-per-plane-side behavior above.
    #[serde(default)]
    pub tools: Vec<Ref>,
    /// Stable piece IDs from an all-pieces preview. None keeps every piece.
    /// Missing or ambiguous IDs fail instead of selecting another piece.
    #[serde(default)]
    pub retained_pieces: Option<Vec<BodyId>>,
}

fn default_split_plane() -> Ref {
    Ref::FrontPlane
}

fn identity_names(body: &Body) -> BTreeSet<String> {
    body.names
        .faces
        .iter()
        .flatten()
        .map(|name| {
            // A split face retains its ancestry, irrespective of its piece suffix.
            name.rsplit_once('~')
                .filter(|(_, suffix)| {
                    !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit())
                })
                .map_or(name.as_str(), |(base, _)| base)
                .to_owned()
        })
        .collect()
}

fn signature(body: &Body) -> String {
    // Specified FNV-1a, not a process-randomized hash or a kernel solid index.
    let mut hash = 0xcbf29ce484222325_u64;
    let names: BTreeSet<_> = body.names.faces.iter().map(|f| &f[0]).collect();
    for name in names {
        for byte in name.bytes().chain(std::iter::once(0)) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    format!("{hash:016x}")
}

pub(crate) fn partition(body: &Body) -> Result<Vec<Body>, String> {
    let mut result = Vec::new();
    for shape in body.shape.solids() {
        let indices = shape.indices_in(&body.shape, Kind::Face).map_err(|e| e.0)?;
        let faces = indices
            .iter()
            .map(|i| body.names.faces[*i as usize].clone())
            .collect();
        let names = Naming::from_faces(&shape, faces);
        result.push(Body { shape, names });
    }
    Ok(result)
}

impl State {
    pub fn qualified_ref_for(&self, kind: Kind, index: u32) -> Option<Ref> {
        let whole = &self.everything()?.shape;
        for body in self.bodies.iter().chain(&self.surfaces) {
            let indices = body.geometry.shape.indices_in(whole, kind).ok()?;
            if let Some(local) = indices.iter().position(|i| *i == index) {
                let names = &body.geometry.names;
                let name = match kind {
                    Kind::Face => names.face_name(local as u32)?.to_owned(),
                    Kind::Edge => names.edges.get(local)?.clone(),
                    Kind::Vertex => names.vertices.get(local)?.clone(),
                    _ => return None,
                };
                return Some(Ref::BodyTopology {
                    body: body.id.clone(),
                    kind,
                    name,
                });
            }
        }
        self.ref_for(kind, index)
    }

    pub fn solid(&self, id: &BodyId) -> Result<&SolidBody, String> {
        if let Some(b) = self.bodies.iter().find(|b| &b.id == id) {
            return Ok(b);
        }
        if self.surfaces.iter().any(|b| &b.id == id) {
            return Err(format!(
                "body {} is a surface body; select a solid body",
                id.0
            ));
        }
        Err(format!("body {} no longer exists", id.0))
    }

    /// A surface body by ID.
    pub fn surface(&self, id: &BodyId) -> Result<&SolidBody, String> {
        if let Some(b) = self.surfaces.iter().find(|b| &b.id == id) {
            return Ok(b);
        }
        if self.bodies.iter().any(|b| &b.id == id) {
            return Err(format!(
                "body {} is a solid body; select a surface body",
                id.0
            ));
        }
        Err(format!("body {} no longer exists", id.0))
    }

    /// A solid or surface body by ID, and whether it is a surface.
    pub fn any_body(&self, id: &BodyId) -> Result<(&SolidBody, bool), String> {
        if let Some(b) = self.bodies.iter().find(|b| &b.id == id) {
            return Ok((b, false));
        }
        self.surfaces
            .iter()
            .find(|b| &b.id == id)
            .map(|b| (b, true))
            .ok_or_else(|| format!("body {} no longer exists", id.0))
    }

    /// Every body, solids then surfaces, in one compound: what references
    /// resolve against and what is shown and exported. Solid sub-shapes have
    /// the same indices as in [`State::body`].
    pub fn everything(&self) -> Option<&Body> {
        self.all.as_ref().or(self.body.as_ref())
    }

    /// A face of [`State::everything`] by index, as its own shape.
    pub fn face_shape(&self, index: u32) -> Result<Shape, String> {
        self.everything()
            .ok_or("there's no body")?
            .shape
            .sub_shape(Kind::Face, index)
            .map_err(|e| e.0)
    }

    /// The surface body a face of [`State::everything`] belongs to, if any.
    pub fn surface_of_face(&self, index: u32) -> Option<usize> {
        let whole = &self.everything()?.shape;
        self.surfaces.iter().position(|b| {
            b.geometry
                .shape
                .indices_in(whole, Kind::Face)
                .is_ok_and(|ix| ix.contains(&index))
        })
    }

    /// Rebuild [`State::everything`] after the solids or surfaces changed.
    pub(crate) fn aggregate_all(&mut self) -> Result<(), String> {
        if self.surfaces.is_empty() {
            self.all = None;
            return Ok(());
        }
        let shapes = self
            .body
            .iter()
            .map(|b| &b.shape)
            .chain(self.surfaces.iter().map(|b| &b.geometry.shape));
        let shape = Shape::compound(shapes);
        let mut faces = vec![Vec::new(); shape.count(Kind::Face) as usize];
        let parts = self
            .body
            .iter()
            .chain(self.surfaces.iter().map(|b| &b.geometry));
        for part in parts {
            for (local, global) in part
                .shape
                .indices_in(&shape, Kind::Face)
                .map_err(|e| e.0)?
                .into_iter()
                .enumerate()
            {
                for name in &part.names.faces[local] {
                    if !faces[global as usize].contains(name) {
                        faces[global as usize].push(name.clone());
                    }
                }
            }
        }
        let names = Naming::from_faces(&shape, faces);
        self.all = Some(Body { shape, names });
        Ok(())
    }

    /// Reconcile legacy operations using inherited face provenance. Ambiguous
    /// matches create new identities; they never silently reuse an old body ID.
    pub(crate) fn refresh_bodies(&mut self, fid: FeatureId, prev: &State) -> Result<(), String> {
        let Some(aggregate) = &self.body else {
            self.bodies.clear();
            return Ok(());
        };
        let parts = partition(aggregate)?;
        let old_names: Vec<_> = prev
            .bodies
            .iter()
            .map(|b| identity_names(&b.geometry))
            .collect();
        let matches: Vec<Vec<usize>> = parts
            .iter()
            .map(|b| {
                let names = identity_names(b);
                old_names
                    .iter()
                    .enumerate()
                    .filter_map(|(i, old)| (!old.is_disjoint(&names)).then_some(i))
                    .collect()
            })
            .collect();
        let mut used = HashSet::new();
        self.bodies = parts
            .into_iter()
            .enumerate()
            .map(|(i, geometry)| {
                let id = if matches[i].len() == 1
                    && matches
                        .iter()
                        .filter(|m| m.contains(&matches[i][0]))
                        .count()
                        == 1
                {
                    prev.bodies[matches[i][0]].id.clone()
                } else {
                    BodyId(format!("b{fid}:{}", signature(&geometry)))
                };
                if !used.insert(id.clone()) {
                    return Err("body identity is ambiguous after this operation".into());
                }
                Ok(SolidBody { id, geometry })
            })
            .collect::<Result<_, String>>()?;
        Ok(())
    }

    /// Rebuild the aggregate cache using topological identity, including solids
    /// sharing boundary faces. Never concatenate face arrays by assumed order.
    pub(crate) fn aggregate_bodies(&mut self) -> Result<(), String> {
        if self.bodies.is_empty() {
            self.body = None;
            return Ok(());
        }
        let shape = Shape::compound(self.bodies.iter().map(|b| &b.geometry.shape));
        let mut faces = vec![Vec::new(); shape.count(Kind::Face) as usize];
        for body in &self.bodies {
            for (local, global) in body
                .geometry
                .shape
                .indices_in(&shape, Kind::Face)
                .map_err(|e| e.0)?
                .into_iter()
                .enumerate()
            {
                for name in &body.geometry.names.faces[local] {
                    if !faces[global as usize].contains(name) {
                        faces[global as usize].push(name.clone());
                    }
                }
            }
        }
        let names = Naming::from_faces(&shape, faces);
        self.body = Some(Body { shape, names });
        Ok(())
    }
}

fn selected<'a>(
    state: &'a State,
    ids: &[BodyId],
    min: usize,
) -> Result<Vec<&'a SolidBody>, String> {
    if ids.len() < min {
        return Err(format!("select at least {min} bodies"));
    }
    let unique: HashSet<_> = ids.iter().collect();
    if unique.len() != ids.len() {
        return Err("a body was selected more than once".into());
    }
    ids.iter().map(|id| state.solid(id)).collect()
}

fn boolean(a: &Body, b: &Body, operation: BoolOp, fid: FeatureId) -> Result<Body, String> {
    let result = a.shape.boolean(operation, &b.shape).map_err(|e| e.0)?;
    if result.shape.count(Kind::Solid) == 0 {
        return Err("the operation leaves no solid material".into());
    }
    let names = Naming::from_history(
        &result.shape,
        &result.history,
        Inputs {
            named: [Some(&a.names), Some(&b.names)],
        },
        &mut |_| None,
        &fid.to_string(),
    );
    Ok(Body {
        shape: result.shape,
        names,
    })
}

pub(crate) fn combine(
    fid: FeatureId,
    p: &Combine,
    prev: &State,
    state: &mut State,
) -> Result<(), String> {
    let inputs = selected(prev, &p.bodies, 2)?;
    let mut geometry = inputs[0].geometry.clone();
    for tool in &inputs[1..] {
        geometry = boolean(&geometry, &tool.geometry, p.operation, fid)?;
    }
    let parts = partition(&geometry)?;
    if p.operation == BoolOp::Fuse && parts.len() > 1 {
        return Err("the bodies don't touch; adding them would leave separate bodies".into());
    }
    state.bodies.retain(|b| !p.bodies.contains(&b.id));
    for geometry in parts {
        state.bodies.push(SolidBody {
            id: BodyId(format!("b{fid}:{}", signature(&geometry))),
            geometry,
        });
    }
    state.aggregate_bodies()
}

/// Geometry a mate can relate.
#[derive(Debug, Clone, Copy)]
enum MateGeo {
    Plane { p: DVec3, n: DVec3 },
    Axis { o: DVec3, d: DVec3, r: f64 },
    Point(DVec3),
}

fn mate_geo(state: &State, r: &Ref) -> Result<MateGeo, String> {
    let res = resolve(state, r)?;
    if let crate::Resolved::Face { info, .. } = &res {
        return Ok(match info.kind {
            of_geom::SurfaceKind::Plane => MateGeo::Plane {
                p: info.point,
                n: info.normal.normalize(),
            },
            of_geom::SurfaceKind::Cylinder | of_geom::SurfaceKind::Cone => MateGeo::Axis {
                o: info.axis_origin,
                d: info.axis_dir.normalize(),
                r: info.radius,
            },
            _ => return Err("mate flat faces, cylinders, edges, axes or points".into()),
        });
    }
    if let Ok(plane) = res.plane() {
        return Ok(MateGeo::Plane {
            p: plane.origin,
            n: plane.normal,
        });
    }
    if let Ok((o, d)) = res.axis() {
        let r = match &res {
            crate::Resolved::Edge { info, .. } if info.kind == of_geom::CurveKind::Circle => {
                info.radius
            }
            _ => 0.0,
        };
        return Ok(MateGeo::Axis { o, d, r });
    }
    Ok(MateGeo::Point(res.point()?))
}

fn moved_geo(g: MateGeo, t: &DAffine3) -> MateGeo {
    match g {
        MateGeo::Plane { p, n } => MateGeo::Plane {
            p: t.transform_point3(p),
            n: t.transform_vector3(n),
        },
        MateGeo::Axis { o, d, r } => MateGeo::Axis {
            o: t.transform_point3(o),
            d: t.transform_vector3(d),
            r,
        },
        MateGeo::Point(p) => MateGeo::Point(t.transform_point3(p)),
    }
}

fn mate_residuals(
    kind: &BodyMateKind,
    m: MateGeo,
    f: MateGeo,
    out: &mut Vec<f64>,
) -> Result<(), String> {
    use MateGeo::*;
    let perp = |v: DVec3, d: DVec3| v - d * v.dot(d);
    match (kind, m, f) {
        (BodyMateKind::Coincident, Plane { p: pm, n: nm }, Plane { p: pf, n: nf }) => {
            out.extend((nm + nf).to_array());
            out.push((pm - pf).dot(nf));
        }
        (BodyMateKind::Coincident, Point(q), Plane { p, n })
        | (BodyMateKind::Coincident, Plane { p, n }, Point(q)) => out.push((q - p).dot(n)),
        (BodyMateKind::Coincident, Point(a), Point(b)) => out.extend((a - b).to_array()),
        (
            BodyMateKind::Coincident | BodyMateKind::Concentric,
            Axis { o: om, d: dm, .. },
            Axis { o: of, d: df, .. },
        ) => {
            out.extend(dm.cross(df).to_array());
            out.extend(perp(om - of, df).to_array());
        }
        (BodyMateKind::Coincident | BodyMateKind::Concentric, Point(q), Axis { o, d, .. })
        | (BodyMateKind::Coincident | BodyMateKind::Concentric, Axis { o, d, .. }, Point(q)) => {
            out.extend(perp(q - o, d).to_array())
        }
        (BodyMateKind::Parallel, Plane { n: a, .. }, Plane { n: b, .. })
        | (BodyMateKind::Parallel, Axis { d: a, .. }, Axis { d: b, .. }) => {
            out.extend(a.cross(b).to_array())
        }
        (BodyMateKind::Parallel, Axis { d, .. }, Plane { n, .. })
        | (BodyMateKind::Parallel, Plane { n, .. }, Axis { d, .. }) => out.push(d.dot(n)),
        (BodyMateKind::Perpendicular, Plane { n: a, .. }, Plane { n: b, .. })
        | (BodyMateKind::Perpendicular, Axis { d: a, .. }, Axis { d: b, .. }) => out.push(a.dot(b)),
        (BodyMateKind::Perpendicular, Axis { d, .. }, Plane { n, .. })
        | (BodyMateKind::Perpendicular, Plane { n, .. }, Axis { d, .. }) => {
            out.extend(d.cross(n).to_array())
        }
        (BodyMateKind::Distance { distance }, Plane { p: pm, n: nm }, Plane { p: pf, n: nf }) => {
            out.extend(nm.cross(nf).to_array());
            out.push((pm - pf).dot(nf).abs() - distance);
        }
        (BodyMateKind::Distance { distance }, Point(a), Point(b)) => {
            out.push((a - b).length() - distance)
        }
        (BodyMateKind::Distance { distance }, Point(q), Plane { p, n })
        | (BodyMateKind::Distance { distance }, Plane { p, n }, Point(q)) => {
            out.push((q - p).dot(n).abs() - distance)
        }
        (BodyMateKind::Angle { angle }, Plane { n: a, .. }, Plane { n: b, .. })
        | (BodyMateKind::Angle { angle }, Axis { d: a, .. }, Axis { d: b, .. }) => {
            out.push(a.dot(b).clamp(-1.0, 1.0).acos() - angle)
        }
        (BodyMateKind::Tangent, Axis { o, d, r }, Plane { p, n })
        | (BodyMateKind::Tangent, Plane { p, n }, Axis { o, d, r }) => {
            out.push(d.dot(n));
            out.push((o - p).dot(n).abs() - r);
        }
        _ => return Err("this mate doesn't apply to the selected geometry".into()),
    }
    Ok(())
}

/// The motion that satisfies `mates`, starting from `start`, turning about
/// `pivot`.
fn solve_mates(
    state: &State,
    mates: &[BodyMate],
    start: DAffine3,
    pivot: DVec3,
) -> Result<DAffine3, String> {
    let pairs: Vec<(BodyMateKind, MateGeo, MateGeo)> = mates
        .iter()
        .map(|m| {
            Ok((
                m.kind.clone(),
                mate_geo(state, &m.moving)?,
                mate_geo(state, &m.fixed)?,
            ))
        })
        .collect::<Result<_, String>>()?;
    let motion = |q: &[f64; 6]| {
        let rv = DVec3::new(q[0], q[1], q[2]);
        let rot = if rv.length() > 1e-15 {
            DMat3::from_axis_angle(rv.normalize(), rv.length())
        } else {
            DMat3::IDENTITY
        };
        let turn = DAffine3::from_translation(pivot)
            * DAffine3::from_mat3(rot)
            * DAffine3::from_translation(-pivot);
        DAffine3::from_translation(DVec3::new(q[3], q[4], q[5])) * turn * start
    };
    let residual = |q: &[f64; 6]| -> Result<Vec<f64>, String> {
        let t = motion(q);
        let mut out = Vec::new();
        for (kind, m, f) in &pairs {
            mate_residuals(kind, moved_geo(*m, &t), *f, &mut out)?;
        }
        Ok(out)
    };
    let mut q = [0.0; 6];
    let mut lambda: f64 = 1e-3;
    for _ in 0..100 {
        let r0 = residual(&q)?;
        let e0: f64 = r0.iter().map(|x| x * x).sum();
        if e0.sqrt() < 1e-10 {
            break;
        }
        let mut j = nalgebra::DMatrix::zeros(r0.len(), 6);
        for k in 0..6 {
            let mut qk = q;
            let h = 1e-7;
            qk[k] += h;
            let r1 = residual(&qk)?;
            for i in 0..r0.len() {
                j[(i, k)] = (r1[i] - r0[i]) / h;
            }
        }
        let jt = j.transpose();
        let a = &jt * &j + nalgebra::DMatrix::identity(6, 6) * lambda;
        let g = &jt * nalgebra::DVector::from_vec(r0.clone());
        let Some(step) = a.lu().solve(&(-g)) else {
            break;
        };
        let mut trial = q;
        for k in 0..6 {
            trial[k] += step[k];
        }
        let e1: f64 = residual(&trial)?.iter().map(|x| x * x).sum();
        if e1 < e0 {
            q = trial;
            lambda = (lambda * 0.3).max(1e-12);
        } else {
            lambda *= 10.0;
        }
    }
    let e: f64 = residual(&q)?.iter().map(|x| x * x).sum();
    if e.sqrt() > 1e-6 {
        return Err("the mates can't all be satisfied".into());
    }
    Ok(motion(&q))
}

/// The rigid motion a Move/Copy Body describes.
fn motion_of(p: &MoveBody, prev: &State, inputs: &[&SolidBody]) -> Result<DAffine3, String> {
    let rotation = match &p.about {
        Some(a) => {
            let (o, d) = resolve(prev, &a.axis)?.axis()?;
            DAffine3::from_translation(o)
                * DAffine3::from_axis_angle(d.normalize(), a.angle)
                * DAffine3::from_translation(-o)
        }
        None => {
            let r = DMat3::from_rotation_z(p.rotation.z)
                * DMat3::from_rotation_y(p.rotation.y)
                * DMat3::from_rotation_x(p.rotation.x);
            DAffine3::from_mat3_translation(r, p.origin - r * p.origin)
        }
    };
    let translation = match (&p.along, &p.to_point) {
        (Some(_), Some(_)) => return Err("move along a direction or to a point, not both".into()),
        (Some(a), None) => resolve(prev, &a.reference)?.direction()?.normalize() * a.distance,
        (None, Some([from, to])) => {
            let a = rotation.transform_point3(resolve(prev, from)?.point()?);
            resolve(prev, to)?.point()? - a
        }
        (None, None) => p.translation,
    };
    let start = DAffine3::from_translation(translation) * rotation;
    if p.mates.is_empty() {
        return Ok(start);
    }
    let pivot = Shape::compound(inputs.iter().map(|b| &b.geometry.shape))
        .bbox()
        .map_or(DVec3::ZERO, |b| b.center());
    solve_mates(prev, &p.mates, start, start.transform_point3(pivot))
}

pub(crate) fn move_body(
    fid: FeatureId,
    p: &MoveBody,
    prev: &State,
    state: &mut State,
) -> Result<(), String> {
    if !p.translation.is_finite() || !p.rotation.is_finite() || !p.origin.is_finite() {
        return Err("translation, rotation and origin must be finite".into());
    }
    if p.copies == 0 || (!p.copy && p.copies != 1) {
        return Err("make at least one copy, and only when copying".into());
    }
    let inputs = selected(prev, &p.bodies, 1)?;
    let transform = motion_of(p, prev, &inputs)?;
    for input in inputs {
        let mut step = transform;
        for n in 1..=p.copies {
            let result = input.geometry.shape.transform(&step).map_err(|e| e.0)?;
            let suffix = if p.copies > 1 {
                format!("{n}.")
            } else {
                String::new()
            };
            let names = if p.copy {
                input.geometry.names.prefixed(
                    &result.shape,
                    &result.history,
                    &format!("{fid}.copy({}).{suffix}", input.id.0),
                )
            } else {
                Naming::from_history(
                    &result.shape,
                    &result.history,
                    Inputs {
                        named: [Some(&input.geometry.names), None],
                    },
                    &mut |_| None,
                    &fid.to_string(),
                )
            };
            let id = match (p.copy, p.copies > 1) {
                (true, true) => BodyId(format!("b{fid}:copy{n}:{}", input.id.0)),
                (true, false) => BodyId(format!("b{fid}:copy:{}", input.id.0)),
                (false, _) => input.id.clone(),
            };
            let body = SolidBody {
                id,
                geometry: Body {
                    shape: result.shape,
                    names,
                },
            };
            if p.copy {
                state.bodies.push(body);
            } else {
                *state.bodies.iter_mut().find(|b| b.id == input.id).unwrap() = body;
            }
            step = transform * step;
        }
    }
    state.aggregate_bodies()
}

pub(crate) fn split_body(
    fid: FeatureId,
    p: &SplitBody,
    prev: &State,
    state: &mut State,
) -> Result<(), String> {
    if !p.tools.is_empty() {
        return split::general_split(fid, p, prev, state);
    }
    if p.retained_pieces.is_some() {
        return Err(
            "piece selection requires general splitting tools; use a plane as a tool".into(),
        );
    }
    let inputs = selected(prev, &p.bodies, 1)?;
    let plane = resolve(prev, &p.plane)?.plane()?;
    state.bodies.retain(|b| !p.bodies.contains(&b.id));
    for input in inputs {
        let result = input.geometry.shape.split_plane(&plane).map_err(|e| e.0)?;
        let names = Naming::from_history(
            &result.shape,
            &result.history,
            Inputs {
                named: [Some(&input.geometry.names), None],
            },
            &mut |_| Some(format!("{fid}.split({})", input.id.0)),
            &fid.to_string(),
        );
        let parts = partition(&Body {
            shape: result.shape,
            names,
        })?;
        if parts.len() < 2 {
            return Err("the plane does not split the selected body".into());
        }
        let mut sides = HashSet::new();
        for geometry in parts {
            let center = geometry.shape.mass_props().map_err(|e| e.0)?.center;
            let positive = (center - plane.origin).dot(plane.normal) > 0.0;
            if !sides.insert(positive) {
                return Err(
                    "this split creates multiple pieces on one side; use additional split features"
                        .into(),
                );
            }
            if (p.keep == KeepSide::Positive && !positive)
                || (p.keep == KeepSide::Negative && positive)
            {
                continue;
            }
            state.bodies.push(SolidBody {
                id: BodyId(format!(
                    "b{fid}:{}:{}",
                    input.id.0,
                    if positive { "positive" } else { "negative" }
                )),
                geometry,
            });
        }
    }
    state.aggregate_bodies()
}

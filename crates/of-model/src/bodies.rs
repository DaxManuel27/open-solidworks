//! Body identity and multibody operations. Aggregate geometry is a derived cache
//! for the Phase 1 operations and whole-part measurement/export.

use std::collections::{BTreeSet, HashSet};

use of_geom::{BoolOp, DAffine3, DMat3, DVec3, Kind, Shape};
use serde::{Deserialize, Serialize};

use crate::naming::{Inputs, Naming};
use crate::{Body, FeatureId, Ref, State, resolve};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Combine {
    /// The first selected body is the target for subtraction.
    pub bodies: Vec<BodyId>,
    pub operation: BoolOp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoveBody {
    pub bodies: Vec<BodyId>,
    #[serde(default)]
    pub translation: DVec3,
    /// Euler XYZ angles in radians, applied X then Y then Z about `origin`.
    #[serde(default)]
    pub rotation: DVec3,
    #[serde(default)]
    pub origin: DVec3,
    #[serde(default)]
    pub copy: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeepSide {
    #[default]
    Both,
    Positive,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SplitBody {
    pub bodies: Vec<BodyId>,
    pub plane: Ref,
    #[serde(default)]
    pub keep: KeepSide,
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
        let whole = &self.body.as_ref()?.shape;
        for body in &self.bodies {
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
        self.bodies
            .iter()
            .find(|b| &b.id == id)
            .ok_or_else(|| format!("body {} no longer exists", id.0))
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
    state.bodies.retain(|b| !p.bodies.contains(&b.id));
    for geometry in parts {
        state.bodies.push(SolidBody {
            id: BodyId(format!("b{fid}:{}", signature(&geometry))),
            geometry,
        });
    }
    state.aggregate_bodies()
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
    let inputs = selected(prev, &p.bodies, 1)?;
    let r = DMat3::from_rotation_z(p.rotation.z)
        * DMat3::from_rotation_y(p.rotation.y)
        * DMat3::from_rotation_x(p.rotation.x);
    let transform = DAffine3::from_mat3_translation(r, p.translation + p.origin - r * p.origin);
    for input in inputs {
        let result = input
            .geometry
            .shape
            .transform(&transform)
            .map_err(|e| e.0)?;
        let names = if p.copy {
            input.geometry.names.prefixed(
                &result.shape,
                &result.history,
                &format!("{fid}.copy({}).", input.id.0),
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
        let body = SolidBody {
            id: if p.copy {
                BodyId(format!("b{fid}:copy:{}", input.id.0))
            } else {
                input.id.clone()
            },
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
    }
    state.aggregate_bodies()
}

pub(crate) fn split_body(
    fid: FeatureId,
    p: &SplitBody,
    prev: &State,
    state: &mut State,
) -> Result<(), String> {
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

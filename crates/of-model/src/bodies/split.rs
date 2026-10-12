//! General splitting. Piece identities use oriented source/tool face ancestry,
//! never an OCCT solid ordinal or a geometric nearest-neighbour match.

use std::collections::{BTreeSet, HashSet};

use of_geom::{Kind, Shape};

use super::{BodyId, KeepSide, SolidBody, SplitBody, selected};
use crate::naming::{Inputs, Naming};
use crate::{Body, FeatureId, Ref, Resolved, State, resolve};

// Naming's numeric split suffixes are positional, so they cannot participate in
// persistent piece identity. Indistinguishable ancestry is explicitly ambiguous.
fn ancestry(mut name: &str) -> &str {
    while let Some((base, suffix)) = name.rsplit_once('~') {
        if suffix.is_empty() || !suffix.bytes().all(|c| c.is_ascii_digit()) {
            break;
        }
        name = base;
    }
    name
}

fn hash_names(names: &BTreeSet<String>) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for name in names {
        for byte in name.bytes().chain(std::iter::once(0)) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    format!("{hash:016x}")
}

fn cutting_tool(state: &State, reference: &Ref, targets: &[BodyId]) -> Result<Body, String> {
    let (shape, names) = match reference {
        Ref::Body { id } => {
            if targets.contains(id) {
                return Err("a splitting tool cannot also be a target body".into());
            }
            let body = &state.solid(id)?.geometry;
            let names = body
                .names
                .faces
                .iter()
                .map(|face| vec![format!("body({}).{}", id.0, ancestry(&face[0]))])
                .collect();
            (body.shape.clone(), names)
        }
        Ref::Face { name } => {
            let aggregate = state.body.as_ref().ok_or("there is no body")?;
            let exact: Vec<_> = aggregate
                .names
                .faces
                .iter()
                .enumerate()
                .filter(|(_, names)| names.first() == Some(name))
                .collect();
            if exact.len() != 1 {
                return Err(format!("splitting face {name} is missing or ambiguous"));
            }
            let face = aggregate
                .shape
                .sub_shape(Kind::Face, exact[0].0 as u32)
                .map_err(|e| e.0)?;
            (face, vec![vec![format!("face({})", ancestry(name))]])
        }
        Ref::BodyTopology {
            body,
            kind: Kind::Face,
            name,
        } => {
            if targets.contains(body) {
                return Err("a splitting face cannot belong to a target body".into());
            }
            let geometry = &state.solid(body)?.geometry;
            let local = geometry
                .names
                .faces
                .iter()
                .position(|f| f.first() == Some(name))
                .ok_or_else(|| format!("splitting face {name} no longer exists"))?;
            (
                geometry
                    .shape
                    .sub_shape(Kind::Face, local as u32)
                    .map_err(|e| e.0)?,
                vec![vec![format!("body({}).{}", body.0, ancestry(name))]],
            )
        }
        _ => {
            let resolved = resolve(state, reference)?;
            if !matches!(resolved, Resolved::Plane(_) | Resolved::CoordSys { .. }) {
                return Err(
                    "select a plane, bounded face or whole body as a splitting tool".into(),
                );
            }
            let key = serde_json::to_string(reference).map_err(|e| e.to_string())?;
            (
                Shape::plane_face(&resolved.plane()?).map_err(|e| e.0)?,
                vec![vec![format!("plane({key})")]],
            )
        }
    };
    // Do not use Naming::from_faces here: its positional uniqueness suffixes are
    // inappropriate for identity. The original face tokens stay alongside it.
    Ok(Body {
        names: Naming::from_faces(&shape, names),
        shape,
    })
}

pub(super) fn general_split(
    fid: FeatureId,
    params: &SplitBody,
    previous: &State,
    state: &mut State,
) -> Result<(), String> {
    if params.keep != KeepSide::Both {
        return Err("positive/negative side selection applies only to legacy plane splits; select retained pieces for general tools".into());
    }
    let inputs = selected(previous, &params.bodies, 1)?;
    if params.tools.iter().collect::<HashSet<_>>().len() != params.tools.len() {
        return Err("a splitting tool was selected more than once".into());
    }
    let tools = params
        .tools
        .iter()
        .map(|r| cutting_tool(previous, r, &params.bodies))
        .collect::<Result<Vec<_>, _>>()?;
    let tool_shape = Shape::compound(tools.iter().map(|t| &t.shape));
    let mut tool_names = vec![Vec::new(); tool_shape.count(Kind::Face) as usize];
    for tool in &tools {
        for (local, global) in tool
            .shape
            .indices_in(&tool_shape, Kind::Face)
            .map_err(|e| e.0)?
            .into_iter()
            .enumerate()
        {
            tool_names[global as usize].extend(tool.names.faces[local].iter().cloned());
        }
    }
    let named_tools = Naming::from_faces(&tool_shape, tool_names.clone());
    let mut pieces = Vec::new();
    let mut identities = HashSet::new();
    for input in inputs {
        let result = input
            .geometry
            .shape
            .split_tools(&tool_shape)
            .map_err(|e| e.0)?;
        let solids = result.shape.solids();
        if solids.len() < 2 {
            return Err(format!(
                "splitting tools do not divide body {} into multiple pieces",
                input.id.0
            ));
        }
        let names = Naming::from_history(
            &result.shape,
            &result.history,
            Inputs {
                named: [Some(&input.geometry.names), Some(&named_tools)],
            },
            &mut |_| None,
            &format!("{fid}.split"),
        );
        for shape in solids {
            let indices = shape
                .indices_in(&result.shape, Kind::Face)
                .map_err(|e| e.0)?;
            let mut provenance = BTreeSet::new();
            let mut faces = Vec::new();
            for (local, global) in indices.into_iter().enumerate() {
                let mut found = false;
                for entry in result.history.iter().filter(|h| {
                    h.to_kind == Kind::Face && h.to == global && h.from_kind == Kind::Face
                }) {
                    found = true;
                    if entry.input == 0 {
                        provenance.insert(format!(
                            "source:{}",
                            ancestry(&input.geometry.names.faces[entry.from as usize][0])
                        ));
                    } else {
                        let orientation = shape.face_orientation(local as u32).map_err(|e| e.0)?
                            * tool_shape.face_orientation(entry.from).map_err(|e| e.0)?;
                        for name in &tool_names[entry.from as usize] {
                            provenance.insert(format!("tool:{}:{orientation}", ancestry(name)));
                        }
                    }
                }
                if !found {
                    return Err(
                        "split piece identity is unresolved: kernel omitted face provenance".into(),
                    );
                }
                faces.push(names.faces[global as usize].clone());
            }
            let id = BodyId(format!(
                "b{fid}:piece:{}:{}",
                input.id.0,
                hash_names(&provenance)
            ));
            if !identities.insert(id.clone()) {
                return Err("split piece identity is ambiguous: multiple pieces share the same oriented face provenance; use distinct splitting tools".into());
            }
            let names = Naming::from_faces(&shape, faces);
            pieces.push(SolidBody {
                id,
                geometry: Body { shape, names },
            });
        }
    }
    if let Some(retained) = &params.retained_pieces {
        if retained.is_empty() {
            return Err("select at least one split piece to retain".into());
        }
        if retained.iter().collect::<HashSet<_>>().len() != retained.len() {
            return Err("a retained split piece was selected more than once".into());
        }
        for id in retained {
            if !identities.contains(id) {
                return Err(format!(
                    "retained split piece {} is unresolved after a tool or topology change; preview all pieces and select again",
                    id.0
                ));
            }
        }
        pieces.retain(|p| retained.contains(&p.id));
    }
    state.bodies.retain(|b| !params.bodies.contains(&b.id));
    state.bodies.extend(pieces);
    state.aggregate_bodies()
}

//! Phase 6 queries and commands: draft, undercut, deviation, projected area
//! and body comparison; saving bodies as parts; mirrored parts.

use std::path::{Path, PathBuf};

use of_model::body_tools::{InsertPart, InsertTransfer};
use of_model::geom::{DVec3, Kind, Shape, SurfaceKind};
use of_model::{BodyId, Document, FeatureKind, Model, Ref, State};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

/// What to analyse.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Analysis {
    /// Draft of every solid face against a pull direction (plane, face or edge).
    Draft {
        pull: Ref,
        #[serde(default)]
        reverse: bool,
        /// Required draft, radians.
        angle: f64,
    },
    /// Faces a mould opening along the pull direction would trap.
    Undercut {
        pull: Ref,
        #[serde(default)]
        reverse: bool,
    },
    /// Distances from a mesh body's vertices to a solid or surface body (or
    /// all of them).
    Deviation {
        mesh: BodyId,
        #[serde(default)]
        body: Option<BodyId>,
        /// Report the share of vertices within each distance, mm.
        #[serde(default)]
        limits: Vec<f64>,
    },
    /// Area of faces seen along a plane's normal; with `hidden`, overlaps
    /// count twice.
    ProjectedArea {
        faces: Vec<Ref>,
        plane: Ref,
        #[serde(default)]
        hidden: bool,
    },
    /// Which faces two bodies share, which changed, and how their volumes
    /// differ.
    Compare { a: BodyId, b: BodyId },
}

#[derive(Deserialize, JsonSchema)]
pub struct AnalysisParams {
    pub analysis: Analysis,
}

fn round(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

fn pull_of(state: &State, pull: &Ref, reverse: bool) -> Result<DVec3, String> {
    let d = of_model::resolve(state, pull)?.direction()?.normalize();
    Ok(if reverse { -d } else { d })
}

pub fn analysis(model: &Model, p: &AnalysisParams) -> Result<Value, String> {
    let state = model.state();
    let face_ref = |i: u32| state.ref_for(Kind::Face, i);
    match &p.analysis {
        Analysis::Draft {
            pull,
            reverse,
            angle,
        } => {
            let faces = of_model::analysis::draft(state, pull_of(state, pull, *reverse)?, *angle)?;
            let mut counts = std::collections::BTreeMap::<String, usize>::new();
            let list: Vec<Value> = faces
                .iter()
                .map(|f| {
                    let class = serde_json::to_value(f.class).unwrap();
                    *counts
                        .entry(class.as_str().unwrap().to_string())
                        .or_default() += 1;
                    json!({ "face": f.face, "ref": face_ref(f.face), "class": class,
                            "min": round(f.min), "max": round(f.max), "steep": f.steep })
                })
                .collect();
            Ok(json!({ "faces": list, "counts": counts }))
        }
        Analysis::Undercut { pull, reverse } => {
            let faces = of_model::analysis::undercut(state, pull_of(state, pull, *reverse)?)?;
            let list: Vec<Value> = faces
                .iter()
                .map(|f| json!({ "face": f.face, "ref": face_ref(f.face), "class": f.class }))
                .collect();
            Ok(json!({ "faces": list }))
        }
        Analysis::Deviation { mesh, body, limits } => {
            let m = &state.mesh(mesh)?.mesh;
            let shape = match body {
                Some(id) => state.any_body(id)?.0.geometry.shape.clone(),
                None => state
                    .everything()
                    .map(|b| b.shape.clone())
                    .ok_or("there's no body to measure against")?,
            };
            let d = of_model::analysis::deviation(m, &shape, limits)?;
            Ok(json!({
                "mean": round(d.mean), "rms": round(d.rms), "max": round(d.max),
                "within": d.within.iter().map(|(l, f)| json!({ "distance": l, "fraction": round(*f) })).collect::<Vec<_>>(),
                "distances": d.distances.iter().map(|v| round(*v)).collect::<Vec<_>>(),
            }))
        }
        Analysis::ProjectedArea {
            faces,
            plane,
            hidden,
        } => {
            let normal = of_model::resolve(state, plane)?.plane()?.normal;
            let indices = faces
                .iter()
                .map(|r| match state.any_index_of(r)? {
                    (Kind::Face, i) => Ok(i),
                    _ => Err("select faces".to_string()),
                })
                .collect::<Result<Vec<_>, String>>()?;
            let (area, error) =
                of_model::analysis::projected_area(state, &indices, normal, *hidden)?;
            Ok(json!({ "area": round(area), "error": round(error) }))
        }
        Analysis::Compare { a, b } => compare(state, a, b),
    }
}

/// Faces of `b`'s whole-model indices, with their info.
fn face_list(state: &State, id: &BodyId) -> Result<(Shape, Vec<u32>), String> {
    let (body, _) = state.any_body(id)?;
    let whole = &state.everything().ok_or("there's no body")?.shape;
    let ix = body
        .geometry
        .shape
        .indices_in(whole, Kind::Face)
        .map_err(|e| e.0)?;
    Ok((body.geometry.shape.clone(), ix))
}

fn compare(state: &State, a: &BodyId, b: &BodyId) -> Result<Value, String> {
    let (sa, ia) = face_list(state, a)?;
    let (sb, ib) = face_list(state, b)?;
    let size = Shape::compound([&sa, &sb])
        .bbox()
        .map_or(1.0, |x| x.diagonal());
    let tol = size * 1e-7;
    let class_of = |s: &Shape, other: &Shape, f: u32| -> Result<&'static str, String> {
        let face = s.sub_shape(Kind::Face, f).map_err(|e| e.0)?;
        let info = s.face_info(f).map_err(|e| e.0)?;
        let same = (0..other.count(Kind::Face)).any(|g| {
            other.face_info(g).is_ok_and(|o| {
                o.kind == info.kind
                    && (o.area - info.area).abs() <= 1e-6 * info.area.max(1.0)
                    && (o.centroid - info.centroid).length() <= tol
            })
        });
        if same {
            return Ok("matching");
        }
        let on = other.faces_on(&face, tol).map_err(|e| e.0)?;
        let touching = !on.is_empty()
            || (0..other.count(Kind::Face)).any(|g| {
                other.face_info(g).is_ok_and(|o| {
                    o.kind == info.kind
                        && o.kind == SurfaceKind::Plane
                        && o.normal.cross(info.normal).length() < 1e-9
                        && (o.point - info.point).dot(info.normal).abs() < tol
                })
            });
        Ok(if touching { "modified" } else { "unique" })
    };
    let list = |s: &Shape, other: &Shape, ix: &[u32]| -> Result<Vec<Value>, String> {
        (0..s.count(Kind::Face))
            .map(|f| {
                let g = ix[f as usize];
                Ok(json!({ "face": g, "ref": state.ref_for(Kind::Face, g), "class": class_of(s, other, f)? }))
            })
            .collect()
    };
    let volume = |s: &Shape| s.mass_props().map(|m| m.volume).unwrap_or(0.0);
    let only = |x: &Shape, y: &Shape| {
        x.boolean(of_model::geom::BoolOp::Cut, y)
            .map(|r| volume(&r.shape))
            .unwrap_or(0.0)
    };
    Ok(json!({
        "a": list(&sa, &sb, &ia)?,
        "b": list(&sb, &sa, &ib)?,
        "only_a": round(only(&sa, &sb)),
        "only_b": round(only(&sb, &sa)),
    }))
}

#[derive(Deserialize, JsonSchema)]
pub struct SaveBodies {
    /// The folder the new part files go in, one per body, named after it.
    pub folder: PathBuf,
    pub bodies: Vec<BodyId>,
    /// Also write an assembly of this name placing the new parts.
    #[serde(default)]
    pub assembly: Option<String>,
}

/// A derived part file holding one body of `source`, linked to it.
fn derived(source: &Path, bodies: Vec<BodyId>, mirror: Option<Ref>) -> Document {
    let mut doc = Document::new();
    doc.add(FeatureKind::InsertPart(InsertPart {
        path: source.display().to_string(),
        configuration: None,
        transfer: InsertTransfer::default(),
        bodies,
        mirror,
        translation: [0.0; 3],
        rotation: [0.0; 3],
    }));
    doc
}

/// A file name for a body: its display name, without characters files
/// can't hold.
fn file_name(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_.".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let clean = clean.trim().trim_matches('.');
    if clean.is_empty() {
        "body".into()
    } else {
        clean.into()
    }
}

pub fn save_bodies(model: &Model, source: Option<&Path>, p: &SaveBodies) -> Result<Value, String> {
    let source = source.ok_or("save the part first: saved bodies stay linked to its file")?;
    if p.bodies.is_empty() {
        return Err("choose bodies to save".into());
    }
    std::fs::create_dir_all(&p.folder)
        .map_err(|e| format!("can't make {}: {e}", p.folder.display()))?;
    let state = model.state();
    let mut written = Vec::new();
    for id in &p.bodies {
        // The name the tree shows.
        let name = match state.any_body(id)? {
            (_, false) => {
                let i = state.bodies.iter().position(|b| &b.id == id).unwrap();
                crate::views::body_name(&model.doc, state, i)
            }
            (_, true) => {
                let i = state.surfaces.iter().position(|b| &b.id == id).unwrap();
                crate::views::surface_name(&model.doc, state, i)
            }
        };
        let mut path = p.folder.join(format!("{}.ofp", file_name(&name)));
        let mut n = 2;
        while written.contains(&path) {
            path = p.folder.join(format!("{} ({n}).ofp", file_name(&name)));
            n += 1;
        }
        let doc = derived(source, vec![id.clone()], None);
        let check = Model::new(doc.clone());
        if let Some(e) = check.statuses().into_iter().find_map(|s| s.error) {
            return Err(format!("{} can't be saved: {e}", id.0));
        }
        of_model::file::save(&doc, &path)?;
        written.push(path);
    }
    let mut assembly = None;
    if let Some(name) = &p.assembly {
        let path = p
            .folder
            .join(format!("{}.ofa", file_name(name.trim_end_matches(".ofa"))));
        let mut asm = of_assembly::Assembly::default();
        asm.save(&path)?;
        for part in &written {
            asm.insert(part, None, None)?;
        }
        asm.save(&path)?;
        assembly = Some(path);
    }
    Ok(json!({ "parts": written, "assembly": assembly }))
}

#[derive(Deserialize, JsonSchema)]
pub struct MirrorPart {
    /// The new, opposite-hand `.ofp` file.
    pub path: PathBuf,
    /// The plane to mirror in; defaults to the right plane.
    #[serde(default)]
    pub plane: Option<Ref>,
}

pub fn mirror_part(source: Option<&Path>, p: &MirrorPart) -> Result<Value, String> {
    let source = source.ok_or("save the part first: the mirrored part stays linked to its file")?;
    let doc = derived(
        source,
        vec![],
        Some(p.plane.clone().unwrap_or(Ref::RightPlane)),
    );
    let check = Model::new(doc.clone());
    if let Some(e) = check.statuses().into_iter().find_map(|s| s.error) {
        return Err(format!("the mirrored part doesn't build: {e}"));
    }
    of_model::file::save(&doc, &p.path)?;
    Ok(json!({ "path": p.path }))
}

//! Assembly export (STEP with product structure, or assembled geometry for
//! IGES and STL) and STEP assembly import.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use of_mate::Pose;
use of_model::geom::{STEP_ROOT, Shape, StepAssemblyWriter, read_step_assembly};
use of_model::{Document, FeatureKind, Import, ImportFormat};

use crate::analysis::world_shape;
use crate::assembly::{Assembly, Instance, Loaded};

fn definition_name(i: &Instance) -> String {
    let stem = i
        .file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("component")
        .to_string();
    match &i.loaded {
        Loaded::Part(p) if p.doc().configurations.items.len() > 1 => {
            format!("{stem} ({})", p.doc().configurations.active().name)
        }
        _ => stem,
    }
}

/// Write a STEP file in which each part is defined once and placed as named
/// instances, nested like the assembly's subassemblies.
pub fn export_step(asm: &mut Assembly, path: &Path) -> Result<(), String> {
    let instances = asm.instances();
    if !instances.iter().any(|i| i.part().is_some()) {
        return Err("the assembly has no parts to export".into());
    }
    let name = asm
        .path()
        .and_then(|p| p.file_stem())
        .and_then(|s| s.to_str())
        .unwrap_or("Assembly")
        .to_string();
    let mut w = StepAssemblyWriter::new(&name).map_err(|e| e.0)?;
    let mut parts: HashMap<(PathBuf, Option<u32>), u32> = HashMap::new();
    let mut nodes: HashMap<Vec<String>, (u32, Pose)> = HashMap::new();
    for i in &instances {
        let (parent, parent_pose) = match i.key.len() {
            1 => (STEP_ROOT, Pose::IDENTITY),
            n => match nodes.get(&i.key[..n - 1]) {
                Some(p) => *p,
                None => continue,
            },
        };
        let local = parent_pose.inverse().then(&i.pose);
        let label = match &i.loaded {
            Loaded::Part(p) => {
                let Some(shape) = p.shape() else { continue };
                let key = (crate::cache::canonical(&i.file), i.configuration);
                match parts.get(&key) {
                    Some(l) => *l,
                    None => {
                        let l = w.part(&definition_name(i), shape).map_err(|e| e.0)?;
                        parts.insert(key, l);
                        l
                    }
                }
            }
            Loaded::Assembly(_) => {
                let l = w.assembly(&definition_name(i)).map_err(|e| e.0)?;
                nodes.insert(i.key.clone(), (l, i.pose));
                l
            }
            Loaded::Missing(_) => continue,
        };
        w.place(parent, label, &i.name, &local.affine())
            .map_err(|e| e.0)?;
    }
    w.write(&path.to_string_lossy()).map_err(|e| e.0)
}

/// Every part instance's solid in the assembly frame, as one compound.
pub fn assembled_shape(asm: &mut Assembly) -> Result<Shape, String> {
    let shapes: Vec<Shape> = asm.instances().iter().filter_map(world_shape).collect();
    if shapes.is_empty() {
        return Err("the assembly has no parts to export".into());
    }
    Ok(Shape::compound(shapes.iter()))
}

/// How Defeature simplifies each component.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Silhouette {
    /// The box around each component.
    BoundingBox,
    /// The cylinder around each component, along its most distinct side.
    Cylinder,
}

/// Every visible component replaced by a simple solid around it, placed
/// where it is in the assembly, as a part document embedding them.
pub fn defeatured_part(asm: &mut Assembly, mode: Silhouette) -> Result<Document, String> {
    use base64::Engine;
    let shape = silhouette(asm, mode)?;
    let tmp =
        std::env::temp_dir().join(format!("openforge-silhouette-{}.step", std::process::id()));
    shape.write_step(&tmp.to_string_lossy()).map_err(|e| e.0)?;
    let bytes = std::fs::read(&tmp).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&tmp);
    let mut doc = Document::new();
    doc.add(FeatureKind::Import(Import {
        format: ImportFormat::Step,
        source: "silhouette.step".into(),
        data: base64::engine::general_purpose::STANDARD.encode(bytes),
        mesh: None,
    }));
    Ok(doc)
}

/// Every visible component replaced by a simple solid around it, placed
/// where it is in the assembly.
pub fn silhouette(asm: &mut Assembly, mode: Silhouette) -> Result<Shape, String> {
    use of_model::geom::{Curve2, DAffine3, DVec2, DVec3, Plane, Profile, ProfileFace};
    let mut out = Vec::new();
    for i in asm.instances().iter().filter(|i| !i.hidden) {
        let Some(shape) = world_shape(i) else {
            continue;
        };
        let Some(b) = shape.bbox() else { continue };
        let size = (b.max - b.min).max(DVec3::splat(1e-3));
        let solid = match mode {
            Silhouette::BoundingBox => Shape::make_box(size.x, size.y, size.z)
                .and_then(|s| s.transform(&DAffine3::from_translation(b.min)))
                .map(|r| r.shape),
            Silhouette::Cylinder => {
                // The axis is the side least like the other two.
                let d = [size.x, size.y, size.z];
                let axis = (0..3)
                    .max_by(|&a, &c| {
                        let odd = |k: usize| {
                            let (p, q) = (d[(k + 1) % 3], d[(k + 2) % 3]);
                            (d[k] - (p + q) / 2.0).abs() - (p - q).abs()
                        };
                        odd(a).total_cmp(&odd(c))
                    })
                    .unwrap();
                let dir = [DVec3::X, DVec3::Y, DVec3::Z][axis];
                let radius = 0.5 * (d[(axis + 1) % 3].powi(2) + d[(axis + 2) % 3].powi(2)).sqrt();
                let centre = b.center() - dir * (d[axis] / 2.0);
                let plane = Plane::new(centre, dir.any_orthonormal_vector(), dir);
                Profile {
                    plane,
                    faces: vec![ProfileFace {
                        outer: vec![Curve2::Circle {
                            center: DVec2::ZERO,
                            radius,
                        }],
                        holes: vec![],
                    }],
                }
                .to_shape()
                .and_then(|f| f.prism(dir * d[axis]))
                .map(|r| r.shape)
            }
        }
        .map_err(|e| e.0)?;
        out.push(solid);
    }
    if out.is_empty() {
        return Err("the assembly has no visible parts".into());
    }
    Ok(Shape::compound(&out))
}

fn safe_name(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || "-_ .".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').to_string();
    if trimmed.is_empty() {
        "part".into()
    } else {
        trimmed
    }
}

/// Read a STEP assembly, write one `.ofp` per distinct part into `dir` and
/// return an assembly (unsaved) placing every instance where the file had
/// it. Components start fixed so they keep those positions.
pub fn import_step(step: &Path, dir: &Path) -> Result<Assembly, String> {
    use base64::Engine;
    let nodes = read_step_assembly(&step.to_string_lossy()).map_err(|e| e.0)?;
    std::fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    let mut world: Vec<Pose> = Vec::with_capacity(nodes.len());
    for n in &nodes {
        let local = Pose::from_affine(&n.placement);
        let pose = match n.parent {
            Some(p) => world[p].then(&local),
            None => local,
        };
        world.push(pose);
    }
    let mut asm = Assembly::default();
    let mut files: HashMap<String, PathBuf> = HashMap::new();
    let mut used: Vec<String> = Vec::new();
    let tmp = std::env::temp_dir().join(format!("openforge-import-{}.step", std::process::id()));
    for (n, pose) in nodes.iter().zip(&world) {
        let Some(shape) = &n.shape else { continue };
        let definition = if n.definition.is_empty() {
            n.name.clone()
        } else {
            n.definition.clone()
        };
        // Same definition name with different geometry gets its own file.
        let size = shape
            .mass_props()
            .map(|m| (m.volume * 1e3).round() as i64)
            .unwrap_or(0);
        let key = format!("{definition}#{size}");
        let file = match files.get(&key) {
            Some(f) => f.clone(),
            None => {
                let mut stem = safe_name(&definition);
                let base = stem.clone();
                let mut k = 2;
                while used.contains(&stem) {
                    stem = format!("{base}-{k}");
                    k += 1;
                }
                used.push(stem.clone());
                shape.write_step(&tmp.to_string_lossy()).map_err(|e| e.0)?;
                let bytes = std::fs::read(&tmp).map_err(|e| e.to_string())?;
                let mut doc = Document::new();
                let id = doc.add(FeatureKind::Import(Import {
                    format: ImportFormat::Step,
                    source: format!("{definition}.step"),
                    data: base64::engine::general_purpose::STANDARD.encode(bytes),
                    mesh: None,
                }));
                if let Some(f) = doc.feature_mut(id) {
                    f.name = definition.clone();
                }
                let file = dir.join(format!("{stem}.ofp"));
                of_model::file::save(&doc, &file)?;
                files.insert(key, file.clone());
                file
            }
        };
        let k = asm.insert(&file, Some(*pose), None)?;
        if let Some(c) = asm.doc.component_mut(&k) {
            c.fixed = true;
            if !n.name.is_empty() {
                c.name = n.name.clone();
            }
        }
    }
    std::fs::remove_file(&tmp).ok();
    if asm.doc.components.is_empty() {
        return Err("the STEP file has no parts".into());
    }
    Ok(asm)
}

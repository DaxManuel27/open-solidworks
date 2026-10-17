//! Models shown in drawing views: parts built in a configuration and solved
//! assemblies, reloaded when their files change, combined into one shape per
//! view so hidden lines are removed across components.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use of_assembly::cache::{Cache, Part, canonical};
use of_assembly::{Assembly, CompRef, Loaded, Pose};
use of_model::geom::{DAffine3, Kind, Shape};
use of_model::{FeatureKind, Model, Ref};

/// One part placed in a view's model: a part view has one piece with an
/// empty component path.
pub struct Piece {
    pub component: Vec<String>,
    pub part: Rc<Part>,
    pub pose: Pose,
    /// Range of this piece's edges in the combined shape.
    pub first_edge: u32,
    pub edges: u32,
    /// When only some of the part's bodies are shown: each local edge's
    /// index in the part's whole shape.
    pub edge_map: Option<Vec<u32>>,
}

/// A model ready to project.
pub struct Scene {
    pub shape: Shape,
    pub pieces: Vec<Piece>,
    /// Changes whenever the geometry or placements change.
    pub key: String,
    pub is_assembly: bool,
    /// Assembly explode lines (start and end of each component's travel).
    pub explode_lines: Vec<[of_assembly::DVec3; 2]>,
}

impl Scene {
    /// The stable reference for an edge of the combined shape.
    pub fn edge_ref(&self, index: u32) -> Option<CompRef> {
        let p = self
            .pieces
            .iter()
            .find(|p| index >= p.first_edge && index < p.first_edge + p.edges)?;
        let local = index - p.first_edge;
        let global = match &p.edge_map {
            Some(map) => *map.get(local as usize)?,
            None => local,
        };
        let r = p.part.state().ref_for(Kind::Edge, global)?;
        Some(CompRef {
            component: p.component.clone(),
            entity: r,
        })
    }

    /// The piece and part state an entity reference points into.
    pub fn piece(&self, component: &[String]) -> Option<&Piece> {
        self.pieces.iter().find(|p| p.component == component)
    }

    /// World-space index of an edge reference in the combined shape.
    pub fn edge_index(&self, r: &CompRef) -> Option<u32> {
        let p = self.piece(&r.component)?;
        let (kind, i) = p.part.state().index_of(&r.entity).ok()?;
        if kind != Kind::Edge {
            return None;
        }
        let local = match &p.edge_map {
            Some(map) => map.iter().position(|g| *g == i)? as u32,
            None => i,
        };
        Some(p.first_edge + local)
    }
}

pub fn affine(p: &Pose) -> DAffine3 {
    DAffine3::from_rotation_translation(p.rotation, p.translation)
}

struct LoadedAssembly {
    asm: Assembly,
    hash: String,
}

/// Loaded models, keyed by canonical path and configuration.
#[derive(Default)]
pub struct Sources {
    parts: Cache,
    assemblies: HashMap<(PathBuf, Option<u32>), LoadedAssembly>,
    /// Parts with their Flat-Pattern feature in effect.
    flats: HashMap<(PathBuf, Option<u32>), Rc<Part>>,
}

/// Whether a path names an assembly.
pub fn is_assembly(path: &Path) -> bool {
    of_assembly::file::is_assembly(path)
}

impl Sources {
    /// The content hash of a model file.
    pub fn hash(&mut self, path: &Path) -> Result<String, String> {
        self.parts.hash(&canonical(path))
    }

    /// A part built in a configuration, flattened when `flat`.
    pub fn part(
        &mut self,
        path: &Path,
        configuration: Option<u32>,
        flat: bool,
    ) -> Result<Rc<Part>, String> {
        let part = self.parts.part(path, configuration)?;
        if !flat {
            return Ok(part);
        }
        let key = (part.path.clone(), configuration);
        if let Some(f) = self.flats.get(&key)
            && f.hash == part.hash
        {
            return Ok(f.clone());
        }
        let flat = Rc::new(flatten(&part)?);
        self.flats.insert(key, flat.clone());
        Ok(flat)
    }

    /// A solved assembly in a configuration, reopened when its file changed
    /// and re-solved when a component's file changed.
    pub fn assembly(
        &mut self,
        path: &Path,
        configuration: Option<u32>,
    ) -> Result<&mut Assembly, String> {
        let path = canonical(path);
        let hash = self.parts.hash(&path)?;
        let key = (path.clone(), configuration);
        let stale = self.assemblies.get(&key).is_none_or(|a| a.hash != hash);
        if stale {
            let mut asm = Assembly::open(&path)?;
            if let Some(c) = configuration {
                if !asm.doc.configurations.iter().any(|x| x.id == c) {
                    return Err(format!("the assembly has no configuration {c}"));
                }
                asm.doc.active_configuration = c;
            }
            asm.refresh();
            asm.solve(&[], false);
            self.assemblies
                .insert(key.clone(), LoadedAssembly { asm, hash });
        } else {
            let a = self.assemblies.get_mut(&key).expect("checked");
            if !a.asm.refresh().is_empty() {
                a.asm.solve(&[], false);
            }
        }
        Ok(&mut self.assemblies.get_mut(&key).expect("inserted").asm)
    }

    /// Custom properties of a model, with links evaluated.
    pub fn properties(
        &mut self,
        path: &Path,
        configuration: Option<u32>,
    ) -> Result<std::collections::BTreeMap<String, String>, String> {
        if is_assembly(path) {
            let asm = self.assembly(path, configuration)?;
            Ok(asm.doc.properties.clone())
        } else {
            let part = self.part(path, configuration, false)?;
            Ok(of_model::properties::effective(part.doc())
                .into_iter()
                .map(|(k, v)| {
                    let value = of_model::properties::evaluate(part.doc(), part.state(), &v.value);
                    (k, value)
                })
                .collect())
        }
    }

    /// The model of a view, combined into one shape. Components listed in
    /// `hidden` (by key path prefix) are left out.
    pub fn scene(
        &mut self,
        path: &Path,
        configuration: Option<u32>,
        flat: bool,
        exploded: bool,
        hidden: &[Vec<String>],
        bodies: Option<&[of_model::BodyId]>,
    ) -> Result<Scene, String> {
        if (flat || bodies.is_some()) && is_assembly(path) {
            return Err("flat patterns and body views show parts".into());
        }
        if !is_assembly(path) {
            let part = self.part(path, configuration, flat)?;
            let whole = part
                .shape()
                .cloned()
                .ok_or_else(|| "the part has no solid geometry".to_string())?;
            let (shape, edge_map) = match bodies {
                Some(ids) => {
                    let chosen: Vec<&Shape> = ids
                        .iter()
                        .map(|id| {
                            part.state()
                                .solid(id)
                                .map(|b| &b.geometry.shape)
                                .map_err(|_| format!("the part has no body {}", id.0))
                        })
                        .collect::<Result<_, _>>()?;
                    if chosen.is_empty() {
                        return Err("choose at least one body to show".into());
                    }
                    let shape = Shape::compound(chosen);
                    let map = shape.indices_in(&whole, Kind::Edge).map_err(|e| e.0)?;
                    (shape, Some(map))
                }
                None => (whole, None),
            };
            let edges = shape.count(Kind::Edge);
            let key = format!(
                "{}#{}#{:?}#{}#{:?}",
                part.path.display(),
                part.hash,
                configuration,
                if flat { "flat" } else { "formed" },
                bodies
            );
            return Ok(Scene {
                shape,
                pieces: vec![Piece {
                    component: Vec::new(),
                    part,
                    pose: Pose::IDENTITY,
                    first_edge: 0,
                    edges,
                    edge_map,
                }],
                key,
                is_assembly: false,
                explode_lines: Vec::new(),
            });
        }
        let asm = self.assembly(path, configuration)?;
        let instances = asm.instances();
        let (poses, lines) = if exploded {
            let steps = asm.doc.active().explode.clone();
            let e = of_assembly::explode::explode(&instances, &steps, None);
            (Some(e.poses), e.lines)
        } else {
            (None, Vec::new())
        };
        let mut placed_shapes = Vec::new();
        let mut pieces = Vec::new();
        let mut key = format!("{}#", path.display());
        let mut first = 0;
        for i in &instances {
            if i.hidden || hidden.iter().any(|h| i.key.starts_with(h)) {
                continue;
            }
            let Loaded::Part(part) = &i.loaded else {
                continue;
            };
            let Some(s) = part.shape() else { continue };
            let pose = poses
                .as_ref()
                .and_then(|p| p.get(&i.key).copied())
                .unwrap_or(i.pose);
            let placed = s.located(&affine(&pose)).map_err(|e| e.to_string())?;
            placed_shapes.push(placed);
            let edges = s.count(Kind::Edge);
            key.push_str(&format!(
                "{}:{}:{:?}:{:?}|",
                i.key_string(),
                part.hash,
                pose.translation.to_array(),
                pose.rotation.to_array()
            ));
            pieces.push(Piece {
                component: i.key.clone(),
                part: part.clone(),
                pose,
                first_edge: first,
                edges,
                edge_map: None,
            });
            first += edges;
        }
        if pieces.is_empty() {
            return Err("the assembly has no visible parts".into());
        }
        let shape = Shape::compound(&placed_shapes);
        // Edge maps of the combined shape list each piece's edges in its own
        // order; coincident duplicate instances would merge and break that.
        if shape.count(Kind::Edge) != first {
            return Err("two components occupy the same place; hide one to draw this view".into());
        }
        Ok(Scene {
            shape,
            pieces,
            key,
            is_assembly: true,
            explode_lines: lines,
        })
    }

    /// The part an entity reference points into and where it sits, without
    /// combining the model's shapes.
    pub fn locate(
        &mut self,
        path: &Path,
        configuration: Option<u32>,
        flat: bool,
        exploded: bool,
        r: &CompRef,
    ) -> Result<(Rc<Part>, Pose), String> {
        if !is_assembly(path) {
            if !r.component.is_empty() {
                return Err("a part view has no components".into());
            }
            return Ok((self.part(path, configuration, flat)?, Pose::IDENTITY));
        }
        let asm = self.assembly(path, configuration)?;
        let instances = asm.instances();
        let i = instances
            .iter()
            .find(|i| i.key == r.component)
            .ok_or("the component is not in the assembly")?;
        let Loaded::Part(part) = &i.loaded else {
            return Err("the component is not a part".into());
        };
        let mut pose = i.pose;
        if exploded {
            let steps = asm.doc.active().explode.clone();
            let e = of_assembly::explode::explode(&instances, &steps, None);
            if let Some(p) = e.poses.get(&i.key) {
                pose = *p;
            }
        }
        Ok((part.clone(), pose))
    }

    /// Forget a file so it reloads on next use.
    pub fn forget(&mut self, path: &Path) {
        self.parts.forget(path);
        let c = canonical(path);
        self.flats.retain(|(p, _), _| *p != c);
        self.assemblies.retain(|(p, _), _| *p != c);
    }
}

/// Reference to a part entity with no component (part views).
/// `part` with its Flat-Pattern feature in effect (added when it has none),
/// in every configuration.
fn flatten(part: &Part) -> Result<Part, String> {
    if part.state().sheet_metal.is_empty() {
        return Err("the part has no sheet metal to flatten".into());
    }
    let mut doc = part.doc().clone();
    let id = match doc
        .features
        .iter()
        .find(|f| matches!(f.kind, FeatureKind::FlatPattern(_)))
    {
        Some(f) => f.id,
        None => doc.add(FeatureKind::FlatPattern(Default::default())),
    };
    doc.feature_mut(id).expect("present").suppressed = false;
    for c in &mut doc.configurations.items {
        c.suppressed.remove(&id);
    }
    let mut model = Model::new(doc);
    model.rebuild();
    if let Some(e) = model
        .statuses()
        .iter()
        .find(|s| s.id == id)
        .and_then(|s| s.error.clone())
    {
        return Err(format!("can't flatten the part: {e}"));
    }
    Ok(Part {
        path: part.path.clone(),
        hash: part.hash.clone(),
        configuration: part.configuration,
        model,
        error: part.error.clone(),
    })
}

pub fn part_ref(entity: Ref) -> CompRef {
    CompRef {
        component: Vec::new(),
        entity,
    }
}

/// A planar face a relative-to-model view can use.
#[derive(Debug, Clone, PartialEq, serde::Serialize, schemars::JsonSchema)]
pub struct PlanarFace {
    pub entity: CompRef,
    /// Outward normal in the model's (assembly's) space.
    pub normal: [f64; 3],
    /// Area, mm².
    pub area: f64,
}

impl Sources {
    /// Planar faces of a part or of every visible part of an assembly,
    /// largest first.
    pub fn planar_faces(
        &mut self,
        path: &Path,
        configuration: Option<u32>,
    ) -> Result<Vec<PlanarFace>, String> {
        let scene = self.scene(path, configuration, false, false, &[], None)?;
        let mut out = Vec::new();
        for p in &scene.pieces {
            let state = p.part.state();
            let Some(body) = state.body.as_ref() else {
                continue;
            };
            for i in 0..body.shape.count(Kind::Face) {
                let Ok(info) = body.shape.face_info(i) else {
                    continue;
                };
                if info.kind != of_model::geom::SurfaceKind::Plane {
                    continue;
                }
                let Some(r) = state.ref_for(Kind::Face, i) else {
                    continue;
                };
                let n = p.pose.rotation * info.normal;
                out.push(PlanarFace {
                    entity: CompRef {
                        component: p.component.clone(),
                        entity: r,
                    },
                    normal: n.to_array(),
                    area: info.area,
                });
            }
        }
        out.sort_by(|a, b| b.area.total_cmp(&a.area));
        Ok(out)
    }
}

//! An open assembly and the `asm.*` methods clients call on it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use of_assembly::analysis::{self, CollisionOptions, InterferenceOptions};
use of_assembly::bom;
use of_assembly::hardware;
use of_assembly::{
    Assembly, AssemblyConfiguration, AssemblyDocument, CompRef, DQuat, DVec3, DragTarget,
    ExplodeStep, Instance, Loaded, MirrorComponents, Pattern, Pose,
};
use of_model::geom::Shape;
use of_protocol::{ErrorCode, RpcError};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::tools::asm_params::*;
use crate::views;

const UNDO_LIMIT: usize = 100;

type R = Result<Value, RpcError>;

fn args<T: DeserializeOwned>(params: Value) -> Result<T, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    serde_json::from_value(params).map_err(|e| RpcError::params(e.to_string()))
}

fn fail(e: impl std::fmt::Display) -> RpcError {
    RpcError::failed(e.to_string())
}

fn unknown(what: &str, id: impl std::fmt::Display) -> RpcError {
    RpcError::coded(ErrorCode::UnknownRef, format!("no {what} {id}"))
}

/// Column-major 4x4 matrix for the viewport.
fn matrix4(p: &Pose) -> [f64; 16] {
    let a = p.affine();
    let m = a.matrix3;
    let t = a.translation;
    [
        m.x_axis.x, m.x_axis.y, m.x_axis.z, 0.0, //
        m.y_axis.x, m.y_axis.y, m.y_axis.z, 0.0, //
        m.z_axis.x, m.z_axis.y, m.z_axis.z, 0.0, //
        t.x, t.y, t.z, 1.0,
    ]
}

/// Where generated hardware goes unless a folder is given.
pub fn hardware_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("OPENFORGE_HARDWARE_DIR") {
        return PathBuf::from(d);
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    home.join("OpenForge").join("Hardware")
}

pub struct AsmSession {
    pub asm: Assembly,
    dirty: bool,
    undo: Vec<AssemblyDocument>,
    redo: Vec<AssemblyDocument>,
    revision: u64,
    meshes: HashMap<String, Value>,
    /// Explode steps shown: `None` collapsed.
    explode: Option<usize>,
    drag_start: Option<AssemblyDocument>,
}

impl AsmSession {
    pub fn new() -> AsmSession {
        let mut s = AsmSession {
            asm: Assembly::default(),
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            meshes: HashMap::new(),
            explode: None,
            drag_start: None,
        };
        s.asm.solve(&[], true);
        s
    }

    /// A session for an assembly built elsewhere (STEP import); unsaved.
    pub fn from_assembly(asm: Assembly) -> AsmSession {
        let mut s = AsmSession::new();
        s.asm = asm;
        s.dirty = true;
        s.asm.solve(&[], true);
        s
    }

    pub fn open(path: &Path) -> Result<AsmSession, String> {
        let mut s = AsmSession::new();
        s.asm = Assembly::open(path)?;
        s.asm.refresh();
        s.asm.solve(&[], true);
        Ok(s)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn doc(&self) -> &AssemblyDocument {
        &self.asm.doc
    }

    fn checkpoint(&mut self) {
        self.undo.push(self.asm.doc.clone());
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    /// Run a change; restore the document if it fails.
    fn change(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<(), RpcError>,
    ) -> Result<(), RpcError> {
        let saved = self.asm.doc.clone();
        self.checkpoint();
        match f(self) {
            Ok(()) => Ok(()),
            Err(e) => {
                self.asm.doc = saved;
                self.undo.pop();
                self.asm.solve(&[], true);
                Err(e)
            }
        }
    }

    pub fn handle(&mut self, method: &str, params: Value) -> R {
        let before = self.asm.doc.clone();
        let explode = self.explode;
        let result = self.handle_inner(method, params);
        if self.asm.doc != before || self.explode != explode {
            self.revision += 1;
        }
        result.map(|mut v| {
            if let Value::Object(o) = &mut v {
                o.insert("revision".into(), json!(self.revision));
            }
            v
        })
    }

    fn handle_inner(&mut self, method: &str, params: Value) -> R {
        match method {
            "doc.info" | "asm.info" => self.full(),
            "doc.save" => {
                #[derive(serde::Deserialize)]
                struct P {
                    path: Option<PathBuf>,
                }
                let p: P = args(params)?;
                let path = p
                    .path
                    .or_else(|| self.asm.path().map(Path::to_path_buf))
                    .ok_or_else(|| RpcError::params("choose where to save"))?;
                self.asm.save(&path).map_err(fail)?;
                self.dirty = false;
                Ok(self.snapshot())
            }
            "doc.undo" | "doc.redo" => {
                let (from, to) = if method == "doc.undo" {
                    (&mut self.undo, &mut self.redo)
                } else {
                    (&mut self.redo, &mut self.undo)
                };
                if let Some(doc) = from.pop() {
                    to.push(self.asm.doc.clone());
                    self.asm.doc = doc;
                    self.dirty = true;
                    self.asm.solve(&[], true);
                }
                self.full()
            }
            "doc.set_units" => {
                #[derive(serde::Deserialize)]
                struct P {
                    units: of_model::Units,
                }
                let p: P = args(params)?;
                self.checkpoint();
                self.asm.doc.units = p.units;
                Ok(self.snapshot())
            }
            "asm.insert" => {
                let p: InsertComponent = args(params)?;
                let path = p.path.clone();
                let mut key = String::new();
                self.change(|s| {
                    key = s
                        .asm
                        .insert(&path, p.transform.map(Pose::from), p.configuration)
                        .map_err(fail)?;
                    s.asm.solve(&[], true);
                    Ok(())
                })?;
                let mut v = self.full()?;
                v["key"] = json!(key);
                Ok(v)
            }
            "asm.insert_hardware" => {
                let p: InsertHardware = args(params)?;
                let folder = p.folder.unwrap_or_else(hardware_dir);
                let file = hardware::library_part(&folder, &p.hardware).map_err(fail)?;
                let mut key = String::new();
                self.change(|s| {
                    key = s
                        .asm
                        .insert(&file, p.transform.map(Pose::from), None)
                        .map_err(fail)?;
                    s.asm.solve(&[], true);
                    Ok(())
                })?;
                let mut v = self.full()?;
                v["key"] = json!(key);
                v["path"] = json!(file);
                Ok(v)
            }
            "asm.hardware_catalog" => Ok(json!({ "catalog": hardware::catalog() })),
            "asm.component.update" => {
                let p: UpdateComponent = args(params)?;
                if self.asm.doc.component(&p.key).is_none() {
                    return Err(unknown("component", &p.key));
                }
                self.change(|s| {
                    let active = s.asm.doc.active_configuration;
                    let c = s.asm.doc.component_mut(&p.key).unwrap();
                    if let Some(n) = &p.name {
                        if n.trim().is_empty() {
                            return Err(RpcError::params("the name cannot be empty"));
                        }
                        c.name = n.trim().to_string();
                    }
                    if let Some(f) = p.fixed {
                        c.fixed = f;
                    }
                    if let Some(h) = p.hidden {
                        c.hidden = h;
                    }
                    if let Some(e) = p.exclude_from_bom {
                        c.exclude_from_bom = e;
                    }
                    if let Some(f) = p.flexible {
                        c.flexible = f;
                    }
                    if let Some(t) = p.transform {
                        if !t.is_valid() {
                            return Err(RpcError::params("the transform is invalid"));
                        }
                        c.transform = t;
                    }
                    let key = p.key.clone();
                    let config = s
                        .asm
                        .doc
                        .configurations
                        .iter_mut()
                        .find(|c| c.id == active)
                        .unwrap();
                    if let Some(sup) = p.suppressed {
                        config.suppressed.insert(key.clone(), sup);
                    }
                    if let Some(cfg) = p.configuration {
                        match cfg {
                            Some(id) => {
                                config.part_configurations.insert(key.clone(), id);
                            }
                            None => {
                                config.part_configurations.remove(&key);
                            }
                        }
                    }
                    if let Some(c) = s.asm.doc.component(&key).cloned()
                        && let Loaded::Missing(e) = s.asm.load(&c)
                        && !s.asm.doc.is_suppressed(&c)
                    {
                        return Err(fail(e));
                    }
                    s.asm.solve(&[], true);
                    Ok(())
                })?;
                self.full()
            }
            "asm.component.delete" => {
                let p: ComponentKeys = args(params)?;
                for k in &p.keys {
                    if self.asm.doc.component(k).is_none() {
                        return Err(unknown("component", k));
                    }
                }
                let mut deleted = Vec::new();
                self.change(|s| {
                    deleted = s.asm.delete_components(&p.keys);
                    s.asm.solve(&[], true);
                    Ok(())
                })?;
                let mut v = self.full()?;
                v["deleted_mates"] = json!(deleted);
                Ok(v)
            }
            "asm.component.replace" => {
                let p: ReplaceComponent = args(params)?;
                let Some(c) = self.asm.doc.component(&p.key).cloned() else {
                    return Err(unknown("component", &p.key));
                };
                if !p.path.exists() {
                    return Err(fail(format!("{} does not exist", p.path.display())));
                }
                let mut broken = Vec::new();
                self.change(|s| {
                    let path = match s.asm.path() {
                        Some(_) => of_assembly::relative_to(&p.path, &s.asm.base_dir()),
                        None => of_assembly::cache::canonical(&p.path)
                            .to_string_lossy()
                            .replace('\\', "/"),
                    };
                    let cm = s.asm.doc.component_mut(&p.key).unwrap();
                    cm.path = path;
                    cm.configuration = p.configuration;
                    cm.hash = None;
                    if let Loaded::Missing(e) = s.asm.load(&c) {
                        let _ = e;
                    }
                    let updated = s.asm.doc.component(&p.key).cloned().unwrap();
                    if let Loaded::Missing(e) = s.asm.load(&updated) {
                        return Err(fail(e));
                    }
                    s.asm.solve(&[], true);
                    broken = s
                        .asm
                        .diagnostics
                        .mate_errors
                        .keys()
                        .copied()
                        .filter(|id| {
                            s.asm.doc.mate(*id).is_some_and(|m| {
                                of_assembly::mate_components(&m.kind).contains(&p.key)
                            })
                        })
                        .collect();
                    Ok(())
                })?;
                let mut v = self.full()?;
                v["broken_mates"] = json!(broken);
                Ok(v)
            }
            "asm.component.open" => {
                #[derive(serde::Deserialize)]
                struct P {
                    component: Vec<String>,
                }
                let p: P = args(params)?;
                let inst = self
                    .asm
                    .instances()
                    .into_iter()
                    .find(|i| i.key == p.component)
                    .ok_or_else(|| unknown("component", p.component.join("/")))?;
                Ok(json!({ "path": inst.file }))
            }
            "asm.mate.add" => {
                let p: AddMate = args(params)?;
                let mut id = 0;
                self.change(|s| {
                    id = s.asm.add_mate(p.name, p.mate).map_err(fail)?;
                    Ok(())
                })?;
                let mut v = self.full()?;
                v["id"] = json!(id);
                Ok(v)
            }
            "asm.mate.preview" => {
                let p: AddMate = args(params)?;
                let saved = self.asm.doc.clone();
                let result = self.asm.add_mate(p.name, p.mate);
                let instances = self.instances_json();
                self.asm.doc = saved;
                self.asm.solve(&[], true);
                match result {
                    Ok(_) => Ok(json!({ "instances": instances })),
                    Err(e) => Err(fail(e)),
                }
            }
            "asm.mate.update" => {
                let p: UpdateMate = args(params)?;
                if self.asm.doc.mate(p.id).is_none() {
                    return Err(unknown("mate", p.id));
                }
                self.change(|s| {
                    if let Some(kind) = p.mate {
                        s.asm.update_mate(p.id, kind).map_err(fail)?;
                    }
                    let active = s.asm.doc.active_configuration;
                    if let Some(n) = p.name {
                        if n.trim().is_empty() {
                            return Err(RpcError::params("the name cannot be empty"));
                        }
                        let m = s.asm.doc.mates.iter_mut().find(|m| m.id == p.id).unwrap();
                        m.name = n.trim().to_string();
                    }
                    if let Some(sup) = p.suppressed {
                        let c = s
                            .asm
                            .doc
                            .configurations
                            .iter_mut()
                            .find(|c| c.id == active)
                            .unwrap();
                        c.mates_suppressed.insert(p.id.to_string(), sup);
                    }
                    s.asm.solve(&[], true);
                    Ok(())
                })?;
                self.full()
            }
            "asm.mate.delete" => {
                let p: ItemIds = args(params)?;
                self.change(|s| {
                    s.asm.doc.mates.retain(|m| !p.ids.contains(&m.id));
                    for c in &mut s.asm.doc.configurations {
                        c.mates_suppressed
                            .retain(|k, _| !p.ids.iter().any(|i| i.to_string() == *k));
                    }
                    s.asm.solve(&[], true);
                    Ok(())
                })?;
                self.full()
            }
            "asm.move" => self.move_component(params),
            "asm.pattern.add" | "asm.pattern.update" => {
                let (id, p) = if method == "asm.pattern.add" {
                    (None, args::<AddPattern>(params)?)
                } else {
                    let u: UpdatePattern = args(params)?;
                    (Some(u.id), u.pattern)
                };
                let mut new_id = 0;
                self.change(|s| {
                    for seed in &p.seeds {
                        if s.asm.doc.component(seed).is_none() {
                            return Err(unknown("component", seed));
                        }
                    }
                    let id = match id {
                        Some(id) => id,
                        None => s.asm.doc.take_id(),
                    };
                    new_id = id;
                    let name = p.name.clone().unwrap_or_else(|| {
                        let base = match p.pattern {
                            of_assembly::PatternKind::Linear { .. } => "LocalLPattern",
                            of_assembly::PatternKind::Circular { .. } => "LocalCirPattern",
                            of_assembly::PatternKind::PatternDriven { .. } => "DerivedPattern",
                            of_assembly::PatternKind::SketchDriven { .. } => "SketchPattern",
                            of_assembly::PatternKind::CurveDriven { .. } => "CurvePattern",
                        };
                        let n = s.asm.doc.patterns.len() + 1;
                        format!("{base}{n}")
                    });
                    let pattern = Pattern {
                        id,
                        name,
                        kind: p.pattern.clone(),
                        seeds: p.seeds.clone(),
                        skip: p.skip.clone(),
                        suppressed: false,
                    };
                    match s.asm.doc.patterns.iter_mut().find(|x| x.id == id) {
                        Some(existing) => {
                            let suppressed = existing.suppressed;
                            *existing = Pattern {
                                suppressed,
                                ..pattern
                            };
                        }
                        None if method == "asm.pattern.update" => {
                            return Err(unknown("pattern", id));
                        }
                        None => s.asm.doc.patterns.push(pattern),
                    }
                    s.check_derived(id)
                })?;
                let mut v = self.full()?;
                v["id"] = json!(new_id);
                Ok(v)
            }
            "asm.mirror.add" | "asm.mirror.update" => {
                let (id, p) = if method == "asm.mirror.add" {
                    (None, args::<AddMirror>(params)?)
                } else {
                    let u: UpdateMirror = args(params)?;
                    (Some(u.id), u.mirror)
                };
                let mut new_id = 0;
                self.change(|s| {
                    for item in &p.components {
                        if s.asm.doc.component(&item.component).is_none() {
                            return Err(unknown("component", &item.component));
                        }
                    }
                    let id = match id {
                        Some(id) => id,
                        None => s.asm.doc.take_id(),
                    };
                    new_id = id;
                    let name = p.name.clone().unwrap_or_else(|| {
                        format!("MirrorComponent{}", s.asm.doc.mirrors.len() + 1)
                    });
                    let m = MirrorComponents {
                        id,
                        name,
                        plane: p.plane.clone(),
                        components: p.components.clone(),
                        position: p.position,
                        suppressed: false,
                    };
                    match s.asm.doc.mirrors.iter_mut().find(|x| x.id == id) {
                        Some(existing) => *existing = m,
                        None if method == "asm.mirror.update" => return Err(unknown("mirror", id)),
                        None => s.asm.doc.mirrors.push(m),
                    }
                    s.check_derived(id)
                })?;
                let mut v = self.full()?;
                v["id"] = json!(new_id);
                Ok(v)
            }
            "asm.feature.delete" | "asm.feature.suppress" => {
                #[derive(serde::Deserialize)]
                struct P {
                    id: u32,
                    #[serde(default)]
                    suppressed: bool,
                }
                let p: P = args(params)?;
                let exists = self.asm.doc.patterns.iter().any(|x| x.id == p.id)
                    || self.asm.doc.mirrors.iter().any(|x| x.id == p.id);
                if !exists {
                    return Err(unknown("pattern or mirror", p.id));
                }
                self.change(|s| {
                    if method == "asm.feature.delete" {
                        s.asm.doc.patterns.retain(|x| x.id != p.id);
                        s.asm.doc.mirrors.retain(|x| x.id != p.id);
                        // Mates to the instances it made go with it.
                        let tag_p = format!("@p{}.", p.id);
                        let tag_m = format!("@m{}", p.id);
                        s.asm.doc.mates.retain(|m| {
                            !of_assembly::mate_refs(&m.kind).iter().any(|r| {
                                r.component
                                    .first()
                                    .is_some_and(|k| k.contains(&tag_p) || k.ends_with(&tag_m))
                            })
                        });
                    } else {
                        for x in &mut s.asm.doc.patterns {
                            if x.id == p.id {
                                x.suppressed = p.suppressed;
                            }
                        }
                        for x in &mut s.asm.doc.mirrors {
                            if x.id == p.id {
                                x.suppressed = p.suppressed;
                            }
                        }
                    }
                    s.asm.solve(&[], true);
                    Ok(())
                })?;
                self.full()
            }
            "asm.explode.add" | "asm.explode.update" => {
                let (id, p) = if method == "asm.explode.add" {
                    (None, args::<AddExplodeStep>(params)?)
                } else {
                    let u: UpdateExplodeStep = args(params)?;
                    (Some(u.id), u.step)
                };
                let mut new_id = 0;
                self.change(|s| {
                    for c in &p.components {
                        if c.first().is_none_or(|k| s.asm.doc.component(k).is_none()) {
                            return Err(unknown("component", c.join("/")));
                        }
                    }
                    let id = match id {
                        Some(id) => id,
                        None => s.asm.doc.take_id(),
                    };
                    new_id = id;
                    let count = s.asm.doc.active().explode.len();
                    let name = p
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("Explode Step{}", count + 1));
                    let step = ExplodeStep {
                        id,
                        name,
                        components: p.components.clone(),
                        motion: p.motion.clone(),
                    };
                    let steps = &mut s.asm.doc.active_mut().explode;
                    match steps.iter_mut().find(|x| x.id == id) {
                        Some(existing) => *existing = step,
                        None if method == "asm.explode.update" => {
                            return Err(unknown("explode step", id));
                        }
                        None => steps.push(step),
                    }
                    Ok(())
                })?;
                self.explode = Some(usize::MAX);
                let mut v = self.full()?;
                v["id"] = json!(new_id);
                Ok(v)
            }
            "asm.explode.delete" => {
                let p: ItemIds = args(params)?;
                self.change(|s| {
                    s.asm
                        .doc
                        .active_mut()
                        .explode
                        .retain(|x| !p.ids.contains(&x.id));
                    Ok(())
                })?;
                self.full()
            }
            "asm.explode.show" => {
                let p: ShowExplode = args(params)?;
                self.explode = p.steps;
                self.full()
            }
            "asm.configuration.create" => {
                let p: CreateAssemblyConfiguration = args(params)?;
                if p.name.trim().is_empty() {
                    return Err(RpcError::params("the name cannot be empty"));
                }
                self.change(|s| {
                    let id = s
                        .asm
                        .doc
                        .configurations
                        .iter()
                        .map(|c| c.id)
                        .max()
                        .unwrap_or(0)
                        + 1;
                    let mut c = match p.copy_from {
                        Some(from) => s
                            .asm
                            .doc
                            .configurations
                            .iter()
                            .find(|c| c.id == from)
                            .cloned()
                            .ok_or_else(|| {
                                RpcError::coded(
                                    ErrorCode::UnknownConfiguration,
                                    format!("no configuration {from}"),
                                )
                            })?,
                        None => AssemblyConfiguration::new(id, ""),
                    };
                    c.id = id;
                    c.name = p.name.trim().to_string();
                    for step in &mut c.explode {
                        step.id = s.asm.doc.take_id();
                    }
                    s.asm.doc.configurations.push(c);
                    s.asm.doc.active_configuration = id;
                    s.asm.solve(&[], true);
                    Ok(())
                })?;
                self.full()
            }
            "asm.configuration.activate" | "asm.configuration.delete" => {
                let p: AssemblyConfigurationArgs = args(params)?;
                if !self.asm.doc.configurations.iter().any(|c| c.id == p.id) {
                    return Err(RpcError::coded(
                        ErrorCode::UnknownConfiguration,
                        format!("no configuration {}", p.id),
                    ));
                }
                self.change(|s| {
                    if method == "asm.configuration.activate" {
                        s.asm.doc.active_configuration = p.id;
                    } else {
                        if s.asm.doc.configurations.len() == 1 {
                            return Err(fail("an assembly needs at least one configuration"));
                        }
                        s.asm.doc.configurations.retain(|c| c.id != p.id);
                        if s.asm.doc.active_configuration == p.id {
                            s.asm.doc.active_configuration = s.asm.doc.configurations[0].id;
                        }
                    }
                    s.asm.solve(&[], true);
                    Ok(())
                })?;
                self.full()
            }
            "asm.configuration.rename" => {
                #[derive(serde::Deserialize)]
                struct P {
                    id: u32,
                    name: String,
                }
                let p: P = args(params)?;
                if p.name.trim().is_empty() {
                    return Err(RpcError::params("the name cannot be empty"));
                }
                self.change(|s| {
                    let c = s
                        .asm
                        .doc
                        .configurations
                        .iter_mut()
                        .find(|c| c.id == p.id)
                        .ok_or_else(|| {
                            RpcError::coded(
                                ErrorCode::UnknownConfiguration,
                                format!("no configuration {}", p.id),
                            )
                        })?;
                    c.name = p.name.trim().to_string();
                    Ok(())
                })?;
                self.full()
            }
            "asm.reference.add" => {
                let p: AddReference = args(params)?;
                use of_model::FeatureKind as K;
                if !matches!(
                    p.feature,
                    K::Plane(_) | K::Axis(_) | K::Point(_) | K::CoordSys(_)
                ) {
                    return Err(RpcError::params(
                        "assembly reference geometry is a plane, axis, point or coordinate system",
                    ));
                }
                let mut id = 0;
                self.change(|s| {
                    id = s.asm.doc.reference.add(p.feature);
                    if let Some(n) = p.name
                        && let Some(f) = s.asm.doc.reference.feature_mut(id)
                    {
                        f.name = n;
                    }
                    let state = s.asm.reference_model().statuses();
                    if let Some(e) = state
                        .iter()
                        .find(|st| st.id == id)
                        .and_then(|st| st.error.clone())
                    {
                        return Err(fail(e));
                    }
                    Ok(())
                })?;
                let mut v = self.full()?;
                v["id"] = json!(id);
                Ok(v)
            }
            "asm.reference.delete" => {
                let p: ItemIds = args(params)?;
                self.change(|s| {
                    s.asm.doc.reference.delete(&p.ids);
                    s.asm.solve(&[], true);
                    Ok(())
                })?;
                self.full()
            }
            "asm.pick" => {
                let p: PickRef = args(params)?;
                let inst = self
                    .asm
                    .instances()
                    .into_iter()
                    .find(|i| i.key == p.component)
                    .ok_or_else(|| unknown("component", p.component.join("/")))?;
                let part = inst
                    .part()
                    .ok_or_else(|| fail("pick a face, edge or vertex of a part"))?;
                let entity = part
                    .state()
                    .ref_for(p.kind, p.index)
                    .ok_or_else(|| unknown("geometry", p.index))?;
                let r = CompRef {
                    component: p.component,
                    entity,
                };
                let geo = self.asm.world_geometry(&r).ok();
                Ok(json!({ "ref": r, "geometry": geo.map(|g| geometry_json(&g)) }))
            }
            "asm.topology" => {
                let p: ComponentPath = args(params)?;
                self.topology(&p.component)
            }
            "asm.measure" | "measure" => {
                let p: MeasureRefs = args(params)?;
                self.measure(&p.refs)
            }
            "asm.interference" => {
                let p: Interference = args(params)?;
                let found = analysis::interference(
                    &mut self.asm,
                    &InterferenceOptions {
                        components: p.components,
                        treat_coincidence: p.treat_coincidence,
                        subassemblies_as_components: p.subassemblies_as_components,
                        include_hidden: p.include_hidden,
                    },
                );
                let items: Vec<Value> = found
                    .iter()
                    .map(|i| {
                        json!({
                            "a": i.a, "b": i.b, "volume": i.volume, "ignored": i.ignored,
                            "mesh": i.shape.as_ref().and_then(|s| views::mesh(s).ok()),
                        })
                    })
                    .collect();
                Ok(json!({ "interferences": items }))
            }
            "asm.interference.ignore" => {
                let p: IgnoreInterference = args(params)?;
                self.change(|s| {
                    let key = analysis::pair_key(&p.a, &p.b);
                    s.asm.doc.ignored_interferences.retain(|k| *k != key);
                    if p.ignored {
                        s.asm.doc.ignored_interferences.push(key);
                    }
                    Ok(())
                })?;
                Ok(self.snapshot())
            }
            "asm.clearance" => {
                let p: Clearance = args(params)?;
                if p.minimum.is_nan() || p.minimum <= 0.0 {
                    return Err(RpcError::params("the minimum clearance must be positive"));
                }
                Ok(
                    json!({ "clearances": analysis::clearance(&mut self.asm, &p.components, p.minimum) }),
                )
            }
            "asm.bom" => {
                let p: Bom = args(params)?;
                let rows = bom::bom(&mut self.asm, p.kind);
                if let Some(path) = &p.csv {
                    std::fs::write(path, bom::csv(&rows))
                        .map_err(|e| fail(format!("could not write {}: {e}", path.display())))?;
                }
                Ok(json!({ "rows": rows }))
            }
            "asm.mass_properties" | "mass_properties" => Ok(json!(
                analysis::mass_properties(&mut self.asm).map_err(fail)?
            )),
            "export" => {
                #[derive(serde::Deserialize)]
                struct P {
                    path: PathBuf,
                }
                let p: P = args(params)?;
                let ext = p
                    .path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                match ext.as_str() {
                    "step" | "stp" => {
                        of_assembly::exchange::export_step(&mut self.asm, &p.path).map_err(fail)?
                    }
                    "iges" | "igs" | "stl" => {
                        let shape =
                            of_assembly::exchange::assembled_shape(&mut self.asm).map_err(fail)?;
                        let format = of_io::ExportFormat::from_path(&p.path)
                            .ok_or_else(|| RpcError::params("export as .step, .iges or .stl"))?;
                        of_io::export(&shape, &p.path, format).map_err(fail)?;
                    }
                    _ => return Err(RpcError::params("export as .step, .iges or .stl")),
                }
                Ok(json!({ "path": p.path }))
            }
            "asm.reload" => {
                let changed = self.asm.refresh();
                self.meshes.clear();
                self.asm.solve(&[], true);
                let mut v = self.full()?;
                v["reloaded"] = json!(changed);
                Ok(v)
            }
            _ => Err(RpcError::new(
                RpcError::METHOD_NOT_FOUND,
                format!("{method} is not available while an assembly is open"),
            )),
        }
    }

    /// Fail if a pattern or mirror can't place its instances.
    fn check_derived(&mut self, id: u32) -> Result<(), RpcError> {
        self.asm.diagnostics.component_errors.clear();
        self.asm.instances();
        let key_p = format!("pattern {id}");
        let key_m = format!("mirror {id}");
        if let Some(e) = self
            .asm
            .diagnostics
            .component_errors
            .get(&key_p)
            .or_else(|| self.asm.diagnostics.component_errors.get(&key_m))
        {
            return Err(fail(e.clone()));
        }
        self.asm.solve(&[], true);
        Ok(())
    }

    fn move_component(&mut self, params: Value) -> R {
        let p: MoveComponent = args(params)?;
        let phase = p.phase.unwrap_or(DragPhase::End);
        let current = self
            .asm
            .world_pose(&p.component)
            .ok_or_else(|| unknown("component", p.component.join("/")))?;
        let world = |v: [f64; 3]| DVec3::from(v);
        let target = match p.motion {
            Motion::Delta { translation } => DragTarget::Pose(Pose::new(
                current.translation + world(translation),
                current.rotation,
            )),
            Motion::To { local, point } => {
                let now = current.point(world(local));
                DragTarget::Pose(Pose::new(
                    current.translation + world(point) - now,
                    current.rotation,
                ))
            }
            Motion::Along {
                reference,
                distance,
            } => {
                let d = self
                    .asm
                    .world_geometry(&reference)
                    .map_err(fail)?
                    .direction()
                    .ok_or_else(|| fail("select an edge, axis or flat face for the direction"))?;
                DragTarget::Pose(Pose::new(
                    current.translation + d.normalize() * distance,
                    current.rotation,
                ))
            }
            Motion::Rotate { angles } => {
                // About X, then Y, then Z of the assembly.
                let q = DQuat::from_rotation_z(angles[2])
                    * DQuat::from_rotation_y(angles[1])
                    * DQuat::from_rotation_x(angles[0]);
                DragTarget::Pose(Pose::new(current.translation, q * current.rotation))
            }
            Motion::About { reference, angle } => {
                let (o, d) = self
                    .asm
                    .world_geometry(&reference)
                    .map_err(fail)?
                    .axis()
                    .ok_or_else(|| {
                        fail("select an axis, straight edge or cylinder to rotate about")
                    })?;
                let r = of_assembly::patterns::rotation_about(o, d, angle);
                DragTarget::Pose(r.then(&current))
            }
            Motion::Pose { transform } => DragTarget::Pose(transform.into()),
            Motion::Drag { local, point } => DragTarget::Point {
                local: world(local),
                target: world(point),
            },
        };
        if phase == DragPhase::Start || (phase == DragPhase::End && self.drag_start.is_none()) {
            self.drag_start = Some(self.asm.doc.clone());
        }
        let collision = p.collision.map(|c| CollisionOptions {
            dragged_only: c.dragged_only,
        });
        let outcome = self
            .asm
            .drag(&p.component, target, collision)
            .map_err(fail)?;
        let collision = json!({
            "components": outcome.collision.as_ref().map(|(a, b)| [a, b]),
            "faces": outcome.faces.iter().map(|(k, f)| json!({"component": k, "faces": f})).collect::<Vec<_>>(),
        });
        if phase == DragPhase::End {
            if let Some(start) = self.drag_start.take()
                && start != self.asm.doc
            {
                self.undo.push(start);
                if self.undo.len() > UNDO_LIMIT {
                    self.undo.remove(0);
                }
                self.redo.clear();
                self.dirty = true;
            }
            self.asm.solve(&[], true);
            let mut v = self.full()?;
            v["collision"] = collision;
            v["moved"] = json!(outcome.moved);
            return Ok(v);
        }
        Ok(json!({
            "instances": self.instances_json(),
            "collision": collision,
            "moved": outcome.moved,
        }))
    }

    /// Faces and edges of one component in the assembly frame, with the
    /// component references mates and measurements take.
    fn topology(&mut self, component: &[String]) -> R {
        let inst = self
            .asm
            .instances()
            .into_iter()
            .find(|i| i.key == component)
            .ok_or_else(|| unknown("component", component.join("/")))?;
        let part = inst
            .part()
            .ok_or_else(|| fail("that component is not a part; list one of its parts"))?;
        let state = part.state();
        let Some(body) = &state.body else {
            return Ok(json!({ "faces": [], "edges": [] }));
        };
        let pose = inst.pose;
        let round = |v: f64| (v * 1e4).round() / 1e4;
        let p3 = |v: DVec3| {
            let w = pose.point(v);
            [round(w.x), round(w.y), round(w.z)]
        };
        let v3 = |v: DVec3| {
            let w = pose.vector(v);
            [round(w.x), round(w.y), round(w.z)]
        };
        let r = |entity: of_model::Ref| CompRef {
            component: component.to_vec(),
            entity,
        };
        let faces: Vec<Value> = (0..body.shape.count(of_model::geom::Kind::Face))
            .filter_map(|i| {
                let f = body.shape.face_info(i).ok()?;
                Some(json!({
                    "ref": r(state.ref_for(of_model::geom::Kind::Face, i)?),
                    "surface": f.kind,
                    "area": round(f.area),
                    "centroid": p3(f.centroid),
                    "normal": v3(f.normal),
                    "radius": (f.radius > 0.0).then(|| round(f.radius)),
                }))
            })
            .collect();
        let edges: Vec<Value> = (0..body.shape.count(of_model::geom::Kind::Edge))
            .filter_map(|i| {
                let e = body.shape.edge_info(i).ok()?;
                Some(json!({
                    "ref": r(state.ref_for(of_model::geom::Kind::Edge, i)?),
                    "curve": e.kind,
                    "length": round(e.length),
                    "start": p3(e.start),
                    "end": p3(e.end),
                    "radius": (e.radius > 0.0).then(|| round(e.radius)),
                }))
            })
            .collect();
        Ok(json!({ "faces": faces, "edges": edges }))
    }

    fn measure(&mut self, refs: &[CompRef]) -> R {
        let geos: Vec<of_assembly::geometry::Geo> = refs
            .iter()
            .map(|r| self.asm.world_geometry(r))
            .collect::<Result<_, _>>()
            .map_err(fail)?;
        let mut out = json!({ "geometry": geos.iter().map(geometry_json).collect::<Vec<_>>() });
        if let [a, b] = refs {
            let sa = self.ref_shape(a)?;
            let sb = self.ref_shape(b)?;
            if let (Some(sa), Some(sb)) = (sa, sb) {
                let d = sa.distance(&sb).map_err(|e| fail(e.0))?;
                out["distance"] = json!(d.value);
                out["points"] = json!([d.a.to_array(), d.b.to_array()]);
            } else {
                out["distance"] = json!(geos[0].point().distance(geos[1].point()));
            }
            if let (Some(da), Some(db)) = (geos[0].direction(), geos[1].direction()) {
                out["angle"] = json!(da.angle_between(db));
            }
        }
        Ok(out)
    }

    /// A face, edge or vertex as a placed shape, for distances.
    fn ref_shape(&mut self, r: &CompRef) -> Result<Option<Shape>, RpcError> {
        let instances = self.asm.instances();
        let Some(inst) = instances.iter().find(|i| i.key == r.component) else {
            return Ok(None);
        };
        let Some(part) = inst.part() else {
            return Ok(None);
        };
        let Ok((kind, index)) = part.state().index_of(&r.entity) else {
            return Ok(None);
        };
        let Some(shape) = part.shape() else {
            return Ok(None);
        };
        let sub = shape.sub_shape(kind, index).map_err(|e| fail(e.0))?;
        Ok(Some(
            sub.located(&inst.pose.affine()).map_err(|e| fail(e.0))?,
        ))
    }

    fn part_key(inst: &Instance) -> Option<String> {
        let p = inst.part()?;
        Some(format!(
            "{}#{}#{}",
            p.path.display(),
            p.hash,
            p.configuration.map_or("-".into(), |c| c.to_string())
        ))
    }

    fn instances_json(&mut self) -> Vec<Value> {
        let instances = self.asm.instances();
        let exploded = self.explode.map(|n| {
            of_assembly::explode::explode(&instances, &self.asm.doc.active().explode, Some(n))
        });
        instances
            .iter()
            .map(|i| {
                let shown = exploded
                    .as_ref()
                    .and_then(|e| e.poses.get(&i.key))
                    .copied()
                    .unwrap_or(i.pose);
                json!({
                    "key": i.key,
                    "name": i.name,
                    "part": AsmSession::part_key(i),
                    "transform": matrix4(&shown),
                    "hidden": i.hidden,
                    "derived": i.derived,
                    "kind": match &i.loaded {
                        Loaded::Part(_) => "part",
                        Loaded::Assembly(_) => "assembly",
                        Loaded::Missing(_) => "missing",
                    },
                })
            })
            .collect()
    }

    /// Everything the viewport draws.
    fn scene(&mut self) -> Result<Value, RpcError> {
        let instances = self.asm.instances();
        let mut parts = serde_json::Map::new();
        let mut boxes: Vec<DVec3> = Vec::new();
        for i in &instances {
            let (Some(key), Some(part)) = (AsmSession::part_key(i), i.part()) else {
                continue;
            };
            if let Some(b) = part.shape().and_then(|s| s.bbox()) {
                for x in [b.min.x, b.max.x] {
                    for y in [b.min.y, b.max.y] {
                        for z in [b.min.z, b.max.z] {
                            boxes.push(i.pose.point(DVec3::new(x, y, z)));
                        }
                    }
                }
            }
            if parts.contains_key(&key) {
                continue;
            }
            let mesh = match self.meshes.get(&key) {
                Some(m) => m.clone(),
                None => {
                    let m = match part.shape() {
                        Some(s) => json!(views::mesh(s).map_err(fail)?),
                        None => Value::Null,
                    };
                    self.meshes.insert(key.clone(), m.clone());
                    m
                }
            };
            parts.insert(key, mesh);
        }
        let lines: Vec<[[f64; 3]; 2]> = match self.explode {
            Some(n) => {
                of_assembly::explode::explode(&instances, &self.asm.doc.active().explode, Some(n))
                    .lines
                    .iter()
                    .map(|[a, b]| [a.to_array(), b.to_array()])
                    .collect()
            }
            None => Vec::new(),
        };
        let bbox = if boxes.is_empty() {
            Value::Null
        } else {
            let lo = boxes.iter().fold(DVec3::splat(f64::MAX), |a, b| a.min(*b));
            let hi = boxes.iter().fold(DVec3::splat(f64::MIN), |a, b| a.max(*b));
            json!({ "min": lo.to_array(), "max": hi.to_array() })
        };
        let reference = {
            let doc = self.asm.doc.reference.clone();
            let state = self.asm.reference_state().clone();
            views::scene(&doc, &state).map_err(fail)?
        };
        Ok(json!({
            "kind": "assembly",
            "parts": parts,
            "instances": self.instances_json(),
            "explode_lines": lines,
            "exploded": self.explode.is_some(),
            "datums": reference["datums"],
            "bbox": bbox,
        }))
    }

    fn tree(&mut self) -> Value {
        let instances = self.asm.instances();
        let diagnostics = self.asm.diagnostics.clone();
        let doc = self.asm.doc.clone();
        let node = |i: &Instance| {
            let key = i.key.join("/");
            let dof = diagnostics.dof.get(&key).copied();
            json!({
                "key": i.key,
                "name": i.name,
                "file": i.file,
                "kind": match &i.loaded {
                    Loaded::Part(_) => "part",
                    Loaded::Assembly(_) => "assembly",
                    Loaded::Missing(_) => "missing",
                },
                "error": match &i.loaded {
                    Loaded::Missing(e) => Some(e.clone()),
                    Loaded::Part(p) => p.error.clone(),
                    Loaded::Assembly(_) => None,
                },
                "fixed": i.fixed,
                "dof": dof,
                "under_defined": dof.is_some_and(|d| d > 0),
                "hidden": i.hidden,
                "derived": i.derived,
                "configuration": i.configuration,
            })
        };
        let mut out: Vec<Value> = Vec::new();
        for c in &doc.components {
            let key = c.id.to_string();
            let suppressed = doc.is_suppressed(c);
            let mut v = match instances.iter().find(|i| i.key == [key.clone()]) {
                Some(i) => node(i),
                None => json!({
                    "key": [key], "name": c.name, "file": self.asm.file_of(c),
                    "kind": "suppressed", "fixed": c.fixed,
                }),
            };
            v["suppressed"] = json!(suppressed);
            v["flexible"] = json!(c.flexible);
            v["exclude_from_bom"] = json!(c.exclude_from_bom);
            v["children"] = json!(children(&instances, std::slice::from_ref(&key), &node));
            out.push(v);
        }
        for i in instances
            .iter()
            .filter(|i| i.derived.is_some() && i.key.len() == 1)
        {
            let mut v = node(i);
            v["children"] = json!(children(&instances, &i.key, &node));
            out.push(v);
        }
        Value::Array(out)
    }

    fn snapshot(&mut self) -> Value {
        let d = self.asm.diagnostics.clone();
        let doc = self.asm.doc.clone();
        let mates: Vec<Value> = doc
            .mates
            .iter()
            .map(|m| {
                json!({
                    "id": m.id,
                    "name": m.name,
                    "type": of_assembly::mate_base_name(&m.kind),
                    "definition": m.kind,
                    "suppressed": doc.is_mate_suppressed(m),
                    "error": d.mate_errors.get(&m.id),
                    "conflicting": d.conflicting.contains(&m.id),
                    "redundant": d.redundant.contains(&m.id),
                    "components": of_assembly::mate_components(&m.kind),
                })
            })
            .collect();
        let reference: Vec<Value> = doc
            .reference
            .features
            .iter()
            .map(|f| json!({ "id": f.id, "name": f.name, "type": f.kind.base_name() }))
            .collect();
        json!({
            "kind": "assembly",
            "path": self.asm.path(),
            "dirty": self.dirty,
            "can_undo": !self.undo.is_empty(),
            "can_redo": !self.redo.is_empty(),
            "units": doc.units,
            "components": self.tree(),
            "mates": mates,
            "patterns": doc.patterns,
            "mirrors": doc.mirrors,
            "reference": reference,
            "configurations": {
                "active": doc.active_configuration,
                "items": doc.configurations,
            },
            "explode": {
                "steps": doc.active().explode,
                "shown": self.explode,
            },
            "ignored_interferences": doc.ignored_interferences,
            "diagnostics": {
                "solved": d.solved,
                "free": d.free,
                "component_errors": d.component_errors,
            },
            // Part fields clients expect on every document.
            "features": [],
            "editing": null,
        })
    }

    pub fn full(&mut self) -> R {
        let mut v = self.snapshot();
        v["scene"] = self.scene()?;
        Ok(v)
    }
}

impl Default for AsmSession {
    fn default() -> Self {
        AsmSession::new()
    }
}

fn children(
    instances: &[Instance],
    parent: &[String],
    node: &dyn Fn(&Instance) -> Value,
) -> Vec<Value> {
    instances
        .iter()
        .filter(|i| i.key.len() == parent.len() + 1 && i.key.starts_with(parent))
        .map(|i| {
            let mut v = node(i);
            v["children"] = json!(children(instances, &i.key, node));
            v
        })
        .collect()
}

pub fn geometry_json(g: &of_assembly::geometry::Geo) -> Value {
    use of_assembly::geometry::Geo;
    let v = |p: &DVec3| p.to_array();
    match g {
        Geo::Point(p) => json!({ "type": "point", "point": v(p) }),
        Geo::Line { origin, dir, ends } => json!({
            "type": "line", "origin": v(origin), "dir": v(dir),
            "ends": ends.map(|(a, b)| [a.to_array(), b.to_array()]),
        }),
        Geo::Plane {
            origin,
            normal,
            center,
            ..
        } => json!({
            "type": "plane", "origin": v(origin), "normal": v(normal), "center": v(center),
        }),
        Geo::Cylinder {
            origin,
            dir,
            radius,
        } => json!({
            "type": "cylinder", "origin": v(origin), "dir": v(dir), "radius": radius,
        }),
        Geo::Cone { origin, dir } => json!({ "type": "cone", "origin": v(origin), "dir": v(dir) }),
        Geo::Sphere { center, radius } => {
            json!({ "type": "sphere", "center": v(center), "radius": radius })
        }
        Geo::Circle {
            center,
            normal,
            radius,
        } => json!({
            "type": "circle", "center": v(center), "normal": v(normal), "radius": radius,
        }),
        Geo::Curve { points, closed } => {
            json!({ "type": "curve", "points": points.len(), "closed": closed })
        }
    }
}

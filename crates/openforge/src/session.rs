//! One open part and the methods clients call on it.

use std::path::PathBuf;

use of_model::geom::{DVec2, Kind};
use of_model::sketch::{self as sk, Geometry, Id, Sketch, tools};
use of_model::{
    Document, FeatureId, FeatureKind, Import, ImportFormat, Model, Ref, SketchFeature, State,
    Units, file, materials, measure,
};
use of_protocol::RpcError;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::tools::params::*;
use crate::views;

mod drafts;

const UNDO_LIMIT: usize = 100;

pub struct Session {
    generation: String,
    drafts: std::collections::BTreeMap<String, drafts::Draft>,
    committed: std::collections::BTreeMap<String, bool>,
    revision: u64,
    model: Model,
    path: Option<PathBuf>,
    dirty: bool,
    undo: Vec<Document>,
    redo: Vec<Document>,
    /// Feature being edited: the model is shown as it was just before it.
    editing: Option<FeatureId>,
}

type R = Result<Value, RpcError>;

fn args<T: DeserializeOwned>(params: Value) -> Result<T, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    serde_json::from_value(params).map_err(|e| RpcError::params(e.to_string()))
}

fn fail(e: impl std::fmt::Display) -> RpcError {
    RpcError::failed(e.to_string())
}

fn v2(a: [f64; 2]) -> DVec2 {
    DVec2::new(a[0], a[1])
}

impl PointArg {
    fn get(&self, s: &mut Sketch) -> Result<Id, RpcError> {
        match self {
            PointArg::Id { id } => {
                s.point(*id).map_err(fail)?;
                Ok(*id)
            }
            PointArg::At { at } => Ok(s.add_point(v2(*at))),
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Session::new()
    }
}

impl Session {
    pub fn new() -> Session {
        Session {
            generation: drafts::generation(),
            drafts: Default::default(),
            committed: Default::default(),
            revision: 0,
            model: Model::new(Document::new()),
            path: None,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            editing: None,
        }
    }

    pub fn open(path: PathBuf) -> Result<Session, String> {
        let doc = file::load(&path)?;
        let mut s = Session::new();
        s.model = Model::new(doc);
        s.path = Some(path);
        Ok(s)
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Snapshot the document before changing it.
    fn checkpoint(&mut self) {
        self.undo.push(self.model.doc.clone());
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    fn replace_doc(&mut self, doc: Document) {
        self.model = Model::new(doc);
        self.editing = None;
    }

    /// The model as shown: before the feature being edited, else at the rollback bar.
    fn display(&mut self) -> &State {
        match self.editing {
            Some(id) if self.model.doc.feature(id).is_some() => self.model.state_before(id),
            _ => self.model.state(),
        }
    }

    /// The model the user is picking on. Previews are drawn over it but
    /// never picked, so picked references always exist in the real model.
    fn visible(&mut self) -> State {
        self.display().clone()
    }

    fn snapshot(&mut self) -> Value {
        self.model.rebuild();
        let mut v = views::snapshot(&self.model);
        v["path"] = json!(self.path);
        v["dirty"] = json!(self.dirty);
        v["can_undo"] = json!(!self.undo.is_empty());
        v["can_redo"] = json!(!self.redo.is_empty());
        v["editing"] = json!(self.editing);
        v
    }

    /// Snapshot plus the scene to draw.
    fn full(&mut self) -> R {
        let mut v = self.snapshot();
        let doc = self.model.doc.clone();
        let state = self.display().clone();
        v["scene"] = views::scene(&doc, &state).map_err(fail)?;
        Ok(v)
    }

    fn sketch_of(&mut self, id: FeatureId) -> Result<&mut Sketch, RpcError> {
        match self.model.doc.feature_mut(id).map(|f| &mut f.kind) {
            Some(FeatureKind::Sketch(s)) => Ok(&mut s.sketch),
            _ => Err(RpcError::params(format!("feature {id} is not a sketch"))),
        }
    }

    fn sketch_view(&mut self, id: FeatureId) -> R {
        self.model.rebuild();
        if (!self.model.doc.equations.is_empty()
            || self.model.doc.configurations != of_model::configurations::Configurations::default())
            && let Some((plane, sketch)) =
                self.model.state_after(id).and_then(|s| s.sketches.get(&id))
        {
            let sketch = sketch.clone();
            let report = sketch.analyse();
            return Ok(views::sketch(id, plane, &sketch, &report));
        }
        let plane_ref = match self.model.doc.feature(id).map(|f| &f.kind) {
            Some(FeatureKind::Sketch(s)) => s.plane.clone(),
            _ => return Err(RpcError::params(format!("feature {id} is not a sketch"))),
        };
        let before = self.model.state_before(id).clone();
        let plane = of_model_resolve_plane(&before, &plane_ref)?;
        let sketch = self.sketch_of(id)?;
        let report = sketch.analyse();
        Ok(views::sketch(id, &plane, sketch, &report))
    }

    /// Handle one method call.
    pub fn handle(&mut self, method: &str, params: Value) -> R {
        let before = self.model.doc.clone();
        let editing = self.editing;
        let revision = self.revision;
        let mut result = self.handle_inner(method, params);
        // Keep revisions monotonic even when new/open replaces the session.
        let changed = self.model.doc != before
            || self.editing != editing
            || (result.is_ok() && matches!(method, "doc.new" | "doc.open"));
        self.revision = revision + u64::from(changed);
        if let Ok(Value::Object(ref mut object)) = result {
            object.insert("revision".into(), json!(self.revision));
            object.insert("generation".into(), json!(self.generation));
            if let Some(Value::Object(sketch)) = object.get_mut("sketch") {
                sketch.insert("revision".into(), json!(self.revision));
            }
        }
        result
    }

    fn handle_inner(&mut self, method: &str, params: Value) -> R {
        if method.starts_with("draft.") {
            return self.draft_request(method, params);
        }
        match method {
            "assistant.tools" => Ok(crate::tools::assistant_catalog()),
            "doc.feature" => {
                let FeatureRef { id } = args(params)?;
                Ok(
                    json!({"feature":self.model.doc.feature(id).ok_or_else(|| RpcError::params("no such feature"))?}),
                )
            }
            "parameter.set" => {
                let p: SetParameter = args(params)?;
                let target = self
                    .model
                    .doc
                    .features
                    .iter()
                    .flat_map(of_model::equations::parameters)
                    .find(|t| t.key == p.key)
                    .ok_or_else(|| RpcError::params("unknown parameter"))?;
                let mut doc = self.model.doc.clone();
                of_model::equations::set_parameter(
                    doc.feature_mut(target.feature).unwrap(),
                    &target,
                    of_model::equations::Quantity {
                        value: p.value,
                        ..target.value
                    },
                )
                .map_err(fail)?;
                doc.equations.remove(&p.key);
                self.checkpoint();
                self.model.doc = doc;
                self.model.rebuild_all();
                self.full()
            }
            "configuration.create"
            | "configuration.update"
            | "configuration.delete"
            | "configuration.activate" => {
                use of_model::configurations::Configuration;
                let mut trial = self.model.doc.clone();
                let configs = &mut trial.configurations;
                match method {
                    "configuration.create" => {
                        let p: CreateConfiguration = args(params)?;
                        let id = configs.next_id;
                        configs.next_id = id
                            .checked_add(1)
                            .ok_or_else(|| fail("configuration IDs exhausted"))?;
                        let name = p.name;
                        let mut config = if let Some(source) = p.copy_from {
                            configs
                                .items
                                .iter()
                                .find(|c| c.id == source)
                                .cloned()
                                .ok_or_else(|| RpcError::params("no such source configuration"))?
                        } else {
                            Configuration::new(id, name.clone())
                        };
                        config.id = id;
                        config.name = name.trim().into();
                        configs.items.push(config);
                        configs.active = id;
                    }
                    "configuration.update" => {
                        let UpdateConfiguration { mut configuration } = args(params)?;
                        configuration.name = configuration.name.trim().into();
                        let target = configs
                            .items
                            .iter_mut()
                            .find(|c| c.id == configuration.id)
                            .ok_or_else(|| RpcError::params("no such configuration"))?;
                        *target = configuration;
                    }
                    "configuration.delete" => {
                        let ConfigurationRef { id } = args(params)?;
                        if !configs.items.iter().any(|c| c.id == id) {
                            return Err(RpcError::params("no such configuration"));
                        }
                        if configs.items.len() == 1 {
                            return Err(RpcError::params("keep at least one configuration"));
                        }
                        configs.items.retain(|c| c.id != id);
                        if configs.active == id {
                            configs.active = configs.items[0].id;
                        }
                    }
                    _ => {
                        let ConfigurationRef { id } = args(params)?;
                        configs.active = id;
                    }
                }
                trial.validate().map_err(RpcError::params)?;
                self.checkpoint();
                self.model.doc = trial;
                self.model.rebuild_all();
                self.editing = None;
                self.full()
            }
            "equations.set" | "equations.preview" => {
                let p: SetEquations = args(params)?;
                let mut trial = self.model.doc.clone();
                trial.variables = p.variables;
                trial.equations = p.equations;
                trial.validate().map_err(RpcError::params)?;
                let keys: std::collections::HashSet<_> = trial
                    .features
                    .iter()
                    .flat_map(of_model::equations::parameters)
                    .map(|p| p.key)
                    .collect();
                if let Some(key) = trial.equations.keys().find(|key| !keys.contains(*key)) {
                    return Err(RpcError::params(format!(
                        "parameter {key} no longer exists"
                    )));
                }
                if method == "equations.preview" {
                    return Ok(json!({ "values": views::equation_values(&trial) }));
                }
                self.checkpoint();
                self.model.doc = trial;
                self.model.rebuild_all();
                self.editing = None;
                self.full()
            }
            "core.info" => Ok(json!(of_protocol::CoreInfo {
                protocol_version: of_protocol::PROTOCOL_VERSION,
                scene_version: of_protocol::SCENE_VERSION,
                part_file_version: file::VERSION,
                capabilities: vec![
                    "revisioned_preview".into(),
                    "assistant_drafts".into(),
                    "part_v1_read".into(),
                    "multibody".into(),
                    "equations".into(),
                    "configurations".into(),
                    "curves".into(),
                    "sweep_loft".into(),
                    "draft".into(),
                    "variable_fillet".into(),
                    "split_line".into(),
                ],
            })),
            "body.display" => {
                let p: BodyDisplay = args(params)?;
                self.display().solid(&p.id).map_err(fail)?;
                if p.name.as_ref().is_some_and(|n| n.trim().is_empty()) {
                    return Err(RpcError::params("the body name cannot be empty"));
                }
                self.checkpoint();
                let display = self.model.doc.body_display.entry(p.id).or_default();
                if let Some(name) = p.name {
                    display.name = Some(name.trim().into());
                }
                if let Some(hidden) = p.hidden {
                    display.hidden = hidden;
                }
                self.full()
            }
            "doc.info" => self.full(),
            "doc.new" => {
                *self = Session::new();
                self.full()
            }
            "doc.open" => {
                let p: FilePath = args(params)?;
                *self = Session::open(p.path).map_err(fail)?;
                self.full()
            }
            "doc.save" => {
                let p: SavePath = args(params)?;
                let path = p
                    .path
                    .or(self.path.clone())
                    .ok_or_else(|| RpcError::params("choose where to save"))?;
                file::save(&self.model.doc, &path).map_err(fail)?;
                self.path = Some(path);
                self.dirty = false;
                Ok(self.snapshot())
            }
            "doc.undo" | "doc.redo" => {
                let (from, to) = if method == "doc.undo" {
                    (&mut self.undo, &mut self.redo)
                } else {
                    (&mut self.redo, &mut self.undo)
                };
                let Some(doc) = from.pop() else {
                    return self.full();
                };
                to.push(self.model.doc.clone());
                self.dirty = true;
                self.replace_doc(doc);
                self.full()
            }
            "doc.set_units" => {
                #[derive(Deserialize)]
                struct P {
                    units: Units,
                }
                let p: P = args(params)?;
                self.checkpoint();
                self.model.doc.units = p.units;
                Ok(self.snapshot())
            }
            "doc.set_material" => {
                let p: SetMaterial = args(params)?;
                let m = match (p.name, p.density) {
                    (None, _) => None,
                    (Some(n), Some(d)) => Some(materials::Material {
                        name: n,
                        density: d,
                    }),
                    (Some(n), None) => Some(
                        materials::by_name(&n)
                            .ok_or_else(|| RpcError::params(format!("unknown material {n}")))?,
                    ),
                };
                self.checkpoint();
                self.model.doc.material = m;
                Ok(self.snapshot())
            }
            "doc.rollback" => {
                #[derive(Deserialize)]
                struct P {
                    index: Option<usize>,
                }
                let p: P = args(params)?;
                self.checkpoint();
                self.model.doc.rollback = p.index.filter(|i| *i < self.model.doc.features.len());
                self.full()
            }
            "doc.topology" => self.topology(),
            "feature.add" => {
                let p: AddFeature = args(params)?;
                self.checkpoint();
                let id = self.model.doc.add(p.feature.kind);
                self.model.doc.feature_mut(id).unwrap().scope = p.feature.scope;
                if let Some(n) = p.name {
                    self.model.doc.feature_mut(id).unwrap().name = n;
                }
                self.model.invalidate(id);
                self.editing = None;
                let mut v = self.full()?;
                v["id"] = json!(id);
                Ok(v)
            }
            "feature.update" => {
                let p: UpdateFeature = args(params)?;
                self.checkpoint();
                let f = self
                    .model
                    .doc
                    .feature_mut(p.id)
                    .ok_or_else(|| RpcError::params("no such feature"))?;
                f.kind = p.feature.kind;
                f.scope = p.feature.scope;
                self.model.invalidate(p.id);
                self.editing = None;
                self.full()
            }
            "feature.delete" => {
                let p: DeleteFeatures = args(params)?;
                self.checkpoint();
                let first = p
                    .ids
                    .iter()
                    .filter_map(|id| self.model.doc.index_of(*id))
                    .min()
                    .unwrap_or(0);
                let deleted = self.model.doc.delete(&p.ids);
                self.model.invalidate_from(first);
                self.editing = None;
                let mut v = self.full()?;
                v["deleted"] = json!(deleted);
                Ok(v)
            }
            "feature.rename" => {
                #[derive(Deserialize)]
                struct P {
                    id: FeatureId,
                    name: String,
                }
                let p: P = args(params)?;
                if p.name.trim().is_empty() {
                    return Err(RpcError::params("the name can't be empty"));
                }
                self.checkpoint();
                self.model
                    .doc
                    .feature_mut(p.id)
                    .ok_or_else(|| RpcError::params("no such feature"))?
                    .name = p.name.trim().into();
                Ok(self.snapshot())
            }
            "feature.suppress" => {
                #[derive(Deserialize)]
                struct P {
                    id: FeatureId,
                    suppressed: bool,
                }
                let p: P = args(params)?;
                self.checkpoint();
                self.model
                    .doc
                    .feature_mut(p.id)
                    .ok_or_else(|| RpcError::params("no such feature"))?
                    .suppressed = p.suppressed;
                self.model.invalidate(p.id);
                self.full()
            }
            "feature.reorder" => {
                #[derive(Deserialize)]
                struct P {
                    id: FeatureId,
                    to: usize,
                }
                let p: P = args(params)?;
                let from = self
                    .model
                    .doc
                    .index_of(p.id)
                    .ok_or_else(|| RpcError::params("no such feature"))?;
                let before = self.model.doc.clone();
                self.model.doc.reorder(p.id, p.to).map_err(fail)?;
                self.undo.push(before);
                self.redo.clear();
                self.dirty = true;
                self.model.invalidate_from(from.min(p.to));
                self.full()
            }
            "feature.preview" => {
                #[derive(Deserialize)]
                struct P {
                    feature: Option<FeatureInput>,
                    editing: Option<FeatureId>,
                    revision: Option<u64>,
                }
                let p: P = args(params)?;
                if p.revision.is_some_and(|r| r != self.revision) {
                    return Err(fail("the document changed; refresh before previewing"));
                }
                let Some(kind) = p.feature else {
                    return Ok(json!({ "scene": null, "error": null }));
                };
                match self
                    .model
                    .preview_scoped(&kind.kind, p.editing, kind.scope.as_deref())
                {
                    Ok(state) => {
                        let scene = views::scene(&self.model.doc, &state).map_err(fail)?;
                        Ok(json!({ "scene": scene, "error": null }))
                    }
                    Err(e) => Ok(json!({ "scene": null, "error": e })),
                }
            }
            "edit.begin" => {
                #[derive(Deserialize)]
                struct P {
                    id: FeatureId,
                }
                let p: P = args(params)?;
                self.model
                    .doc
                    .feature(p.id)
                    .ok_or_else(|| RpcError::params("no such feature"))?;
                self.editing = Some(p.id);
                self.full()
            }
            "edit.end" => {
                self.editing = None;
                self.full()
            }
            "sketch.new" => {
                let p: NewSketch = args(params)?;
                // Check the plane resolves before adding anything.
                let state = self.visible();
                of_model_resolve_plane(&state, &p.plane)?;
                self.checkpoint();
                let id = self.model.doc.add(FeatureKind::Sketch(SketchFeature {
                    plane: p.plane,
                    sketch: Sketch::new(),
                }));
                self.model.invalidate(id);
                self.editing = Some(id);
                let mut v = self.full()?;
                v["id"] = json!(id);
                v["sketch"] = self.sketch_view(id)?;
                Ok(v)
            }
            "sketch.get" => {
                let p: FeatureRef = args(params)?;
                self.sketch_view(p.id)
            }
            "sketch.edit" => {
                let p: EditSketch = args(params)?;
                let created = self.sketch_edit(p.id, p.op)?;
                let mut v = self.sketch_view(p.id)?;
                v["created"] = json!(created);
                Ok(v)
            }
            "sketch.close" => {
                self.editing = None;
                self.full()
            }
            "select.ref" => {
                #[derive(Deserialize)]
                struct P {
                    kind: Kind,
                    index: u32,
                }
                let p: P = args(params)?;
                let state = self.visible();
                let r = state
                    .qualified_ref_for(p.kind, p.index)
                    .ok_or_else(|| RpcError::params("nothing there"))?;
                Ok(json!({ "ref": r }))
            }
            "select.locate" => {
                // Kernel index of a reference in the visible model, for highlighting.
                #[derive(Deserialize)]
                struct P {
                    r#ref: Ref,
                }
                let p: P = args(params)?;
                let state = self.visible();
                match state.index_of(&p.r#ref) {
                    Ok((kind, index)) => Ok(json!({ "kind": kind, "index": index })),
                    Err(_) => Ok(json!(null)),
                }
            }
            "select.plane" => {
                // The plane of a planar face or reference, for "Normal To".
                #[derive(Deserialize)]
                struct P {
                    r#ref: Ref,
                }
                let p: P = args(params)?;
                let state = self.visible();
                match of_model::resolve(&state, &p.r#ref).and_then(|r| r.plane()) {
                    Ok(pl) => Ok(
                        json!({ "origin": [pl.origin.x, pl.origin.y, pl.origin.z], "x_dir": [pl.x_dir.x, pl.x_dir.y, pl.x_dir.z], "normal": [pl.normal.x, pl.normal.y, pl.normal.z] }),
                    ),
                    Err(_) => Ok(json!(null)),
                }
            }
            "measure" => {
                let p: Measure = args(params)?;
                let state = self.visible();
                Ok(json!(measure::measure(&state, &p.refs).map_err(fail)?))
            }
            "mass_properties" => {
                let state = self.model.state().clone();
                Ok(json!(
                    measure::mass_properties(&self.model.doc, &state).map_err(fail)?
                ))
            }
            "export" => {
                let p: Export = args(params)?;
                let format = p
                    .format
                    .or_else(|| of_io::ExportFormat::from_path(&p.path))
                    .ok_or_else(|| RpcError::params("export as .step, .iges or .stl"))?;
                let export_model;
                let model = if let Some(configuration) = p.configuration {
                    let mut doc = self.model.doc.clone();
                    doc.configurations.active = configuration;
                    doc.validate().map_err(RpcError::params)?;
                    export_model = Model::new(doc);
                    &export_model
                } else {
                    &self.model
                };
                if model
                    .statuses()
                    .iter()
                    .any(|s| !s.rolled_back && !s.suppressed && s.error.is_some())
                {
                    return Err(fail(
                        "resolve feature errors before exporting this configuration",
                    ));
                }
                let body = model
                    .state()
                    .body
                    .as_ref()
                    .ok_or_else(|| fail("there's nothing to export"))?;
                of_io::export(&body.shape, &p.path, format).map_err(fail)?;
                Ok(json!({ "path": p.path }))
            }
            "import" => {
                let p: FilePath = args(params)?;
                let data = of_io::read_import(&p.path).map_err(fail)?;
                let format = match data.kind {
                    of_io::ImportKind::Step => ImportFormat::Step,
                    of_io::ImportKind::Iges => ImportFormat::Iges,
                };
                self.checkpoint();
                let id = self.model.doc.add(FeatureKind::Import(Import {
                    format,
                    source: data.source.clone(),
                    data: data.data,
                }));
                self.model.doc.feature_mut(id).unwrap().name = data.source;
                self.model.invalidate(id);
                let mut v = self.full()?;
                v["id"] = json!(id);
                Ok(v)
            }
            "hole.sizes" => {
                let iso: Vec<&str> = of_model::holes::ISO.iter().map(|s| s.name).collect();
                let ansi: Vec<&str> = of_model::holes::ANSI.iter().map(|s| s.name).collect();
                Ok(json!({ "iso": iso, "ansi": ansi }))
            }
            "hole.dims" => {
                let h: of_model::Hole = args(params)?;
                let d = of_model::hole_dims(&h).map_err(fail)?;
                Ok(json!({ "diameter": d.diameter, "cbore": d.cbore, "csink": d.csink }))
            }
            "materials.list" => {
                let list: Vec<Value> = materials::LIBRARY
                    .iter()
                    .map(|(n, d)| json!({ "name": n, "density": d }))
                    .collect();
                Ok(json!(list))
            }
            "units.parse" => {
                #[derive(Deserialize)]
                struct P {
                    text: String,
                    #[serde(default)]
                    angle: bool,
                }
                let p: P = args(params)?;
                let u = self.model.doc.units;
                let v = if p.angle {
                    u.parse_angle(&p.text)
                } else {
                    u.parse_length(&p.text)
                };
                Ok(json!(v))
            }
            _ => Err(RpcError::new(
                RpcError::METHOD_NOT_FOUND,
                format!("unknown method {method}"),
            )),
        }
    }

    fn sketch_edit(&mut self, id: FeatureId, op: SketchOp) -> Result<Vec<Id>, RpcError> {
        // Dragging is continuous: no undo step and no downstream rebuild yet.
        let is_drag = matches!(op, SketchOp::Drag { .. });
        let convert_state = if let SketchOp::Convert { .. } = op {
            Some(self.model.state_before(id).clone())
        } else {
            None
        };
        let plane = match &convert_state {
            Some(state) => {
                let plane_ref = match &self.model.doc.feature(id).map(|f| &f.kind) {
                    Some(FeatureKind::Sketch(s)) => s.plane.clone(),
                    _ => return Err(RpcError::params(format!("feature {id} is not a sketch"))),
                };
                Some(of_model_resolve_plane(state, &plane_ref)?)
            }
            None => None,
        };
        if !is_drag {
            self.checkpoint();
        }
        let s = self.sketch_of(id)?;
        let before = s.clone();
        let mut created: Vec<Id> = Vec::new();
        let result: Result<(), RpcError> = (|| {
            match op {
                SketchOp::AddPoint { at } => created.push(s.add_point(v2(at))),
                SketchOp::AddLine { a, b } => {
                    let (a, b) = (a.get(s)?, b.get(s)?);
                    if (s.point(a).unwrap() - s.point(b).unwrap()).length() < 1e-9 {
                        return Err(RpcError::params("the line has no length"));
                    }
                    created.push(s.add_line(a, b).map_err(fail)?);
                    created.extend([a, b]);
                }
                SketchOp::AddCircle { center, radius } => {
                    if radius <= 0.0 {
                        return Err(RpcError::params("the radius must be more than zero"));
                    }
                    let c = center.get(s)?;
                    created.push(s.add_circle(c, radius).map_err(fail)?);
                }
                SketchOp::AddArc3 { start, end, via } => {
                    let (a, b) = (start.get(s)?, end.get(s)?);
                    created.push(tools::three_point_arc(s, a, b, v2(via)).map_err(fail)?);
                }
                SketchOp::AddCenterArc {
                    center,
                    start,
                    end,
                    clockwise,
                } => {
                    let c = center.get(s)?;
                    let st = start.get(s)?;
                    let (cp, sp) = (s.point(c).unwrap(), s.point(st).unwrap());
                    let r = (sp - cp).length();
                    let dir = (v2(end) - cp).normalize_or(DVec2::X);
                    let e = s.add_point(cp + dir * r);
                    let arc = if clockwise {
                        s.add_arc(c, e, st)
                    } else {
                        s.add_arc(c, st, e)
                    };
                    created.push(arc.map_err(fail)?);
                    created.push(e);
                }
                SketchOp::AddTangentArc {
                    start,
                    tangent,
                    end,
                } => {
                    let e = end.get(s)?;
                    created.push(tools::tangent_arc(s, start, v2(tangent), e).map_err(fail)?);
                }
                SketchOp::AddSpline { points, closed } => {
                    let ids: Vec<Id> = points.iter().map(|p| p.get(s)).collect::<Result<_, _>>()?;
                    created.push(s.add_spline(ids, closed).map_err(fail)?);
                }
                SketchOp::Rectangle { a, b, center } => {
                    let out = if center {
                        tools::center_rectangle(s, v2(a), v2(b))
                    } else {
                        tools::corner_rectangle(s, v2(a), v2(b))
                    };
                    created = out.entities;
                }
                SketchOp::Slot { a, b, width } => {
                    created = tools::straight_slot(s, v2(a), v2(b), width)
                        .map_err(fail)?
                        .entities
                }
                SketchOp::Polygon {
                    center,
                    vertex,
                    sides,
                } => {
                    created = tools::polygon(s, v2(center), v2(vertex), sides)
                        .map_err(fail)?
                        .entities
                }
                SketchOp::Constraint { constraint } => {
                    created.push(s.add_constraint(constraint).map_err(fail)?)
                }
                SketchOp::Dimension { constraint, value } => {
                    let cid = s.add_measured(constraint).map_err(fail)?;
                    if let Some(v) = value {
                        s.set_value(cid, v).map_err(fail)?;
                    }
                    created.push(cid);
                }
                SketchOp::SetValue { id: cid, value } => s.set_value(cid, value).map_err(fail)?,
                SketchOp::SetDriving { id: cid, driving } => {
                    s.set_driving(cid, driving).map_err(fail)?
                }
                SketchOp::Delete { ids } => s.delete(&ids),
                SketchOp::Drag { point, to } => {
                    s.point(point).map_err(fail)?;
                    s.drag(point, v2(to));
                    return Ok(());
                }
                SketchOp::Move { ids, delta } => {
                    let mut pts: Vec<Id> = Vec::new();
                    for i in &ids {
                        let e = s.entity(*i).map_err(fail)?;
                        match e.geo {
                            Geometry::Point { .. } => pts.push(*i),
                            _ => pts.extend(e.point_refs()),
                        }
                    }
                    pts.sort_unstable();
                    pts.dedup();
                    for p in pts.into_iter().filter(|p| *p != sk::ORIGIN) {
                        let q = s.point(p).unwrap();
                        s.set_point(p, q + v2(delta)).unwrap();
                    }
                }
                SketchOp::Trim { entity, pick } => {
                    created = tools::trim(s, entity, v2(pick)).map_err(fail)?.entities
                }
                SketchOp::Extend { entity, pick } => {
                    created = tools::extend(s, entity, v2(pick)).map_err(fail)?.entities
                }
                SketchOp::Offset { ids, distance } => {
                    created = tools::offset(s, &ids, distance).map_err(fail)?.entities
                }
                SketchOp::Mirror { ids, about } => {
                    created = tools::mirror(s, &ids, about).map_err(fail)?.entities
                }
                SketchOp::Fillet { point, radius } => {
                    created = tools::sketch_fillet(s, point, radius)
                        .map_err(fail)?
                        .entities
                }
                SketchOp::Construction { ids, construction } => {
                    for i in ids {
                        if let Some(e) = s.entities.get_mut(&i)
                            && !matches!(e.geo, Geometry::Point { .. })
                        {
                            e.construction = construction;
                        }
                    }
                }
                SketchOp::Convert { refs } => {
                    let state = convert_state.as_ref().unwrap();
                    created = of_model::convert_edges(state, plane.as_ref().unwrap(), s, &refs)
                        .map_err(fail)?;
                }
            }
            // A failed solve keeps the change so the user can see and fix the
            // conflict; the sketch shows as over defined.
            s.solve();
            Ok(())
        })();
        if let Err(e) = result {
            *self.sketch_of(id)? = before;
            if !is_drag {
                self.undo.pop();
            }
            return Err(e);
        }
        self.model.invalidate(id);
        Ok(created)
    }

    /// Named faces and edges of the current model, for scripts and AI
    /// assistants that can't click on geometry.
    fn topology(&mut self) -> R {
        let state = self.visible();
        let Some(body) = &state.body else {
            return Ok(json!({ "faces": [], "edges": [] }));
        };
        let r3 = |v: of_model::geom::DVec3| [round(v.x), round(v.y), round(v.z)];
        let faces: Vec<Value> = (0..body.shape.count(Kind::Face))
            .filter_map(|i| {
                let f = body.shape.face_info(i).ok()?;
                Some(json!({
                    "ref": state.ref_for(Kind::Face, i)?,
                    "surface": f.kind,
                    "area": round(f.area),
                    "centroid": r3(f.centroid),
                    "normal": r3(f.normal),
                    "radius": (f.radius > 0.0).then(|| round(f.radius)),
                }))
            })
            .collect();
        let edges: Vec<Value> = (0..body.shape.count(Kind::Edge))
            .filter_map(|i| {
                let e = body.shape.edge_info(i).ok()?;
                Some(json!({
                    "ref": state.ref_for(Kind::Edge, i)?,
                    "curve": e.kind,
                    "length": round(e.length),
                    "start": r3(e.start),
                    "end": r3(e.end),
                    "radius": (e.radius > 0.0).then(|| round(e.radius)),
                }))
            })
            .collect();
        Ok(json!({ "faces": faces, "edges": edges }))
    }
}

fn round(v: f64) -> f64 {
    (v * 1e4).round() / 1e4
}

fn of_model_resolve_plane(state: &State, r: &Ref) -> Result<of_model::geom::Plane, RpcError> {
    of_model::resolve(state, r)
        .and_then(|x| x.plane())
        .map_err(fail)
}

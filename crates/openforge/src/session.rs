//! One open part and the methods clients call on it.

use std::path::{Path, PathBuf};

use of_model::geom::{DVec2, DVec3, Kind};
use of_model::sketch::{self as sk, Geometry, Id, Sketch, tools};
use of_model::{
    Document, FeatureId, FeatureKind, Import, ImportFormat, Model, Ref, SketchFeature, State,
    Units, file, materials, measure,
};
use of_protocol::{ErrorCode, RpcError};
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
    /// The open assembly, when the document is an assembly rather than a part.
    assembly: Option<Box<crate::asm::AsmSession>>,
    /// The open drawing, when the document is a drawing.
    drawing: Option<Box<crate::drawing::DrawSession>>,
}

type R = Result<Value, RpcError>;

fn args<T: DeserializeOwned>(params: Value) -> Result<T, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    serde_json::from_value(params).map_err(|e| RpcError::params(e.to_string()))
}

fn fail(e: impl std::fmt::Display) -> RpcError {
    RpcError::failed(e.to_string())
}

/// Sketch errors that name a missing or mistyped entity get a stable code.
fn sketch_fail(e: sk::SketchError) -> RpcError {
    let code = match e {
        sk::SketchError::NoEntity(_) | sk::SketchError::WrongKind(..) => ErrorCode::UnknownEntity,
        sk::SketchError::Invalid(_) => ErrorCode::Failed,
    };
    RpcError::coded(code, e.to_string())
}

fn v2(a: [f64; 2]) -> DVec2 {
    DVec2::new(a[0], a[1])
}

impl PointArg {
    fn get(&self, s: &mut Sketch) -> Result<Id, RpcError> {
        match self {
            PointArg::Id { id } => {
                s.point(*id).map_err(sketch_fail)?;
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
            assembly: None,
            drawing: None,
        }
    }

    pub fn open(path: PathBuf) -> Result<Session, String> {
        if crate::drawing::is_drawing(&path) {
            let mut s = Session::new();
            s.drawing = Some(Box::new(crate::drawing::DrawSession::open(&path)?));
            return Ok(s);
        }
        if of_assembly::file::is_assembly(&path) {
            let mut s = Session::new();
            s.assembly = Some(Box::new(crate::asm::AsmSession::open(&path)?));
            return Ok(s);
        }
        let doc = file::load(&path)?;
        let mut s = Session::new();
        s.model = Model::new(doc);
        s.path = Some(path);
        Ok(s)
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    pub(crate) fn model_mut(&mut self) -> &mut Model {
        &mut self.model
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn generation(&self) -> &str {
        &self.generation
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
            _ => Err(RpcError::coded(
                ErrorCode::NotASketch,
                format!("feature {id} is not a sketch"),
            )),
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
            _ => {
                return Err(RpcError::coded(
                    ErrorCode::NotASketch,
                    format!("feature {id} is not a sketch"),
                ));
            }
        };
        let before = self.model.state_before(id).clone();
        let plane = of_model_resolve_plane(&before, &plane_ref)?;
        let sketch = self.sketch_of(id)?;
        let report = sketch.analyse();
        Ok(views::sketch(id, &plane, sketch, &report))
    }

    /// The open assembly, if the document is one.
    pub fn assembly(&self) -> Option<&crate::asm::AsmSession> {
        self.assembly.as_deref()
    }

    /// The open drawing, if the document is one.
    pub fn drawing(&self) -> Option<&crate::drawing::DrawSession> {
        self.drawing.as_deref()
    }

    /// Methods an open drawing answers besides `drawing.*`.
    fn for_drawing(method: &str) -> bool {
        method.starts_with("drawing.")
            || matches!(
                method,
                "doc.info" | "doc.save" | "doc.undo" | "doc.redo" | "export"
            )
    }

    /// Methods an open assembly answers; the rest stay with parts.
    fn for_assembly(method: &str) -> bool {
        method.starts_with("asm.")
            || matches!(
                method,
                "doc.info"
                    | "doc.save"
                    | "doc.undo"
                    | "doc.redo"
                    | "doc.set_units"
                    | "export"
                    | "measure"
                    | "mass_properties"
            )
    }

    /// Methods that work the same whatever the document is.
    fn for_any(method: &str) -> bool {
        matches!(
            method,
            "core.info"
                | "materials.list"
                | "units.parse"
                | "hole.sizes"
                | "hole.dims"
                | "assistant.tools"
        )
    }

    /// Open an assembly file, or start a new assembly.
    fn begin_assembly(&mut self, path: Option<PathBuf>) -> R {
        let a = match path {
            Some(p) => crate::asm::AsmSession::open(&p).map_err(fail)?,
            None => crate::asm::AsmSession::new(),
        };
        *self = Session::new();
        self.assembly = Some(Box::new(a));
        self.assembly.as_mut().unwrap().full()
    }

    /// Handle one method call.
    pub fn handle(&mut self, method: &str, params: Value) -> R {
        // Assemblies: new and open switch document kind; the open assembly
        // answers its methods with its own revision counter.
        let opens_assembly = method == "doc.open"
            && params
                .get("path")
                .and_then(Value::as_str)
                .is_some_and(|p| of_assembly::file::is_assembly(Path::new(p)));
        if method == "asm.import_step" {
            let p: crate::tools::asm_params::ImportStep = args(params)?;
            let folder = p.folder.unwrap_or_else(|| {
                let stem = p
                    .path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("import")
                    .to_string();
                p.path.with_file_name(format!("{stem} parts"))
            });
            let asm = of_assembly::exchange::import_step(&p.path, &folder).map_err(fail)?;
            *self = Session::new();
            self.assembly = Some(Box::new(crate::asm::AsmSession::from_assembly(asm)));
            let mut v = self.assembly.as_mut().unwrap().full()?;
            v["folder"] = json!(folder);
            v["revision"] = json!(self.revision);
            v["generation"] = json!(self.generation);
            return Ok(v);
        }
        if method == "asm.new" || opens_assembly {
            let path = if opens_assembly {
                params
                    .get("path")
                    .and_then(Value::as_str)
                    .map(PathBuf::from)
            } else {
                None
            };
            let revision = self.revision;
            let mut result = self.begin_assembly(path);
            self.revision = revision + 1;
            if let Ok(Value::Object(o)) = &mut result {
                o.insert("revision".into(), json!(self.revision));
                o.insert("generation".into(), json!(self.generation));
            }
            return result;
        }
        // Drawings: like assemblies, new and open switch document kind.
        let opens_drawing = method == "doc.open"
            && params
                .get("path")
                .and_then(Value::as_str)
                .is_some_and(|p| crate::drawing::is_drawing(Path::new(p)));
        if method == "drawing.new" || opens_drawing {
            let d = if opens_drawing {
                let p: FilePath = args(params)?;
                crate::drawing::DrawSession::open(&p.path).map_err(fail)?
            } else {
                crate::drawing::DrawSession::new(&args(params)?)
            };
            let revision = self.revision;
            *self = Session::new();
            self.drawing = Some(Box::new(d));
            self.revision = revision + 1;
            let mut v = self.drawing.as_ref().unwrap().full();
            v["revision"] = json!(self.revision);
            v["generation"] = json!(self.generation);
            return Ok(v);
        }
        if let Some(d) = self.drawing.as_mut()
            && !Self::for_any(method)
        {
            if method == "doc.new" || method == "doc.open" {
                self.drawing = None;
            } else if Self::for_drawing(method) {
                let before = d.revision();
                let mut result = d.handle(method, params);
                if d.revision() != before {
                    self.revision += 1;
                }
                if let Ok(Value::Object(o)) = &mut result {
                    o.insert("revision".into(), json!(self.revision));
                    o.insert("generation".into(), json!(self.generation));
                }
                return result;
            } else {
                return Err(RpcError::coded(
                    ErrorCode::NotAllowed,
                    format!(
                        "{method} is not available in a drawing; open the part or assembly to edit it"
                    ),
                ));
            }
        } else if method.starts_with("drawing.") {
            return Err(RpcError::coded(
                ErrorCode::NotAllowed,
                format!(
                    "{method} needs a drawing; start one with drawing.new or open an .ofd file"
                ),
            ));
        }
        if let Some(a) = self.assembly.as_mut()
            && !Self::for_any(method)
        {
            if method == "doc.new" || method == "doc.open" {
                self.assembly = None;
            } else if Self::for_assembly(method) {
                let before = a.revision();
                let mut result = a.handle(method, params);
                if a.revision() != before {
                    self.revision += 1;
                }
                if let Ok(Value::Object(o)) = &mut result {
                    o.insert("revision".into(), json!(self.revision));
                    o.insert("generation".into(), json!(self.generation));
                }
                return result;
            } else {
                return Err(RpcError::coded(
                    ErrorCode::NotAllowed,
                    format!("{method} needs a part; open the part from the assembly to edit it"),
                ));
            }
        } else if method.starts_with("asm.") {
            return Err(RpcError::coded(
                ErrorCode::NotAllowed,
                format!("{method} needs an assembly; start one with asm.new or open an .ofa file"),
            ));
        }
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
                    json!({"feature":self.model.doc.feature(id).ok_or_else(|| RpcError::coded(ErrorCode::UnknownFeature, "no such feature"))?}),
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
                    .ok_or_else(|| {
                        RpcError::coded(ErrorCode::UnknownParameter, "unknown parameter")
                    })?;
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
                // Only features whose effective definition changed are rebuilt.
                self.model.rebuild();
                self.full()
            }
            "configuration.create"
            | "configuration.update"
            | "configuration.preview"
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
                                .ok_or_else(|| {
                                    RpcError::coded(
                                        ErrorCode::UnknownConfiguration,
                                        "no such source configuration",
                                    )
                                })?
                        } else {
                            Configuration::new(id, name.clone())
                        };
                        config.id = id;
                        config.name = name.trim().into();
                        configs.items.push(config);
                        configs.active = id;
                    }
                    "configuration.update" | "configuration.preview" => {
                        let UpdateConfiguration {
                            mut configuration,
                            revision,
                        } = args(params)?;
                        if revision.is_some_and(|revision| revision != self.revision) {
                            return Err(RpcError::coded(
                                ErrorCode::StaleRevision,
                                "the document changed; refresh before editing a configuration",
                            ));
                        }
                        configuration.name = configuration.name.trim().into();
                        of_model::equations::check_formulas(
                            configuration.variables.iter().chain(
                                configuration
                                    .equations
                                    .iter()
                                    .filter_map(|(k, v)| v.as_ref().map(|v| (k, v))),
                            ),
                        )
                        .map_err(|e| RpcError::coded(ErrorCode::InvalidFormula, e))?;
                        let target = configs
                            .items
                            .iter_mut()
                            .find(|c| c.id == configuration.id)
                            .ok_or_else(|| {
                                RpcError::coded(
                                    ErrorCode::UnknownConfiguration,
                                    "no such configuration",
                                )
                            })?;
                        let preview_id = configuration.id;
                        *target = configuration;
                        if method == "configuration.preview" {
                            configs.active = preview_id;
                        }
                    }
                    "configuration.delete" => {
                        let ConfigurationRef { id } = args(params)?;
                        if !configs.items.iter().any(|c| c.id == id) {
                            return Err(RpcError::coded(
                                ErrorCode::UnknownConfiguration,
                                "no such configuration",
                            ));
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
                        if !configs.items.iter().any(|c| c.id == id) {
                            return Err(RpcError::coded(
                                ErrorCode::UnknownConfiguration,
                                "no such configuration",
                            ));
                        }
                        configs.active = id;
                    }
                }
                trial
                    .validate()
                    .map_err(|e| RpcError::coded(ErrorCode::InvalidFormula, e))?;
                if method == "configuration.preview" {
                    let model = Model::new(trial);
                    return Ok(json!({
                        "values": views::equation_values(&model.doc),
                        "features": views::snapshot(&model)["features"],
                        "scene": views::scene(&model.doc, model.state()).map_err(fail)?,
                    }));
                }
                self.checkpoint();
                self.model.doc = trial;
                // Only features whose effective definition changed are rebuilt.
                self.model.rebuild();
                self.editing = None;
                self.full()
            }
            "equations.set" | "equations.preview" => {
                let p: SetEquations = args(params)?;
                if p.revision.is_some_and(|revision| revision != self.revision) {
                    return Err(RpcError::coded(
                        ErrorCode::StaleRevision,
                        "the document changed; refresh before editing parameters",
                    ));
                }
                // Preview reports syntax errors per field so the editor can disable OK.
                if method == "equations.set" {
                    of_model::equations::check_formulas(
                        p.variables.iter().chain(
                            p.equations
                                .iter()
                                .filter_map(|(k, v)| v.as_ref().map(|v| (k, v))),
                        ),
                    )
                    .map_err(|e| RpcError::coded(ErrorCode::InvalidFormula, e))?;
                }
                let mut trial = self.model.doc.clone();
                let registered: std::collections::BTreeMap<_, _> = trial
                    .features
                    .iter()
                    .flat_map(of_model::equations::parameters)
                    .map(|p| (p.key.clone(), p))
                    .collect();
                for key in p.equations.keys().chain(p.parameters.keys()) {
                    if !registered.contains_key(key) {
                        return Err(RpcError::coded(
                            ErrorCode::UnknownParameter,
                            format!("parameter {key} no longer exists"),
                        ));
                    }
                    if p.parameters.contains_key(key)
                        && p.equations.get(key).is_some_and(Option::is_some)
                    {
                        return Err(RpcError::params(format!(
                            "parameter {key} cannot have both a value and an equation"
                        )));
                    }
                }
                if let Some(id) = p.configuration {
                    let config = trial
                        .configurations
                        .items
                        .iter_mut()
                        .find(|c| c.id == id)
                        .ok_or_else(|| {
                            RpcError::coded(
                                ErrorCode::UnknownConfiguration,
                                "no such configuration",
                            )
                        })?;
                    config.parameters = p.parameters;
                    config.equations = p.equations;
                    config.variables = p.variables;
                    // Evaluate the selected variant, even if a caller edits an inactive one.
                    // Commit below preserves the original active configuration.
                    trial.configurations.active = id;
                    of_model::configurations::resolve(&trial).map_err(fail)?;
                } else {
                    trial.variables = p.variables;
                    trial.equations = p
                        .equations
                        .into_iter()
                        .filter_map(|(key, value)| value.map(|value| (key, value)))
                        .collect();
                    for (key, value) in p.parameters {
                        let parameter = &registered[&key];
                        of_model::equations::set_parameter(
                            trial.feature_mut(parameter.feature).unwrap(),
                            parameter,
                            of_model::equations::Quantity {
                                value,
                                ..parameter.value
                            },
                        )
                        .map_err(fail)?;
                        trial.equations.remove(&key);
                    }
                }
                trial
                    .validate()
                    .map_err(|e| RpcError::coded(ErrorCode::InvalidFormula, e))?;
                if method == "equations.preview" {
                    // Base fields must show base evaluations even while a variant is active.
                    if p.configuration.is_none() {
                        trial.configurations = Default::default();
                    }
                    let model = Model::new(trial);
                    return Ok(json!({
                        "values": views::equation_values(&model.doc),
                        "features": views::snapshot(&model)["features"],
                        "scene": views::scene(&model.doc, model.state()).map_err(fail)?,
                    }));
                }
                trial.configurations.active = self.model.doc.configurations.active;
                self.checkpoint();
                self.model.doc = trial;
                // Only features whose effective definition changed are rebuilt.
                self.model.rebuild();
                self.editing = None;
                self.full()
            }
            "sketch3d.solve" => {
                let p: SolveSpatialSketch = args(params)?;
                let mut sketch = p.sketch;
                let report = sketch
                    .solve(p.drag.map(|d| (d.point, DVec3::from(d.to))))
                    .map_err(RpcError::params)?;
                Ok(json!({"sketch": sketch, "report": report}))
            }
            "core.info" => Ok(json!(of_protocol::CoreInfo {
                protocol_version: of_protocol::PROTOCOL_VERSION,
                scene_version: of_protocol::SCENE_VERSION,
                part_file_version: file::VERSION,
                assembly_file_version: of_assembly::file::VERSION,
                drawing_file_version: of_drawing::file::VERSION,
                capabilities: vec![
                    "revisioned_preview".into(),
                    "assistant_drafts".into(),
                    "part_v1_read".into(),
                    "multibody".into(),
                    "equations".into(),
                    "configurations".into(),
                    "cosmetic_threads".into(),
                    "modeled_threads".into(),
                    "curves".into(),
                    "sketch_contour_paths".into(),
                    "ordered_profile_contours".into(),
                    "curve_segment_references".into(),
                    "general_body_split".into(),
                    "constrained_spatial_sketch".into(),
                    "part_v2_read".into(),
                    "sweep_loft".into(),
                    "draft".into(),
                    "circular_spherical_dome".into(),
                    "elliptical_dome".into(),
                    "noncircular_dome".into(),
                    "step_draft".into(),
                    "rib".into(),
                    "cylindrical_wrap".into(),
                    "conical_wrap".into(),
                    "freeform_wrap".into(),
                    "loft_centerline".into(),
                    "loft_end_conditions".into(),
                    "sweep_scaling_guides".into(),
                    "interior_contour_seams".into(),
                    "projected_curve_pieces".into(),
                    "boundary".into(),
                    "variable_fillet".into(),
                    "planar_face_fillet".into(),
                    "split_line".into(),
                    "assemblies".into(),
                    "standard_mates".into(),
                    "advanced_mates".into(),
                    "mechanical_mates".into(),
                    "collision_detection".into(),
                    "component_patterns".into(),
                    "mirror_components".into(),
                    "interference_detection".into(),
                    "exploded_views".into(),
                    "assembly_bom".into(),
                    "hardware_library".into(),
                    "step_assembly_structure".into(),
                    "part_v4_read".into(),
                    "drawings".into(),
                ],
            })),
            "body.display" => {
                let p: BodyDisplay = args(params)?;
                self.display()
                    .solid(&p.id)
                    .map_err(|e| RpcError::coded(ErrorCode::UnknownBody, e))?;
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
                let mut trial = self.model.doc.clone();
                let id = trial.add(p.feature.kind);
                trial.feature_mut(id).unwrap().scope = p.feature.scope;
                if let Some(n) = p.name {
                    trial.feature_mut(id).unwrap().name = n;
                }
                apply_formulas(&mut trial, id, &p.formulas)?;
                self.checkpoint();
                self.model.doc = trial;
                // Formulas may drive earlier features; the rebuild cache finds them.
                self.model.invalidate(id);
                self.editing = None;
                let mut v = self.full()?;
                v["id"] = json!(id);
                Ok(v)
            }
            "feature.update" => {
                let p: UpdateFeature = args(params)?;
                let mut trial = self.model.doc.clone();
                let f = trial
                    .feature_mut(p.id)
                    .ok_or_else(|| RpcError::coded(ErrorCode::UnknownFeature, "no such feature"))?;
                f.kind = p.feature.kind;
                f.scope = p.feature.scope;
                apply_formulas(&mut trial, p.id, &p.formulas)?;
                self.checkpoint();
                self.model.doc = trial;
                // Formulas may drive earlier features; the rebuild cache finds them.
                self.model.invalidate(p.id);
                self.editing = None;
                self.full()
            }
            "ref.locate" => {
                let p: LocateRef = args(params)?;
                let resolved = self
                    .model
                    .resolve(&p.reference)
                    .map_err(|e| RpcError::coded(ErrorCode::UnknownRef, e))?;
                let point = resolved.point().ok().map(|p| p.to_array());
                let plane = resolved
                    .plane()
                    .ok()
                    .map(|p| json!({"origin": p.origin.to_array(), "normal": p.normal.to_array()}));
                Ok(json!({ "point": point, "plane": plane }))
            }
            "equations.evaluate" => {
                let p: EvaluateFormula = args(params)?;
                let value = of_model::equations::evaluate_in(&self.model.doc, &p.expression)
                    .map_err(|e| RpcError::coded(ErrorCode::InvalidFormula, e))?;
                Ok(json!({ "value": value }))
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
                    .ok_or_else(|| RpcError::coded(ErrorCode::UnknownFeature, "no such feature"))?
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
                    .ok_or_else(|| RpcError::coded(ErrorCode::UnknownFeature, "no such feature"))?
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
                let from =
                    self.model.doc.index_of(p.id).ok_or_else(|| {
                        RpcError::coded(ErrorCode::UnknownFeature, "no such feature")
                    })?;
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
                    return Err(RpcError::coded(
                        ErrorCode::StaleRevision,
                        "the document changed; refresh before previewing",
                    ));
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
                    .ok_or_else(|| RpcError::coded(ErrorCode::UnknownFeature, "no such feature"))?;
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
                Ok(json!(
                    measure::measure(&state, &p.refs)
                        .map_err(|e| RpcError::coded(ErrorCode::UnknownRef, e))?
                ))
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
                    _ => {
                        return Err(RpcError::coded(
                            ErrorCode::NotASketch,
                            format!("feature {id} is not a sketch"),
                        ));
                    }
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
                    created.push(s.add_line(a, b).map_err(sketch_fail)?);
                    created.extend([a, b]);
                }
                SketchOp::AddCircle { center, radius } => {
                    if radius <= 0.0 {
                        return Err(RpcError::params("the radius must be more than zero"));
                    }
                    let c = center.get(s)?;
                    created.push(s.add_circle(c, radius).map_err(sketch_fail)?);
                }
                SketchOp::AddArc3 { start, end, via } => {
                    let (a, b) = (start.get(s)?, end.get(s)?);
                    created.push(tools::three_point_arc(s, a, b, v2(via)).map_err(sketch_fail)?);
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
                    created.push(arc.map_err(sketch_fail)?);
                    created.push(e);
                }
                SketchOp::AddTangentArc {
                    start,
                    tangent,
                    end,
                } => {
                    let e = end.get(s)?;
                    created
                        .push(tools::tangent_arc(s, start, v2(tangent), e).map_err(sketch_fail)?);
                }
                SketchOp::AddSpline { points, closed } => {
                    let ids: Vec<Id> = points.iter().map(|p| p.get(s)).collect::<Result<_, _>>()?;
                    created.push(s.add_spline(ids, closed).map_err(sketch_fail)?);
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
                        .map_err(sketch_fail)?
                        .entities
                }
                SketchOp::Polygon {
                    center,
                    vertex,
                    sides,
                } => {
                    created = tools::polygon(s, v2(center), v2(vertex), sides)
                        .map_err(sketch_fail)?
                        .entities
                }
                SketchOp::Constraint { constraint } => {
                    created.push(s.add_constraint(constraint).map_err(sketch_fail)?)
                }
                SketchOp::Dimension { constraint, value } => {
                    let cid = s.add_measured(constraint).map_err(sketch_fail)?;
                    if let Some(v) = value {
                        s.set_value(cid, v).map_err(sketch_fail)?;
                    }
                    created.push(cid);
                }
                SketchOp::SetValue { id: cid, value } => {
                    s.set_value(cid, value).map_err(sketch_fail)?
                }
                SketchOp::SetDriving { id: cid, driving } => {
                    s.set_driving(cid, driving).map_err(sketch_fail)?
                }
                SketchOp::Delete { ids } => s.delete(&ids),
                SketchOp::Drag { point, to } => {
                    s.point(point).map_err(sketch_fail)?;
                    s.drag(point, v2(to));
                    return Ok(());
                }
                SketchOp::Move { ids, delta } => {
                    let mut pts: Vec<Id> = Vec::new();
                    for i in &ids {
                        let e = s.entity(*i).map_err(sketch_fail)?;
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
                    created = tools::trim(s, entity, v2(pick))
                        .map_err(sketch_fail)?
                        .entities
                }
                SketchOp::Extend { entity, pick } => {
                    created = tools::extend(s, entity, v2(pick))
                        .map_err(sketch_fail)?
                        .entities
                }
                SketchOp::Offset { ids, distance } => {
                    created = tools::offset(s, &ids, distance)
                        .map_err(sketch_fail)?
                        .entities
                }
                SketchOp::Mirror { ids, about } => {
                    created = tools::mirror(s, &ids, about).map_err(sketch_fail)?.entities
                }
                SketchOp::Fillet { point, radius } => {
                    created = tools::sketch_fillet(s, point, radius)
                        .map_err(sketch_fail)?
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
                        .map_err(|e| RpcError::coded(ErrorCode::UnknownRef, e))?;
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
        .map_err(|e| RpcError::coded(ErrorCode::UnknownRef, e))
}

/// Store a feature's formulas as equations on its parameters, after checking
/// that each path names one of its parameters and each formula parses.
fn apply_formulas(
    doc: &mut of_model::Document,
    id: FeatureId,
    formulas: &std::collections::BTreeMap<String, Option<String>>,
) -> Result<(), RpcError> {
    if formulas.is_empty() {
        return Ok(());
    }
    let feature = doc.feature(id).expect("feature exists");
    let registered: std::collections::BTreeSet<String> = of_model::equations::parameters(feature)
        .into_iter()
        .map(|p| p.path)
        .collect();
    for (path, formula) in formulas {
        if formula.is_some() && !registered.contains(path) {
            return Err(RpcError::coded(
                ErrorCode::UnknownParameter,
                format!("{path} is not a parameter of this feature"),
            ));
        }
        let key = format!("f{id}.{path}");
        match formula {
            Some(text) => {
                of_model::equations::check_syntax(text).map_err(|e| {
                    RpcError::coded(ErrorCode::InvalidFormula, format!("{path}: {e}"))
                })?;
                doc.equations.insert(key, text.clone());
            }
            None => {
                doc.equations.remove(&key);
            }
        }
    }
    Ok(())
}

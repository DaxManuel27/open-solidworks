//! The result every tool call returns, for MCP clients and the built-in
//! assistant alike. A call that fails, leaves a new feature error, or adds a
//! material feature that changes nothing is `ok: false`: transport success is
//! never reported as modeling success.

use std::collections::{BTreeMap, BTreeSet};

use of_model::{Document, FeatureId, FeatureKind, Model};
use of_protocol::{ErrorCode, RpcError};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};

use super::Entry;
use crate::session::Session;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Envelope {
    /// True only if the call succeeded, introduced no feature error and every
    /// added or changed material feature changed the part.
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ToolError>,
    /// Document revision after the call.
    pub revision: u64,
    /// Document identity; it changes on new/open and core restart.
    pub generation: String,
    /// Assistant draft the call ran in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<String>,
    /// Features this call added, changed or removed.
    pub changed: Vec<Changed>,
    pub created: Created,
    /// Feature IDs removed by this call.
    pub deleted: Vec<FeatureId>,
    /// Feature errors this call introduced or left on a feature it changed.
    pub feature_errors: BTreeMap<FeatureId, String>,
    /// Errors the part already had before the call, unchanged by it.
    pub existing_errors: BTreeMap<FeatureId, String>,
    pub warnings: Vec<Warning>,
    /// The active configuration after a change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurements: Option<Measurements>,
    /// Suggested next steps.
    pub next: Vec<String>,
    /// The tool's own result, trimmed unless `detail` is `full`.
    pub result: Value,
    /// The core call itself failed, so a draft restores its document.
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) call_failed: bool,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ToolError {
    pub code: ErrorCode,
    pub message: String,
    /// JSON path of the offending argument.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Values allowed at `path`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed: Option<Vec<String>>,
    /// Feature the error is about.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature: Option<FeatureId>,
    pub repair: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    Added,
    Modified,
    Removed,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Changed {
    pub id: FeatureId,
    pub name: String,
    /// Feature `type`, such as `extrude` or `sketch`.
    pub kind: String,
    pub change: Change,
}

/// IDs created by the call, in the form the next call accepts.
#[derive(Debug, Clone, Default, Serialize, JsonSchema)]
pub struct Created {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature: Option<FeatureId>,
    /// New sketch entity and relation IDs, from `edit_sketch`.
    pub entities: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
pub enum WarningCode {
    /// The sketch can still move; add dimensions or relations.
    #[serde(rename = "W_SKETCH_UNDER_DEFINED")]
    SketchUnderDefined,
    /// The sketch has conflicting or redundant relations.
    #[serde(rename = "W_SKETCH_OVER_DEFINED")]
    SketchOverDefined,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Warning {
    pub code: WarningCode,
    pub message: String,
    pub feature: FeatureId,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Measurements {
    pub volume_mm3: f64,
    /// Bounding box `[min, max]`, mm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bbox_mm: Option<[[f64; 3]; 2]>,
    pub body_count: usize,
}

fn errors(model: &Model) -> BTreeMap<FeatureId, String> {
    model
        .statuses()
        .into_iter()
        .filter(|s| !s.suppressed && !s.rolled_back)
        .filter_map(|s| s.error.map(|e| (s.id, e)))
        .collect()
}

/// Features whose kind adds or removes material. Combine, move and split can
/// legitimately leave the volume unchanged.
fn changes_material(kind: &FeatureKind) -> bool {
    use FeatureKind::*;
    matches!(
        kind,
        Extrude(_)
            | Dome(_)
            | Rib(_)
            | Thread(_)
            | Wrap(of_model::Wrap {
                mode: of_model::WrapMode::Emboss | of_model::WrapMode::Deboss,
                ..
            })
            | Revolve(_)
            | Sweep(_)
            | Loft(_)
            | Boundary(_)
            | Hole(_)
            | Shell(_)
            | Fillet(_)
            | Chamfer(_)
            | VariableFillet(_)
            | FaceFillet(_)
            | Draft(_)
            | LinearPattern(_)
            | CircularPattern(_)
            | Mirror(_)
    )
}

fn volume(state: &of_model::State) -> f64 {
    state
        .body
        .as_ref()
        .and_then(|b| b.shape.mass_props().ok())
        .map_or(0.0, |m| m.volume)
}

/// Of `ids`, the material features that rebuilt without error but added or
/// removed no material.
pub fn no_effect(model: &mut Model, ids: impl IntoIterator<Item = FeatureId>) -> Vec<FeatureId> {
    let statuses = model.statuses();
    let mut out = Vec::new();
    for id in ids {
        let Some(feature) = model.doc.feature(id) else {
            continue;
        };
        let built = statuses
            .iter()
            .any(|s| s.id == id && s.error.is_none() && !s.suppressed && !s.rolled_back);
        if !built || !changes_material(&feature.kind) {
            continue;
        }
        let before = volume(model.state_before(id));
        let Some(after) = model.state_after(id).map(volume) else {
            continue;
        };
        if (after - before).abs() <= 1e-6 * before.abs().max(1.0) {
            out.push(id);
        }
    }
    out
}

pub const NO_EFFECT: &str =
    "added or removed no material; a cut on a face sketch usually needs reverse: true";

fn kind(feature: &of_model::Feature) -> String {
    serde_json::to_value(&feature.kind)
        .ok()
        .and_then(|v| v["type"].as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn diff(before: &Document, after: &Document) -> Vec<Changed> {
    let mut out = Vec::new();
    for f in &after.features {
        let change = match before.feature(f.id) {
            None => Change::Added,
            Some(old) if old != f => Change::Modified,
            Some(_) => continue,
        };
        out.push(Changed {
            id: f.id,
            name: f.name.clone(),
            kind: kind(f),
            change,
        });
    }
    for f in &before.features {
        if after.feature(f.id).is_none() {
            out.push(Changed {
                id: f.id,
                name: f.name.clone(),
                kind: kind(f),
                change: Change::Removed,
            });
        }
    }
    out
}

/// Drop scenes everywhere, and by default the bulky parts of results.
fn trim(value: &mut Value, full: bool, mutating: bool) {
    let Value::Object(o) = value else { return };
    o.remove("scene");
    if full {
        return;
    }
    // A mutation's full snapshot is summarized by the envelope; part_info
    // returns it on request.
    if mutating && o.contains_key("features") {
        o.retain(|k, _| {
            matches!(
                k.as_str(),
                "id" | "sketch" | "deleted" | "already_committed"
            )
        });
    }
    fn drop_samples(sketch: &mut serde_json::Map<String, Value>) {
        if let Some(Value::Array(curves)) = sketch.get_mut("curves") {
            for c in curves.iter_mut().filter_map(Value::as_object_mut) {
                c.remove("samples");
            }
        }
    }
    drop_samples(o);
    if let Some(sketch) = o.get_mut("sketch").and_then(Value::as_object_mut) {
        drop_samples(sketch);
    }
    if let Some(Value::Array(bodies)) = o.get_mut("bodies") {
        for b in bodies.iter_mut().filter_map(Value::as_object_mut) {
            for key in ["faces", "edges", "vertices"] {
                b.remove(key);
            }
        }
    }
}

fn hints(env: &Envelope, method: &str) -> Vec<String> {
    let mut next = Vec::new();
    if let Some(e) = &env.error {
        next.push(e.repair.clone());
        return next;
    }
    if env
        .warnings
        .iter()
        .any(|w| w.code == WarningCode::SketchUnderDefined)
    {
        next.push("Add dimensions or relations until the sketch is fully defined.".into());
    }
    match method {
        "sketch.new" => next.push("Add geometry with edit_sketch, then close_sketch.".into()),
        "sketch.close" => next.push("Use the sketch in add_feature.".into()),
        "feature.add" | "feature.update" | "parameter.set" => {
            next.push("Compare measurements with the requested size.".into())
        }
        _ => {}
    }
    next
}

fn tool_error(e: &RpcError) -> ToolError {
    let code = e.error_code();
    ToolError {
        code,
        message: e.message.clone(),
        path: e.data.as_ref().and_then(|d| d.path.clone()),
        allowed: e.data.as_ref().and_then(|d| d.allowed.clone()),
        feature: None,
        repair: code.repair().into(),
    }
}

/// Run one catalog tool and describe exactly what happened.
pub fn call(
    session: &mut Session,
    entry: &Entry,
    mut args: Value,
    draft: Option<&str>,
) -> Envelope {
    let full = args.get("detail").and_then(Value::as_str) == Some("full");
    if let Some(o) = args.as_object_mut() {
        o.remove("detail");
    }
    let before_doc = session.model().doc.clone();
    let before_errors = errors(session.model());
    let validated = entry.validate(&args);
    let result = validated.and_then(|()| session.handle(entry.tool.method, args));
    let after_doc = session.model().doc.clone();
    let mut after_errors = errors(session.model());
    let changed = diff(&before_doc, &after_doc);
    let touched: BTreeSet<FeatureId> = changed
        .iter()
        .filter(|c| c.change != Change::Removed)
        .map(|c| c.id)
        .collect();
    let mut feature_errors = BTreeMap::new();
    let mut existing_errors = BTreeMap::new();
    for (id, message) in std::mem::take(&mut after_errors) {
        if before_errors.get(&id) == Some(&message) && !touched.contains(&id) {
            existing_errors.insert(id, message);
        } else {
            feature_errors.insert(id, message);
        }
    }
    let idle = no_effect(session.model_mut(), touched.iter().copied());
    let statuses = session.model().statuses();
    let mut warnings = Vec::new();
    for c in changed
        .iter()
        .filter(|c| c.kind == "sketch" && c.change != Change::Removed)
    {
        let Some(report) = statuses
            .iter()
            .find(|s| s.id == c.id)
            .and_then(|s| s.sketch.as_ref())
        else {
            continue;
        };
        let code = match report.summary() {
            "Under defined" => WarningCode::SketchUnderDefined,
            "Over defined" => WarningCode::SketchOverDefined,
            _ => continue,
        };
        let message = format!("{} is {}", c.name, report.summary().to_lowercase());
        warnings.push(Warning {
            code,
            message,
            feature: c.id,
        });
    }
    let name = |id: FeatureId| {
        after_doc
            .feature(id)
            .map_or_else(|| format!("feature {id}"), |f| f.name.clone())
    };
    let error = match &result {
        Err(e) => Some(tool_error(e)),
        Ok(_) => {
            if let Some((id, message)) = feature_errors.iter().next() {
                Some(if message == of_model::CUT_REMOVES_NOTHING {
                    ToolError {
                        code: ErrorCode::NoEffect,
                        message: format!("{} {NO_EFFECT}", name(*id)),
                        path: None,
                        allowed: None,
                        feature: Some(*id),
                        repair: ErrorCode::NoEffect.repair().into(),
                    }
                } else {
                    ToolError {
                        code: ErrorCode::RebuildFailed,
                        message: format!("{} failed to rebuild: {message}", name(*id)),
                        path: None,
                        allowed: None,
                        feature: Some(*id),
                        repair: ErrorCode::RebuildFailed.repair().into(),
                    }
                })
            } else {
                idle.first().map(|id| ToolError {
                    code: ErrorCode::NoEffect,
                    message: format!("{} {NO_EFFECT}", name(*id)),
                    path: None,
                    allowed: None,
                    feature: Some(*id),
                    repair: ErrorCode::NoEffect.repair().into(),
                })
            }
        }
    };
    let mutating = !entry.tool.read_only;
    let measurements = (mutating && result.is_ok()).then(|| {
        let state = session.model().state();
        Measurements {
            volume_mm3: volume(state),
            bbox_mm: state
                .body
                .as_ref()
                .and_then(|b| b.shape.bbox())
                .map(|b| [[b.min.x, b.min.y, b.min.z], [b.max.x, b.max.y, b.max.z]]),
            body_count: state.bodies.len(),
        }
    });
    let new_ids = || {
        touched
            .iter()
            .copied()
            .filter(|id| before_doc.feature(*id).is_none())
    };
    let mut value = result.as_ref().cloned().unwrap_or(Value::Null);
    let created = Created {
        feature: new_ids().max(),
        entities: if entry.tool.method == "sketch.edit" {
            value["created"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_u64())
                        .map(|v| v as u32)
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        },
        configuration: after_doc
            .configurations
            .items
            .iter()
            .map(|c| c.id)
            .find(|id| !before_doc.configurations.items.iter().any(|c| c.id == *id)),
    };
    let revision = value["revision"].as_u64().unwrap_or(session.revision());
    trim(&mut value, full, mutating);
    let mut env = Envelope {
        ok: error.is_none(),
        error,
        revision,
        generation: session.generation().to_owned(),
        draft: draft.map(str::to_owned),
        deleted: changed
            .iter()
            .filter(|c| c.change == Change::Removed)
            .map(|c| c.id)
            .collect(),
        changed,
        created,
        feature_errors,
        existing_errors,
        warnings,
        measurements,
        next: Vec::new(),
        result: value,
        call_failed: result.is_err(),
    };
    env.next = hints(&env, entry.tool.method);
    env
}

impl Envelope {
    /// One line for clients that read only text.
    pub fn summary(&self) -> String {
        let mut line = match &self.error {
            Some(e) => format!("Failed ({}): {}", e.code.name(), e.message),
            None => "OK".to_owned(),
        };
        let added: Vec<String> = self
            .changed
            .iter()
            .map(|c| format!("{:?} {} {}", c.change, c.kind, c.id).to_lowercase())
            .collect();
        if !added.is_empty() {
            line.push_str(&format!("; {}", added.join(", ")));
        }
        if let Some(m) = &self.measurements {
            line.push_str(&format!("; volume {:.3} mm³", m.volume_mm3));
        }
        if !self.existing_errors.is_empty() {
            line.push_str(&format!(
                "; {} existing feature errors",
                self.existing_errors.len()
            ));
        }
        line
    }

    pub fn to_value(&self) -> Value {
        json!(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{by_name, catalog, output_schema, tests::example_part};

    fn fixture(name: &str) -> Session {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../bench/fixtures")
            .join(name);
        Session::open(path).unwrap()
    }

    fn run(s: &mut Session, tool: &str, args: Value) -> Envelope {
        call(s, by_name(tool).unwrap(), args, None)
    }

    fn code(env: &Envelope) -> Option<ErrorCode> {
        env.error.as_ref().map(|e| e.code)
    }

    #[test]
    fn a_failed_rebuild_is_not_success() {
        let mut s = example_part();
        let fillet = json!({ "feature": { "type": "fillet",
            "edges": [{ "type": "edge", "name": "E(2.end|2.s5)" }], "radius": 6 } });
        let env = run(&mut s, "add_feature", fillet);
        assert!(!env.ok);
        assert_eq!(code(&env), Some(ErrorCode::RebuildFailed));
        assert_eq!(env.created.feature, Some(5));
        assert!(env.feature_errors.contains_key(&5));
        assert_eq!(env.error.unwrap().feature, Some(5));
        // Updating it to another failing radius is still a failure.
        let still = json!({ "id": 5, "feature": { "type": "fillet",
            "edges": [{ "type": "edge", "name": "E(2.end|2.s5)" }], "radius": 7 } });
        assert_eq!(
            code(&run(&mut s, "update_feature", still)),
            Some(ErrorCode::RebuildFailed)
        );
        let fixed = json!({ "id": 5, "feature": { "type": "fillet",
            "edges": [{ "type": "edge", "name": "E(2.end|2.s5)" }], "radius": 2 } });
        let env = run(&mut s, "update_feature", fixed);
        assert!(env.ok && env.feature_errors.is_empty(), "{:?}", env.error);
    }

    #[test]
    fn existing_errors_are_reported_apart_from_new_ones() {
        let mut s = fixture("plate-failed-fillet.ofp");
        for (tool, args) in [
            ("part_info", json!({})),
            ("set_material", json!({ "name": "PLA" })),
        ] {
            let env = run(&mut s, tool, args);
            assert!(env.ok, "{tool}: {:?}", env.error);
            assert!(env.feature_errors.is_empty(), "{tool}");
            assert!(env.existing_errors.contains_key(&3), "{tool}");
        }
    }

    #[test]
    fn a_cut_that_removes_nothing_fails_and_blocks_the_draft() {
        let mut s = fixture("block.ofp");
        let info = s.handle("doc.info", json!({})).unwrap();
        let begin = json!({ "generation": info["generation"], "revision": info["revision"] });
        let draft = s.handle("draft.begin", begin).unwrap()["id"].clone();
        let apply = |s: &mut Session, method: &str, params: Value| {
            s.handle(
                "draft.apply",
                json!({ "id": draft, "method": method, "params": params }),
            )
            .unwrap()
        };
        let face = json!({ "type": "face", "name": "2.end" });
        apply(&mut s, "sketch.new", json!({ "plane": face }));
        let circle = json!({ "op": "add_circle", "center": { "at": [20, 15] }, "radius": 5 });
        apply(&mut s, "sketch.edit", json!({ "id": 3, "op": circle }));
        apply(&mut s, "sketch.close", json!({}));
        let cut = |reverse: bool| {
            json!({ "type": "extrude", "sketch": 3, "cut": true,
            "reverse": reverse, "end1": { "type": "through_all" } })
        };
        let env = apply(&mut s, "feature.add", json!({ "feature": cut(false) }));
        assert_eq!(env["ok"], false);
        assert_eq!(env["error"]["code"], "E_NO_EFFECT");
        assert_eq!(env["draft"], draft);
        let inspected = s.handle("draft.inspect", json!({ "id": draft })).unwrap();
        assert!(
            inspected["blocking_errors"]["4"].is_string(),
            "{}",
            inspected["blocking_errors"]
        );
        assert!(s.handle("draft.commit", json!({ "id": draft })).is_err());
        let env = apply(
            &mut s,
            "feature.update",
            json!({ "id": 4, "feature": cut(true) }),
        );
        assert_eq!(env["ok"], true, "{}", env["error"]);
        let volume = env["measurements"]["volume_mm3"].as_f64().unwrap();
        assert!((volume - (24000.0 - std::f64::consts::PI * 25.0 * 20.0)).abs() < 0.01);
        s.handle("draft.commit", json!({ "id": draft })).unwrap();
    }

    #[test]
    fn created_ids_are_returned() {
        let mut s = example_part();
        let env = run(
            &mut s,
            "new_sketch",
            json!({ "plane": { "type": "top_plane" } }),
        );
        assert_eq!(env.created.feature, Some(5));
        assert_eq!(env.result["id"], 5);
        assert_eq!(env.warnings.len(), 0);
        let rect = json!({ "id": 5, "op": { "op": "rectangle", "a": [0, 0], "b": [10, 5] } });
        let env = run(&mut s, "edit_sketch", rect);
        assert_eq!(env.created.entities.len(), 8);
        assert_eq!(env.warnings[0].code, WarningCode::SketchUnderDefined);
        let env = run(&mut s, "create_configuration", json!({ "name": "Short" }));
        assert_eq!(env.created.configuration, Some(3));
        let env = run(&mut s, "delete_features", json!({ "ids": [3] }));
        assert_eq!(env.deleted, [3]);
        assert_eq!(env.changed[0].change, Change::Removed);
    }

    #[test]
    fn errors_carry_code_path_and_repair() {
        let mut s = example_part();
        let env = run(&mut s, "get_feature", json!({ "id": 999 }));
        assert_eq!(code(&env), Some(ErrorCode::UnknownFeature));
        assert!(!env.error.unwrap().repair.is_empty());
        let bad = json!({ "feature": { "type": "sheet_metal" } });
        let error = run(&mut s, "add_feature", bad).error.unwrap();
        assert_eq!(error.code, ErrorCode::InvalidArgs);
        assert_eq!(error.path.as_deref(), Some("/feature/type"));
        assert!(error.allowed.unwrap().contains(&"extrude".to_owned()));
    }

    #[test]
    fn results_are_bounded_unless_full_detail_is_requested() {
        let mut s = example_part();
        let summary = run(&mut s, "part_info", json!({})).to_value().to_string();
        let full = run(&mut s, "part_info", json!({ "detail": "full" }))
            .to_value()
            .to_string();
        assert!(summary.len() < full.len());
        assert!(summary.len() < 16_000, "{}", summary.len());
        let added = run(
            &mut s,
            "add_feature",
            json!({ "feature": {
            "type": "extrude", "sketch": 4, "end1": { "type": "blind", "depth": 10 } } }),
        );
        assert!(added.result.get("features").is_none());
        assert!(added.to_value().to_string().len() < 4_000);
    }

    #[test]
    fn every_example_succeeds_with_a_valid_envelope() {
        let validator = jsonschema::draft202012::new(output_schema()).unwrap();
        let dir = std::env::temp_dir().join(format!("openforge-envelopes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for entry in catalog() {
            for mut example in entry.examples() {
                if let Some(path) = example.get_mut("path") {
                    *path = json!(dir.join(path.as_str().unwrap()));
                }
                let mut s = example_part();
                match entry.tool.method {
                    "import" => drop(s.handle("export", example.clone()).unwrap()),
                    "doc.open" => drop(s.handle("doc.save", example.clone()).unwrap()),
                    _ => {}
                }
                let env = call(&mut s, entry, example.clone(), None);
                assert!(env.ok, "{}: {example}: {:?}", entry.tool.name, env.error);
                let value = env.to_value();
                let errors: Vec<String> = validator
                    .iter_errors(&value)
                    .map(|e| e.to_string())
                    .collect();
                assert!(errors.is_empty(), "{}: {errors:?}", entry.tool.name);
                assert!(
                    !value.to_string().contains("\"scene\""),
                    "{}",
                    entry.tool.name
                );
            }
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}

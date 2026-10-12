//! Geometry assertions for benchmark tasks (`bench/tasks/*.json`). The core
//! checks the result of an AI task; no model judges another model's answer.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use of_model::{Document, FeatureKind, Model, file};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// One benchmark task. Paths are relative to the task file. The prompt, mode
/// and selection are read by the harness (`scripts/bench.mjs`), not the checker.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub prompt: String,
    #[serde(default = "edit")]
    pub mode: String,
    /// Starting part; omitted starts from an empty part.
    #[serde(default)]
    pub start: Option<PathBuf>,
    /// Stable refs the user has selected when sending the prompt.
    #[serde(default)]
    pub selection: Vec<Value>,
    pub expect: Expect,
}

fn edit() -> String {
    "edit".into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The task must change the part.
    Change,
    /// The right answer is a question or a refusal: the part is untouched.
    NoChange,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approx {
    pub value: f64,
    pub tol: f64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Range {
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extents {
    /// Bounding-box size along X, Y and Z in mm.
    pub value: [f64; 3],
    pub tol: f64,
    /// Compare sorted sizes, for parts whose orientation is the model's choice.
    #[serde(default)]
    pub sorted: bool,
}

/// Checks on one configuration's geometry.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    #[serde(default)]
    pub valid_solid: bool,
    pub body_count: Option<usize>,
    /// mm³
    pub volume: Option<Approx>,
    pub volume_range: Option<Range>,
    pub bbox_size: Option<Extents>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VariantCheck {
    pub name: String,
    pub part: Geometry,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    pub outcome: Outcome,
    /// No feature error that the start part did not already have.
    #[serde(default)]
    pub no_new_errors: bool,
    /// No feature errors at all, including ones in the start part.
    #[serde(default)]
    pub no_errors: bool,
    pub feature_count: Option<usize>,
    /// Feature IDs of the start part that must still exist.
    #[serde(default)]
    pub keeps_features: Vec<u32>,
    /// Checks on the active configuration.
    #[serde(default)]
    pub part: Geometry,
    #[serde(default)]
    pub configurations: Vec<VariantCheck>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub task: String,
    pub passed: bool,
    pub checks: Vec<Check>,
    /// Informational, not asserted in M0: sketches that are not fully defined.
    pub underdefined_sketches: Vec<u32>,
}

pub fn load_task(path: &Path) -> Result<Task, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut task: Task =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if let Some(start) = &task.start {
        task.start = Some(path.parent().unwrap_or(Path::new(".")).join(start));
    }
    Ok(task)
}

pub fn start_document(task: &Task) -> Result<Document, String> {
    match &task.start {
        Some(path) => file::load(path),
        None => Ok(Document::new()),
    }
}

fn errors(model: &Model) -> BTreeSet<(u32, String)> {
    model
        .statuses()
        .into_iter()
        .filter(|s| !s.suppressed && !s.rolled_back)
        .filter_map(|s| s.error.map(|e| (s.id, e)))
        .collect()
}

fn push(checks: &mut Vec<Check>, name: impl Into<String>, ok: bool, detail: impl Into<String>) {
    checks.push(Check {
        name: name.into(),
        ok,
        detail: detail.into(),
    });
}

fn geometry(checks: &mut Vec<Check>, prefix: &str, model: &Model, want: &Geometry) {
    let state = model.state();
    let body = state.body.as_ref();
    if want.valid_solid {
        let ok = body.is_some_and(|b| b.shape.is_valid());
        push(
            checks,
            format!("{prefix}valid_solid"),
            ok,
            if body.is_none() {
                "no solid body"
            } else if ok {
                "valid"
            } else {
                "invalid shape"
            },
        );
    }
    if let Some(n) = want.body_count {
        let got = state.bodies.len();
        push(
            checks,
            format!("{prefix}body_count"),
            got == n,
            format!("{got} (want {n})"),
        );
    }
    let volume = body
        .and_then(|b| b.shape.mass_props().ok())
        .map(|m| m.volume);
    if let Some(v) = want.volume {
        let ok = volume.is_some_and(|got| (got - v.value).abs() <= v.tol);
        push(
            checks,
            format!("{prefix}volume"),
            ok,
            format!("{volume:.3?} mm³ (want {} ± {})", v.value, v.tol),
        );
    }
    if let Some(r) = want.volume_range {
        let ok = volume.is_some_and(|got| got >= r.min && got <= r.max);
        push(
            checks,
            format!("{prefix}volume_range"),
            ok,
            format!("{volume:.3?} mm³ (want {}..{})", r.min, r.max),
        );
    }
    if let Some(e) = want.bbox_size {
        let size = body.and_then(|b| b.shape.bbox()).map(|b| {
            let d = b.max - b.min;
            [d.x, d.y, d.z]
        });
        let (mut got, mut target) = (size, e.value);
        if e.sorted {
            got = got.map(|mut s| {
                s.sort_by(f64::total_cmp);
                s
            });
            target.sort_by(f64::total_cmp);
        }
        let ok = got.is_some_and(|g| g.iter().zip(target).all(|(a, b)| (a - b).abs() <= e.tol));
        push(
            checks,
            format!("{prefix}bbox_size"),
            ok,
            format!("{got:.3?} (want {target:?} ± {})", e.tol),
        );
    }
}

/// Check a finished task's part against the task's expectations.
pub fn check(task: &Task, start: &Document, end: &Document) -> Report {
    let mut checks = Vec::new();
    let want = &task.expect;
    let changed = start != end;
    match want.outcome {
        Outcome::Change => push(
            &mut checks,
            "changed",
            changed,
            if changed {
                "part changed"
            } else {
                "part unchanged"
            },
        ),
        Outcome::NoChange => push(
            &mut checks,
            "unchanged",
            !changed,
            if changed {
                "part changed"
            } else {
                "part unchanged"
            },
        ),
    }
    let before = Model::new(start.clone());
    let after = Model::new(end.clone());
    let (old, new) = (errors(&before), errors(&after));
    if want.no_new_errors {
        let added: Vec<_> = new.difference(&old).collect();
        push(
            &mut checks,
            "no_new_errors",
            added.is_empty(),
            format!("{added:?}"),
        );
    }
    if want.no_errors {
        push(&mut checks, "no_errors", new.is_empty(), format!("{new:?}"));
    }
    if let Some(n) = want.feature_count {
        let got = end.features.len();
        push(
            &mut checks,
            "feature_count",
            got == n,
            format!("{got} (want {n})"),
        );
    }
    if !want.keeps_features.is_empty() {
        let missing: Vec<_> = want
            .keeps_features
            .iter()
            .filter(|id| end.feature(**id).is_none())
            .collect();
        push(
            &mut checks,
            "keeps_features",
            missing.is_empty(),
            format!("missing {missing:?}"),
        );
    }
    geometry(&mut checks, "", &after, &want.part);
    for variant in &want.configurations {
        let Some(config) = end
            .configurations
            .items
            .iter()
            .find(|c| c.name == variant.name)
        else {
            push(
                &mut checks,
                format!("configuration {}", variant.name),
                false,
                "missing",
            );
            continue;
        };
        let mut doc = end.clone();
        doc.configurations.active = config.id;
        let model = Model::new(doc);
        geometry(
            &mut checks,
            &format!("{}.", variant.name),
            &model,
            &variant.part,
        );
    }
    let underdefined_sketches = after
        .doc
        .features
        .iter()
        .zip(after.statuses())
        .filter(|(f, s)| {
            matches!(f.kind, FeatureKind::Sketch(_))
                && s.sketch
                    .as_ref()
                    .is_some_and(|r| r.summary() != "Fully defined")
        })
        .map(|(f, _)| f.id)
        .collect();
    Report {
        task: task.id.clone(),
        passed: checks.iter().all(|c| c.ok),
        checks,
        underdefined_sketches,
    }
}

/// `openforge check`: compare a finished part with a task. Prints the report.
pub fn run(task: &Path, part: &Path) -> anyhow::Result<bool> {
    let task = load_task(task).map_err(anyhow::Error::msg)?;
    let start = start_document(&task).map_err(anyhow::Error::msg)?;
    let end = file::load(part).map_err(anyhow::Error::msg)?;
    let report = check(&task, &start, &end);
    println!("{}", serde_json::to_string_pretty(&json!(report))?);
    Ok(report.passed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tasks() -> Vec<Task> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/tasks");
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        paths.sort();
        paths.iter().map(|p| load_task(p).unwrap()).collect()
    }

    #[test]
    fn every_task_loads_and_an_untouched_part_only_passes_no_change_tasks() {
        let tasks = tasks();
        assert_eq!(tasks.len(), 10);
        for task in &tasks {
            assert!(["edit", "preview", "ask"].contains(&task.mode.as_str()));
            let start = start_document(task).unwrap();
            let report = check(task, &start, &start);
            assert_eq!(
                report.passed,
                task.expect.outcome == Outcome::NoChange,
                "{}: {:?}",
                task.id,
                report.checks
            );
        }
    }

    #[test]
    fn checks_measure_the_finished_part() {
        let task = tasks()
            .into_iter()
            .find(|t| t.id == "05-edit-thickness")
            .unwrap();
        let start = start_document(&task).unwrap();
        let mut session = crate::session::Session::open(task.start.clone().unwrap()).unwrap();
        let depth = |value| json!({ "key": "f2.end1.depth", "value": value });
        session.handle("parameter.set", depth(12.0)).unwrap();
        let report = check(&task, &start, &session.model().doc);
        assert!(report.passed, "{:?}", report.checks);
        session.handle("parameter.set", depth(11.0)).unwrap();
        let report = check(&task, &start, &session.model().doc);
        let failed: Vec<_> = report
            .checks
            .iter()
            .filter(|c| !c.ok)
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(failed, ["volume", "bbox_size"]);
    }
}

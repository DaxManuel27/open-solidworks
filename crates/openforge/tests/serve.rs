//! Drive `openforge serve` over stdio the way the desktop app does.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

struct Core {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Core {
    fn start() -> Core {
        let mut child = Command::new(env!("CARGO_BIN_EXE_openforge"))
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Core {
            child,
            stdin,
            stdout,
            next: 1,
        }
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next;
        self.next += 1;
        let req = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        writeln!(self.stdin, "{req}").unwrap();
        self.stdin.flush().unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let resp: Value =
            serde_json::from_str(&line).unwrap_or_else(|e| panic!("bad line {line:?}: {e}"));
        assert_eq!(resp["id"], json!(id));
        match resp.get("error") {
            Some(e) => Err(e["message"].as_str().unwrap().to_string()),
            None => Ok(resp["result"].clone()),
        }
    }

    fn ok(&mut self, method: &str, params: Value) -> Value {
        self.call(method, params)
            .unwrap_or_else(|e| panic!("{method} failed: {e}"))
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn no_errors(snapshot: &Value) {
    for f in snapshot["features"].as_array().unwrap() {
        assert!(f["error"].is_null(), "{} failed: {}", f["name"], f["error"]);
    }
}

#[test]
fn capabilities_and_preview_revisions_follow_document_lifetime() {
    let mut core = Core::start();
    let info = core.ok("core.info", json!({}));
    assert_eq!(info["protocol_version"], 2);
    assert_eq!(info["part_file_version"], 2);
    let start = core.ok("doc.info", json!({}));
    let feature = json!({ "type": "point", "mode": "coordinates", "x": 1, "y": 2, "z": 3 });
    let preview = core.ok(
        "feature.preview",
        json!({"feature": feature, "revision": start["revision"]}),
    );
    assert_eq!(preview["revision"], start["revision"]);
    assert!(
        core.ok("doc.info", json!({}))["features"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let added = core.ok("feature.add", json!({"feature": feature}));
    assert!(added["revision"].as_u64() > start["revision"].as_u64());
    assert!(
        core.call(
            "feature.preview",
            json!({"feature": feature, "revision": start["revision"]})
        )
        .unwrap_err()
        .contains("document changed")
    );
    let undone = core.ok("doc.undo", json!({}));
    assert!(undone["revision"].as_u64() > added["revision"].as_u64());
    let fresh = core.ok("doc.new", json!({}));
    assert!(fresh["revision"].as_u64() > undone["revision"].as_u64());
}

#[test]
fn configuration_exports_are_isolated_and_failed_requests_are_transactional() {
    let mut core = Core::start();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../of-model/tests/fixtures/v1-block.ofp");
    core.ok("doc.open", json!({ "path": fixture }));
    let equations = json!({"variables": {"height":"8mm"}, "equations":{"f2.end1.depth":"height"}});
    let before = core.ok("doc.info", json!({}));
    let preview = core.ok("equations.preview", equations.clone());
    assert_eq!(preview["values"]["f2.end1.depth"]["value"]["value"], 8.0);
    assert_eq!(preview["revision"], before["revision"]);
    assert_eq!(core.ok("doc.info", json!({}))["equations"], json!({}));
    core.ok("equations.set", equations);
    let created = core.ok("configuration.create", json!({"name":"Short"}));
    let mut config = created["configurations"]["items"][1].clone();
    config["parameters"] = json!({"f2.end1.depth":3});
    config["material"] = json!({"mode":"set","value":{"name":"Variant material","density":2000}});
    let snapshot = core.ok("configuration.update", json!({"configuration": config}));
    assert_eq!(snapshot["material"], Value::Null);
    assert_eq!(snapshot["effective_material"]["name"], "Variant material");
    assert!(
        (core.ok("mass_properties", json!({}))["volume"]
            .as_f64()
            .unwrap()
            - 600.0)
            .abs()
            < 1e-6
    );
    assert!(
        core.call("configuration.create", json!({"name":"Short"}))
            .is_err()
    );
    assert_eq!(
        core.ok("doc.info", json!({}))["revision"],
        snapshot["revision"]
    );

    let dir = std::env::temp_dir().join(format!("openforge-phase2-config-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let part = dir.join("variants.ofp");
    core.ok("doc.save", json!({"path": part}));
    for (configuration, expected) in [(1, 1600.0), (2, 600.0)] {
        let step = dir.join(format!("variant{configuration}.step"));
        let result = Command::new(env!("CARGO_BIN_EXE_openforge"))
            .arg("export")
            .arg(&part)
            .arg(&step)
            .arg("--configuration")
            .arg(configuration.to_string())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let shape = of_model::geom::Shape::read_step(step.to_str().unwrap()).unwrap();
        assert!((shape.mass_props().unwrap().volume - expected).abs() < 1e-6);
    }
    assert_eq!(
        core.ok("doc.info", json!({}))["configurations"]["active"],
        2
    );
    config["equations"] = json!({"f2.end1.depth":"missing"});
    let broken = core.ok("configuration.update", json!({"configuration":config}));
    assert!(
        broken["features"][1]["error"]
            .as_str()
            .unwrap()
            .contains("missing")
    );
    let failed_export = dir.join("failed.step");
    assert!(
        core.call("export", json!({"path":failed_export}))
            .unwrap_err()
            .contains("resolve feature errors")
    );
    assert!(!failed_export.exists());
    core.ok("doc.undo", json!({}));
    no_errors(&core.ok("doc.info", json!({})));
    core.ok("doc.open", json!({"path":part}));
    assert_eq!(
        core.ok("doc.info", json!({}))["configurations"]["active"],
        2
    );
    assert!(
        (core.ok("mass_properties", json!({}))["volume"]
            .as_f64()
            .unwrap()
            - 600.0)
            .abs()
            < 1e-6
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn model_a_part_end_to_end() {
    let mut core = Core::start();
    let s = core.ok("sketch.new", json!({ "plane": { "type": "front_plane" } }));
    let sketch = s["id"].as_u64().unwrap();
    let v = core.ok(
        "sketch.edit",
        json!({ "id": sketch, "op": { "op": "rectangle", "a": [0, 0], "b": [40, 20] } }),
    );
    assert_eq!(v["curves"].as_array().unwrap().len(), 4);
    assert_eq!(v["status"], "Under defined");
    // Pin a corner to the origin and dimension it: fully defined.
    let corner = v["created"][0].as_u64().unwrap();
    let (bottom, right) = (
        v["created"][4].as_u64().unwrap(),
        v["created"][5].as_u64().unwrap(),
    );
    core.ok("sketch.edit", json!({ "id": sketch, "op": { "op": "constraint", "constraint": { "type": "coincident", "a": corner, "b": 0 } } }));
    core.ok("sketch.edit", json!({ "id": sketch, "op": { "op": "dimension", "constraint": { "type": "length", "line": bottom, "value": 0 }, "value": 50 } }));
    let v = core.ok("sketch.edit", json!({ "id": sketch, "op": { "op": "dimension", "constraint": { "type": "length", "line": right, "value": 0 } } }));
    assert_eq!(v["status"], "Fully defined", "{v}");
    let snap = core.ok("sketch.close", json!({}));
    assert!(snap["scene"]["sketches"].as_array().unwrap().len() == 1);

    let snap = core.ok("feature.add", json!({ "feature": { "type": "extrude", "sketch": sketch, "end1": { "type": "blind", "depth": 10 } } }));
    no_errors(&snap);
    assert_eq!(snap["scene"]["body"]["face_count"], 6);
    let mp = core.ok("mass_properties", json!({}));
    assert!((mp["volume"].as_f64().unwrap() - 10000.0).abs() < 1e-6);

    // Fillet the top edges found through the topology listing.
    let topo = core.ok("doc.topology", json!({}));
    let top_edges: Vec<Value> = topo["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["start"][2] == json!(10.0) && e["end"][2] == json!(10.0))
        .map(|e| e["ref"].clone())
        .collect();
    assert_eq!(top_edges.len(), 4);
    let preview = core.ok(
        "feature.preview",
        json!({ "feature": { "type": "fillet", "edges": top_edges, "radius": 2 } }),
    );
    assert!(preview["error"].is_null());
    let snap = core.ok(
        "feature.add",
        json!({ "feature": { "type": "fillet", "edges": top_edges, "radius": 2 } }),
    );
    no_errors(&snap);

    // A bad fillet reports an error on the feature, not a crash.
    let snap = core.ok(
        "feature.add",
        json!({ "feature": { "type": "fillet", "edges": top_edges, "radius": 50 } }),
    );
    let last = snap["features"].as_array().unwrap().last().unwrap().clone();
    assert!(last["error"].is_string());
    let snap = core.ok("doc.undo", json!({}));
    assert_eq!(snap["features"].as_array().unwrap().len(), 3);

    // Save, export, reopen.
    let dir = std::env::temp_dir();
    let part = dir.join("openforge-e2e.ofp");
    let step = dir.join("openforge-e2e.step");
    core.ok("doc.save", json!({ "path": part }));
    core.ok("export", json!({ "path": step }));
    let snap = core.ok("doc.open", json!({ "path": part }));
    no_errors(&snap);
    assert_eq!(snap["features"].as_array().unwrap().len(), 3);

    // Selecting a face by index gives back a stable reference.
    let r = core.ok("select.ref", json!({ "kind": "face", "index": 0 }));
    assert_eq!(r["ref"]["type"], "body_topology");
    assert_eq!(r["ref"]["kind"], "face");
    assert!(r["ref"]["body"].as_str().unwrap().starts_with('b'));
    let loc = core.ok("select.locate", json!({ "ref": r["ref"] }));
    assert_eq!(loc["index"], 0);

    assert!(core.call("no.such.method", json!({})).is_err());
    assert!(
        core.call("feature.add", json!({ "feature": { "type": "bogus" } }))
            .is_err()
    );
}

#[test]
fn assistant_drafts_are_atomic_guarded_and_idempotent() {
    let mut core = Core::start();
    let base = core.ok("doc.info", json!({}));
    let begin = |core: &mut Core, snapshot: &Value| {
        core.ok(
            "draft.begin",
            json!({"generation":snapshot["generation"],"revision":snapshot["revision"]}),
        )["id"]
            .clone()
    };
    let draft = begin(&mut core, &base);
    let apply = |core: &mut Core, id: &Value, method: &str, params: Value| {
        core.ok(
            "draft.apply",
            json!({"id":id,"method":method,"params":params}),
        )
    };
    let s = apply(
        &mut core,
        &draft,
        "sketch.new",
        json!({"plane":{"type":"front_plane"}}),
    );
    let sketch = &s["result"]["id"];
    apply(
        &mut core,
        &draft,
        "sketch.edit",
        json!({"id":sketch,"op":{"op":"rectangle","a":[0,0],"b":[60,40]}}),
    );
    apply(&mut core, &draft, "sketch.close", json!({}));
    apply(
        &mut core,
        &draft,
        "feature.add",
        json!({"feature":{"type":"extrude","sketch":sketch,"end1":{"type":"blind","depth":5}}}),
    );
    let preview = core.ok("draft.preview", json!({"id":draft}));
    assert_eq!(preview["snapshot"]["features"].as_array().unwrap().len(), 2);
    assert_eq!(core.ok("doc.info", json!({})), base);
    assert!(
        core.call(
            "draft.apply",
            json!({"id":draft,"method":"doc.save","params":{"path":"not-allowed.ofp"}})
        )
        .is_err()
    );
    let committed = core.ok("draft.commit", json!({"id":draft}));
    no_errors(&committed);
    assert_eq!(committed["features"].as_array().unwrap().len(), 2);
    let repeat = core.ok("draft.commit", json!({"id":draft}));
    assert_eq!(repeat["revision"], committed["revision"]);
    assert_eq!(repeat["already_committed"], true);
    let undone = core.ok("doc.undo", json!({}));
    assert!(undone["features"].as_array().unwrap().is_empty());
    assert_eq!(undone["can_undo"], false);
    core.ok("draft.commit", json!({"id":draft}));
    assert!(
        core.ok("doc.info", json!({}))["features"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let redone = core.ok("doc.redo", json!({}));
    let stale = begin(&mut core, &redone);
    core.ok("parameter.set", json!({"key":"f2.end1.depth","value":8}));
    assert!(
        core.call("draft.commit", json!({"id":stale}))
            .unwrap_err()
            .contains("document changed")
    );
    let current = core.ok("doc.info", json!({}));
    let bad = begin(&mut core, &current);
    apply(
        &mut core,
        &bad,
        "feature.add",
        json!({"feature":{"type":"extrude","sketch":999,"end1":{"type":"blind","depth":5}}}),
    );
    assert!(
        core.call("draft.commit", json!({"id":bad}))
            .unwrap_err()
            .contains("resolve feature")
    );
    assert_eq!(core.ok("doc.info", json!({})), current);
    core.ok("draft.discard", json!({"id":bad}));
    assert!(core.call("draft.commit", json!({"id":bad})).is_err());
    let failed = begin(&mut core, &current);
    assert!(
        core.call(
            "draft.apply",
            json!({"id":failed,"method":"feature.rename","params":{"id":999,"name":"Missing"}})
        )
        .is_err()
    );
    assert_eq!(core.ok("doc.info", json!({})), current);
    let old_generation = current["generation"].clone();
    let reset = core.ok("doc.new", json!({}));
    assert_ne!(reset["generation"], old_generation);
    assert!(core.call("draft.commit", json!({"id":failed})).is_err());
}

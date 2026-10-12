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
        // Windows keeps the process's files locked until it has exited.
        let _ = self.child.wait();
    }
}

fn no_errors(snapshot: &Value) {
    for f in snapshot["features"].as_array().unwrap() {
        assert!(f["error"].is_null(), "{} failed: {}", f["name"], f["error"]);
    }
}

#[test]
fn contour_references_are_discoverable_and_work_over_rpc() {
    let mut core = Core::start();
    let sketch = core.ok("sketch.new", json!({"plane":{"type":"front_plane"}}))["id"].clone();
    for (a, b) in [([0, 0], [5, 0]), ([10, 0], [10, 8])] {
        core.ok(
            "sketch.edit",
            json!({"id":sketch,"op":{"op":"add_line","a":{"at":a},"b":{"at":b}}}),
        );
    }
    core.ok("sketch.close", json!({}));
    let view = core.ok("sketch.get", json!({"id":sketch}));
    let contours = view["contours"].as_array().unwrap();
    assert_eq!(contours.len(), 2);
    assert!(contours.iter().all(|c| c["closed"] == false));
    let reference = contours[1]["reference"].clone();
    assert_eq!(reference["type"], "sketch_contour");
    assert_eq!(reference["sketch"], sketch);
    // Feature payloads use the same generated Ref schema as MCP and drafts.
    let result = core.ok(
        "feature.add",
        json!({"feature":{
            "type":"composite_curve", "segments":[{"reference":reference,"reverse":false}]
        }}),
    );
    no_errors(&result);
    assert_eq!(
        result["features"][1]["params"]["segments"][0]["reference"],
        reference
    );
    let id = result["features"][1]["id"].clone();
    let invalid = core.ok(
        "feature.update",
        json!({"id":id,"feature":{
            "type":"composite_curve", "segments":[{"reference":{
                "type":"sketch_contour","sketch":sketch,"entities":[]
            },"reverse":false}]
        }}),
    );
    // Direct desktop RPC retains an editable failed feature, as with other
    // geometry errors. The model must never fall back to a different contour.
    assert!(
        invalid["features"][1]["error"]
            .as_str()
            .unwrap()
            .contains("distinct sketch entity IDs")
    );
    let undone = core.ok("doc.undo", json!({}));
    no_errors(&undone);
    assert_eq!(
        undone["features"][1]["params"]["segments"][0]["reference"],
        reference
    );
}

#[test]
fn capabilities_and_preview_revisions_follow_document_lifetime() {
    let mut core = Core::start();
    let info = core.ok("core.info", json!({}));
    assert_eq!(info["protocol_version"], 3);
    assert_eq!(info["part_file_version"], 4);
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
    drop(core);
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

#[test]
fn general_split_preview_selection_undo_and_reload_use_persistent_piece_ids() {
    let mut core = Core::start();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../of-model/tests/fixtures/v1-multisolid.ofp");
    let initial = core.ok("doc.open", json!({"path":fixture}));
    let target = initial["bodies"][0]["id"].clone();
    let mut tools = Vec::new();
    for distance in [3, 7] {
        let snapshot = core.ok(
            "feature.add",
            json!({"feature":{
                "type":"plane", "mode":"offset", "reference":{"type":"right_plane"},
                "distance":distance, "flip":false
            }}),
        );
        no_errors(&snapshot);
        tools.push(json!({"type":"feature", "id":snapshot["features"].as_array().unwrap().last().unwrap()["id"]}));
    }
    let mut feature = json!({"type":"split_body", "bodies":[target], "tools":tools});
    let before = core.ok("doc.info", json!({}));
    let preview = core.ok(
        "feature.preview",
        json!({"feature":feature, "revision":before["revision"]}),
    );
    assert!(preview["error"].is_null(), "{preview}");
    let pieces: Vec<_> = preview["scene"]["bodies"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["id"].as_str().unwrap().contains(":piece:"))
        .map(|b| b["id"].clone())
        .collect();
    assert_eq!(pieces.len(), 3);
    assert_eq!(
        core.ok("doc.info", json!({}))["revision"],
        before["revision"]
    );
    feature["retained_pieces"] = json!([pieces[0], pieces[2]]);
    let kept_preview = core.ok("feature.preview", json!({"feature":feature}));
    let committed = core.ok("feature.add", json!({"feature":feature}));
    no_errors(&committed);
    assert_eq!(
        committed["scene"]["bodies"],
        kept_preview["scene"]["bodies"]
    );
    assert_eq!(committed["bodies"].as_array().unwrap().len(), 3);
    let split = committed["features"].as_array().unwrap().last().unwrap()["id"].clone();
    let undone = core.ok("doc.undo", json!({}));
    assert_eq!(undone["bodies"].as_array().unwrap().len(), 2);
    let redone = core.ok("doc.redo", json!({}));
    assert_eq!(redone["bodies"], committed["bodies"]);
    let path = std::env::temp_dir().join(format!(
        "openforge-general-split-{}.ofp",
        std::process::id()
    ));
    core.ok("doc.save", json!({"path":path}));
    let reopened = core.ok("doc.open", json!({"path":path}));
    no_errors(&reopened);
    assert_eq!(reopened["bodies"], committed["bodies"]);
    let _ = std::fs::remove_file(path);
    feature["retained_pieces"] = json!(["b999:piece:missing"]);
    let failed = core.ok("feature.update", json!({"id":split, "feature":feature}));
    assert!(
        failed["features"].as_array().unwrap().last().unwrap()["error"]
            .as_str()
            .unwrap()
            .contains("unresolved")
    );
    assert_eq!(failed["bodies"].as_array().unwrap().len(), 2);
    no_errors(&core.ok("doc.undo", json!({})));
}

#[test]
fn generated_segments_are_discoverable_and_reusable_over_rpc() {
    let mut core = Core::start();
    let mut helix =
        json!({"type":"helix","plane":{"type":"front_plane"},"radius":5,"pitch":2,"turns":2});
    let snapshot = core.ok("feature.add", json!({"feature":helix}));
    no_errors(&snapshot);
    let source = snapshot["features"][0]["id"].clone();
    let reference = snapshot["scene"]["curves"][0]["segments"][0]["reference"].clone();
    assert_eq!(reference["type"], "curve_segment");
    assert_eq!(reference["curve"], source);
    assert_eq!(reference["segment"], "helix");
    let feature =
        json!({"type":"composite_curve","segments":[{"reference":reference,"reverse":false}]});
    let preview = core.ok("feature.preview", json!({"feature":feature}));
    assert!(preview["error"].is_null(), "{preview}");
    let committed = core.ok("feature.add", json!({"feature":feature}));
    no_errors(&committed);
    helix["pitch"] = json!(3);
    let updated = core.ok("feature.update", json!({"id":source,"feature":helix}));
    no_errors(&updated);
    assert_eq!(
        updated["features"][1]["params"]["segments"][0]["reference"],
        reference
    );
    let undone = core.ok("doc.undo", json!({}));
    no_errors(&undone);
    assert_eq!(
        undone["features"][1]["params"]["segments"][0]["reference"],
        reference
    );
}

#[test]
fn ordered_profile_objects_are_accepted_by_rpc_and_preserve_authored_seams() {
    let mut core = Core::start();
    let mut profiles = Vec::new();
    for plane in [
        json!({"type":"front_plane"}),
        json!({"type":"feature","id":2}),
    ] {
        let sketch = core.ok("sketch.new", json!({"plane":plane}))["id"].clone();
        for (a, b) in [
            ([0, 0], [10, 0]),
            ([10, 0], [10, 10]),
            ([10, 10], [0, 10]),
            ([0, 10], [0, 0]),
        ] {
            core.ok(
                "sketch.edit",
                json!({"id":sketch,"op":{"op":"add_line","a":{"at":a},"b":{"at":b}}}),
            );
        }
        core.ok("sketch.close", json!({}));
        let view = core.ok("sketch.get", json!({"id":sketch}));
        let contour = &view["contours"][0];
        let entities = contour["reference"]["entities"].clone();
        let start = contour["ordered_entities"][0].clone();
        assert!(start.is_number());
        profiles.push(json!({"sketch":sketch,"entities":entities,"start":start,"reverse":false}));
        if profiles.len() == 1 {
            let datum = core.ok("feature.add", json!({"feature":{"type":"plane","mode":"offset","reference":{"type":"front_plane"},"distance":10,"flip":false}}));
            assert_eq!(datum["features"][1]["id"], 2);
        }
    }
    let feature = json!({"type":"loft","profiles":profiles,"ruled":true});
    let snapshot = core.ok("feature.add", json!({"feature":feature}));
    no_errors(&snapshot);
    assert_eq!(
        snapshot["features"][3]["params"]["profiles"],
        json!(profiles)
    );
    let loft = snapshot["features"][3]["id"].clone();
    let mut invalid = feature.clone();
    invalid["profiles"][0]["start"] = json!(9999);
    let failed = core.ok("feature.update", json!({"id":loft,"feature":invalid}));
    assert!(
        failed["features"][3]["error"]
            .as_str()
            .unwrap()
            .contains("start/seam entity")
    );
    let undone = core.ok("doc.undo", json!({}));
    no_errors(&undone);
    assert_eq!(undone["features"][3]["params"]["profiles"], json!(profiles));
}

#[test]
fn scoped_parameter_editor_preview_commit_reset_and_failed_requests() {
    let mut core = Core::start();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../of-model/tests/fixtures/v1-block.ofp");
    core.ok("doc.open", json!({ "path": fixture }));
    core.ok(
        "equations.set",
        json!({"variables":{"height":"8mm"},"equations":{"f2.end1.depth":"height"}}),
    );
    core.ok("configuration.create", json!({"name":"Variant"}));
    let before = core.ok("doc.info", json!({}));
    let edits = json!({"configuration":2,"variables":{"height":"12mm"},"equations":{"f2.end1.depth":"height"},"revision":before["revision"]});
    let preview = core.ok("equations.preview", edits.clone());
    assert_eq!(preview["values"]["f2.end1.depth"]["value"]["value"], 12.0);
    assert_eq!(preview["revision"], before["revision"]);
    assert!(!preview["scene"]["bodies"].as_array().unwrap().is_empty());
    assert_eq!(
        core.ok("doc.info", json!({}))["configurations"],
        before["configurations"]
    );
    let saved = core.ok("equations.set", edits.clone());
    assert_eq!(saved["variables"]["height"], "8mm");
    assert_eq!(saved["features"][1]["params"]["end1"]["depth"], 5.0);
    assert_eq!(
        saved["equation_values"]["f2.end1.depth"]["value"]["value"],
        12.0
    );
    assert_eq!(
        preview["scene"]["bodies"][0]["id"],
        saved["scene"]["bodies"][0]["id"]
    );
    assert_eq!(preview["scene"]["bbox"], saved["scene"]["bbox"]);
    assert!(
        (core.ok("mass_properties", json!({}))["volume"]
            .as_f64()
            .unwrap()
            - 2400.0)
            .abs()
            < 1e-6
    );
    assert!(
        core.call("equations.set", edits)
            .unwrap_err()
            .contains("document changed")
    );
    for (active, expected) in [(1, 8.0), (2, 12.0), (1, 8.0), (2, 12.0)] {
        let snap = core.ok("configuration.activate", json!({"id":active}));
        assert_eq!(
            snap["equation_values"]["f2.end1.depth"]["value"]["value"],
            expected
        );
    }
    let base = core.ok(
        "equations.preview",
        json!({"variables":{"height":"9mm"},"equations":{"f2.end1.depth":"height"}}),
    );
    assert_eq!(base["values"]["f2.end1.depth"]["value"]["value"], 9.0);
    assert_eq!(
        core.ok("doc.info", json!({}))["equation_values"]["f2.end1.depth"]["value"]["value"],
        12.0
    );
    let numeric =
        json!({"configuration":2,"variables":{},"equations":{},"parameters":{"f2.end1.depth":3.0}});
    let preview = core.ok("equations.preview", numeric.clone());
    let saved = core.ok("equations.set", numeric);
    assert_eq!(preview["scene"]["bbox"], saved["scene"]["bbox"]);
    assert!(
        (core.ok("mass_properties", json!({}))["volume"]
            .as_f64()
            .unwrap()
            - 600.0)
            .abs()
            < 1e-6
    );
    assert_eq!(
        preview["values"]["f2.end1.depth"],
        saved["equation_values"]["f2.end1.depth"]
    );
    assert_eq!(
        saved["equation_values"]["f2.end1.depth"]["value"]["value"],
        3.0
    );
    core.ok("doc.undo", json!({}));
    assert_eq!(
        core.ok("doc.info", json!({}))["equation_values"]["f2.end1.depth"]["value"]["value"],
        12.0
    );
    core.ok("doc.redo", json!({}));
    let reset = core.ok(
        "equations.set",
        json!({"configuration":2,"variables":{},"equations":{},"parameters":{}}),
    );
    assert_eq!(
        reset["equation_values"]["f2.end1.depth"]["value"]["value"],
        8.0
    );
    let bad = core.ok(
        "equations.preview",
        json!({"configuration":2,"variables":{},"equations":{"f2.end1.depth":"3deg"}}),
    );
    assert!(
        bad["values"]["f2.end1.depth"]["error"]
            .as_str()
            .unwrap()
            .contains("incompatible units")
    );
    assert!(
        core.call(
            "equations.set",
            json!({"variables":{},"equations":{"f2.not_a_parameter":"3mm"}})
        )
        .is_err()
    );
    assert_eq!(
        core.ok("doc.info", json!({}))["revision"],
        reset["revision"]
    );
    let config = reset["configurations"]["items"][1].clone();
    let mut variant = config.clone();
    variant["parameters"] = json!({"f2.end1.depth":4.0});
    let preview = core.ok("configuration.preview", json!({"configuration":variant}));
    assert_eq!(preview["values"]["f2.end1.depth"]["value"]["value"], 4.0);
    assert_eq!(
        core.ok("doc.info", json!({}))["configurations"]["items"][1],
        config
    );
}

#[test]
fn cosmetic_thread_metadata_preview_undo_variants_and_export_preserve_solid_geometry() {
    let mut core = Core::start();
    let sketch = core.ok("sketch.new", json!({"plane":{"type":"front_plane"}}))["id"]
        .as_u64()
        .unwrap();
    core.ok(
        "sketch.edit",
        json!({"id":sketch,"op":{"op":"add_circle","center":{"at":[0,0]},"radius":5}}),
    );
    core.ok("sketch.close", json!({}));
    let original = core.ok(
        "feature.add",
        json!({"feature":{"type":"extrude","sketch":sketch,"end1":{"type":"blind","depth":20}}}),
    );
    let topology = core.ok("doc.topology", json!({}));
    let face = topology["faces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["surface"] == "cylinder")
        .unwrap()["ref"]
        .clone();
    let mut feature = json!({"type":"cosmetic_thread","face":face,"diameter":10,"pitch":2,"length":12,"start_offset":1});
    let preview = core.ok("feature.preview", json!({"feature":feature}));
    let added = core.ok("feature.add", json!({"feature":feature}));
    for key in ["id", "body", "parameters", "polylines", "error"] {
        assert_eq!(
            preview["scene"]["cosmetic_threads"][0][key],
            added["scene"]["cosmetic_threads"][0][key]
        );
    }
    assert_eq!(added["scene"]["body"], original["scene"]["body"]);
    let id = added["features"].as_array().unwrap().last().unwrap()["id"]
        .as_u64()
        .unwrap();
    assert!(
        added["scene"]["cosmetic_threads"][0]["polylines"][0]
            .as_array()
            .unwrap()
            .len()
            > 100
    );
    feature["left_handed"] = json!(true);
    feature["pitch"] = json!(3);
    core.ok("feature.update", json!({"id":id,"feature":feature}));
    let undone = core.ok("doc.undo", json!({}));
    assert_eq!(
        undone["scene"]["cosmetic_threads"][0]["parameters"]["pitch"],
        2.0
    );
    let redone = core.ok("doc.redo", json!({}));
    assert_eq!(
        redone["scene"]["cosmetic_threads"][0]["parameters"]["left_handed"],
        true
    );
    core.ok(
        "configuration.create",
        json!({"name":"Short cosmetic thread"}),
    );
    let saved = core.ok("equations.set", json!({"configuration":2,"variables":{},"equations":{},"parameters":{format!("f{id}.length"):9}}));
    assert_eq!(
        saved["scene"]["cosmetic_threads"][0]["parameters"]["length"],
        9.0
    );
    assert_eq!(
        saved["features"].as_array().unwrap().last().unwrap()["params"]["length"],
        12.0
    );
    for active in [1, 2, 1, 2] {
        let snapshot = core.ok("configuration.activate", json!({"id":active}));
        assert_eq!(
            snapshot["scene"]["cosmetic_threads"][0]["parameters"]["length"],
            if active == 1 { 12.0 } else { 9.0 }
        );
        let volume = core.ok("mass_properties", json!({}))["volume"]
            .as_f64()
            .unwrap();
        assert!((volume - 500.0 * std::f64::consts::PI).abs() < 1e-7);
    }
    let dir =
        std::env::temp_dir().join(format!("openforge-cosmetic-thread-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("thread.ofp");
    core.ok("doc.save", json!({"path":path}));
    let reopened = core.ok("doc.open", json!({"path":path}));
    assert_eq!(
        reopened["scene"]["cosmetic_threads"],
        saved["scene"]["cosmetic_threads"]
    );
    let step = dir.join("thread.step");
    core.ok("export", json!({"path":step}));
    core.ok("doc.new", json!({}));
    core.ok("import", json!({"path":step}));
    let volume = core.ok("mass_properties", json!({}))["volume"]
        .as_f64()
        .unwrap();
    assert!((volume - 500.0 * std::f64::consts::PI).abs() < 1e-6);
    assert!(
        core.ok("doc.info", json!({}))["scene"]["cosmetic_threads"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    drop(core);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn spherical_dome_preview_editing_undo_and_persistence_share_the_core_contract() {
    let mut core = Core::start();
    let sketch = core.ok("sketch.new", json!({"plane":{"type":"front_plane"}}))["id"].clone();
    core.ok(
        "sketch.edit",
        json!({"id":sketch,"op":{"op":"add_circle","center":{"at":[0,0]},"radius":10}}),
    );
    core.ok("sketch.close", json!({}));
    core.ok(
        "feature.add",
        json!({"feature":{"type":"extrude","sketch":sketch,"end1":{"type":"blind","depth":8}}}),
    );
    let topology = core.ok("doc.topology", json!({}));
    let face = topology["faces"]
        .as_array()
        .unwrap()
        .iter()
        .position(|f| f["normal"][2].as_f64().unwrap() > 0.99 && f["surface"] == "plane")
        .unwrap();
    let reference = core.ok("select.ref", json!({"kind":"face","index":face}))["ref"].clone();
    let mut feature = json!({"type":"dome","face":reference,"height":3.0});
    let before = core.ok("doc.info", json!({}));
    let preview = core.ok(
        "feature.preview",
        json!({"feature":feature,"revision":before["revision"]}),
    );
    assert!(preview["error"].is_null(), "{preview}");
    assert_eq!(
        core.ok("doc.info", json!({}))["revision"],
        before["revision"]
    );
    let created = core.ok("feature.add", json!({"feature":feature}));
    no_errors(&created);
    assert_eq!(created["scene"]["bodies"], preview["scene"]["bodies"]);
    let id = created["features"].as_array().unwrap().last().unwrap()["id"].clone();
    let expected = 800.0 * std::f64::consts::PI + std::f64::consts::PI * 3.0 * 309.0 / 6.0;
    assert!(
        (core.ok("mass_properties", json!({}))["volume"]
            .as_f64()
            .unwrap()
            - expected)
            .abs()
            < 1e-5
    );
    feature["height"] = json!(2.0);
    feature["inward"] = json!(true);
    no_errors(&core.ok("feature.update", json!({"id":id,"feature":feature})));
    let inward = core.ok("mass_properties", json!({}))["volume"]
        .as_f64()
        .unwrap();
    assert!(inward < 800.0 * std::f64::consts::PI);
    no_errors(&core.ok("doc.undo", json!({})));
    assert!(
        (core.ok("mass_properties", json!({}))["volume"]
            .as_f64()
            .unwrap()
            - expected)
            .abs()
            < 1e-5
    );
    no_errors(&core.ok("doc.redo", json!({})));
    let path = std::env::temp_dir().join(format!("openforge-dome-rpc-{}.ofp", std::process::id()));
    core.ok("doc.save", json!({"path":path}));
    let reopened = core.ok("doc.open", json!({"path":path}));
    no_errors(&reopened);
    assert!(
        (core.ok("mass_properties", json!({}))["volume"]
            .as_f64()
            .unwrap()
            - inward)
            .abs()
            < 1e-5
    );
    let _ = std::fs::remove_file(path);
    feature["height"] = json!(12.0);
    let failed = core.ok("feature.update", json!({"id":id,"feature":feature}));
    assert!(
        failed["features"].as_array().unwrap().last().unwrap()["error"]
            .as_str()
            .unwrap()
            .contains("radius")
    );
    assert!(
        (core.ok("mass_properties", json!({}))["volume"]
            .as_f64()
            .unwrap()
            - 800.0 * std::f64::consts::PI)
            .abs()
            < 1e-5
    );
}

#[test]
fn formula_syntax_errors_are_rejected_before_commit() {
    let mut core = Core::start();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../of-model/tests/fixtures/v1-block.ofp");
    core.ok("doc.open", json!({ "path": fixture }));
    let before = core.ok("doc.info", json!({}));
    let error = core
        .call(
            "equations.set",
            json!({"variables":{},"equations":{"f2.end1.depth":"2 * (3mm"}}),
        )
        .unwrap_err();
    assert!(error.contains("f2.end1.depth"), "{error}");
    let error = core
        .call(
            "equations.set",
            json!({"variables":{"h":"nope(3mm)"},"equations":{}}),
        )
        .unwrap_err();
    assert!(error.contains("unknown function"), "{error}");
    let after = core.ok("doc.info", json!({}));
    assert_eq!(after["revision"], before["revision"]);
    no_errors(&after);
    // Preview keeps reporting the error per field so the editor can disable OK.
    let preview = core.ok(
        "equations.preview",
        json!({"variables":{},"equations":{"f2.end1.depth":"2 *"}}),
    );
    assert!(preview["values"]["f2.end1.depth"]["error"].is_string());
    // Missing names stay per-field diagnostics rather than syntax errors.
    core.ok(
        "equations.set",
        json!({"variables":{},"equations":{"f2.end1.depth":"missing * 2"}}),
    );
    let created = core.ok("configuration.create", json!({"name":"Variant"}));
    let mut variant = created["configurations"]["items"][1].clone();
    variant["equations"] = json!({"f2.end1.depth":"3mm +"});
    let error = core
        .call("configuration.update", json!({"configuration":variant}))
        .unwrap_err();
    assert!(error.contains("f2.end1.depth"), "{error}");
}

#[test]
fn features_take_formulas_when_created_and_edited() {
    let mut core = Core::start();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../of-model/tests/fixtures/v1-block.ofp");
    core.ok("doc.open", json!({ "path": fixture }));
    core.ok(
        "equations.set",
        json!({"variables":{"height":"4mm"},"equations":{}}),
    );
    let value = core.ok(
        "equations.evaluate",
        json!({"expression":"height * 2 + 1mm"}),
    );
    assert_eq!(value["value"]["value"], 9.0);
    assert_eq!(value["value"]["length"], 1);
    assert!(
        core.call("equations.evaluate", json!({"expression":"height *"}))
            .is_err()
    );

    let before = core.ok("doc.info", json!({}));
    let count = before["features"].as_array().unwrap().len();
    let sketch = before["features"][0]["id"].clone();
    let feature = json!({"type":"extrude","sketch":sketch,"end1":{"type":"blind","depth":1}});
    // A path that is not a parameter, or a malformed formula, adds nothing.
    for formulas in [
        json!({"end1.nope":"height"}),
        json!({"end1.depth":"height *"}),
    ] {
        assert!(
            core.call(
                "feature.add",
                json!({"feature":feature,"formulas":formulas})
            )
            .is_err()
        );
    }
    assert_eq!(
        core.ok("doc.info", json!({}))["features"]
            .as_array()
            .unwrap()
            .len(),
        count
    );
    let added = core.ok(
        "feature.add",
        json!({"feature":feature,"formulas":{"end1.depth":"height * 2"}}),
    );
    let id = added["id"].as_u64().unwrap();
    let key = format!("f{id}.end1.depth");
    assert_eq!(added["equations"][&key], "height * 2");
    assert_eq!(added["equation_values"][&key]["value"]["value"], 8.0);
    no_errors(&added);
    // Editing without the path keeps the formula; null removes it.
    let kept = core.ok("feature.update", json!({"id":id,"feature":feature}));
    assert_eq!(kept["equations"][&key], "height * 2");
    let removed = core.ok(
        "feature.update",
        json!({"id":id,"feature":feature,"formulas":{"end1.depth":null}}),
    );
    assert!(removed["equations"][&key].is_null());
}

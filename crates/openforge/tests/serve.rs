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
    assert_eq!(r["ref"]["type"], "face");
    let loc = core.ok("select.locate", json!({ "ref": r["ref"] }));
    assert_eq!(loc["index"], 0);

    assert!(core.call("no.such.method", json!({})).is_err());
    assert!(
        core.call("feature.add", json!({ "feature": { "type": "bogus" } }))
            .is_err()
    );
}

//! Drawings over `openforge serve` and the CLI.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
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
        let _ = self.child.wait();
    }
}

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let d =
            std::env::temp_dir().join(format!("openforge-drawing-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        Dir(d)
    }
    fn path(&self, f: &str) -> PathBuf {
        self.0.join(f)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// Build and save a part from sketch edits and one extrusion.
fn part(core: &mut Core, path: &Path, ops: &[Value], depth: f64) {
    core.ok("doc.new", json!({}));
    let sketch = core.ok("sketch.new", json!({"plane": {"type": "front_plane"}}))["id"].clone();
    for op in ops {
        core.ok("sketch.edit", json!({"id": sketch, "op": op}));
    }
    core.ok("sketch.close", json!({}));
    let r = core.ok(
        "feature.add",
        json!({"feature": {"type": "extrude", "sketch": sketch, "end1": {"type": "blind", "depth": depth}}}),
    );
    for f in r["features"].as_array().unwrap() {
        assert!(f["error"].is_null(), "{}", f["error"]);
    }
    core.ok("doc.save", json!({"path": path}));
}

fn front(path: &Path) -> Value {
    json!({ "type": "model", "model": { "path": path }, "orientation": { "type": "front" } })
}

/// A 100 × 60 × 10 plate.
fn plate(core: &mut Core, dir: &Dir) -> PathBuf {
    let path = dir.path("plate.ofp");
    part(
        core,
        &path,
        &[json!({ "op": "rectangle", "a": [0, 0], "b": [100, 60] })],
        10.0,
    );
    path
}

fn views(drawing: &Value, sheet: usize) -> Vec<u64> {
    drawing["drawing"]["sheets"][sheet]["views"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["id"].as_u64().unwrap())
        .collect()
}

#[test]
fn create_edit_save_export_and_reopen_a_drawing() {
    let dir = Dir::new("edit");
    let mut core = Core::start();
    let info = core.ok("core.info", json!({}));
    assert_eq!(info["drawing_file_version"], 1);
    assert!(
        info["capabilities"]
            .as_array()
            .unwrap()
            .contains(&json!("drawings"))
    );
    let plate = plate(&mut core, &dir);

    let d = core.ok("drawing.new", json!({}));
    assert_eq!(d["kind"], "drawing");
    let sheet = &d["drawing"]["sheets"][0];
    assert_eq!(sheet["size"]["type"], "a3");
    assert_eq!(sheet["projection"], "first");
    let rev = d["revision"].as_u64().unwrap();

    let v = core.ok(
        "drawing.view.add",
        json!({ "sheet": 1, "view": front(&plate), "origin": [150, 180] }),
    );
    let front_id = v["id"].as_u64().unwrap();
    assert!(v["revision"].as_u64().unwrap() > rev);
    let right = json!({ "type": "projected", "parent": front_id, "placement": "right" });
    let v = core.ok(
        "drawing.view.add",
        json!({ "sheet": 1, "view": right, "origin": [280, 100] }),
    );
    let right_id = v["id"].as_u64().unwrap();

    // The projected view is drawn level with the front view.
    let r = core.ok("drawing.render", json!({ "sheet": 1, "summary": true }));
    assert!(r.get("graphics").is_none());
    let boxes: Vec<&Value> = r["views"].as_array().unwrap().iter().collect();
    for b in &boxes {
        assert!(b.get("error").is_none(), "{b}");
    }
    let bbox = |id: u64| boxes.iter().find(|b| b["id"] == id).unwrap()["bbox"].clone();
    assert_eq!(bbox(front_id)[0][1], bbox(right_id)[0][1]);
    let full = core.ok("drawing.render", json!({ "sheet": 1 }));
    let items = full["graphics"]["items"].as_array().unwrap();
    assert!(items.iter().any(|i| i["owner"]["type"] == "view"));
    assert!(items.iter().any(|i| i["owner"]["type"] == "format"));

    // Part and assembly methods are refused while a drawing is open.
    for (method, params) in [
        ("feature.add", json!({ "feature": {} })),
        ("asm.insert", json!({ "path": plate })),
        ("mass_properties", json!({})),
    ] {
        let e = core.call(method, params).unwrap_err();
        assert!(e.contains("drawing"), "{method}: {e}");
    }
    let e = core
        .call(
            "drawing.view.add",
            json!({ "sheet": 1, "view": front(&dir.path("missing.ofp")), "origin": [0, 0] }),
        )
        .unwrap_err();
    assert!(e.contains("does not exist"), "{e}");
    let e = core
        .call(
            "drawing.view.add",
            json!({ "sheet": 9, "view": front(&plate), "origin": [0, 0] }),
        )
        .unwrap_err();
    assert!(e.contains("no sheet 9"), "{e}");

    // Undo and redo the projected view.
    let u = core.ok("doc.undo", json!({}));
    assert_eq!(views(&u, 0), vec![front_id]);
    let u = core.ok("doc.redo", json!({}));
    assert_eq!(views(&u, 0), vec![front_id, right_id]);

    core.ok(
        "drawing.properties.set",
        json!({ "properties": { "DrawnBy": "AK" } }),
    );
    std::fs::create_dir_all(dir.path("drawings")).unwrap();
    let file = dir.path("drawings/plate.ofd");
    let saved = core.ok("doc.save", json!({ "path": file }));
    assert_eq!(saved["dirty"], false);
    assert_eq!(
        saved["drawing"]["sheets"][0]["views"][0]["kind"]["model"]["path"],
        "../plate.ofp"
    );
    for name in ["plate.pdf", "plate.dxf"] {
        core.ok("export", json!({ "path": dir.path(name) }));
    }
    assert!(
        std::fs::read(dir.path("plate.pdf"))
            .unwrap()
            .starts_with(b"%PDF-")
    );
    let dxf = std::fs::read_to_string(dir.path("plate.dxf")).unwrap();
    assert!(dxf.contains("AK"), "the title block shows DrawnBy");
    let e = core
        .call("export", json!({ "path": dir.path("plate.step") }))
        .unwrap_err();
    assert!(e.contains(".pdf or .dxf"), "{e}");

    // Deleting the front view takes the projected view with it.
    let del = core.ok("drawing.view.delete", json!({ "ids": [front_id] }));
    assert_eq!(del["deleted"], json!([front_id, right_id]));

    // A new part replaces the drawing.
    let p = core.ok("doc.new", json!({}));
    assert!(p["features"].is_array());

    // Reopen the saved drawing in a fresh core and from the CLI.
    let mut again = Core::start();
    let d = again.ok("doc.open", json!({ "path": file }));
    assert_eq!(d["kind"], "drawing");
    assert_eq!(views(&d, 0), vec![front_id, right_id]);
    let r = again.ok("drawing.render", json!({ "summary": true }));
    assert_eq!(r["views"].as_array().unwrap().len(), 2);

    let out = Command::new(env!("CARGO_BIN_EXE_openforge"))
        .args(["info"])
        .arg(&file)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{text}");
    assert!(text.contains("Sheet1") && text.contains("ok"), "{text}");
    let pdf = dir.path("cli.pdf");
    let out = Command::new(env!("CARGO_BIN_EXE_openforge"))
        .arg("export")
        .arg(&file)
        .arg(&pdf)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF-"));
}

#[test]
fn sheets_are_added_reordered_and_deleted() {
    let mut core = Core::start();
    core.ok("drawing.new", json!({ "standard": "ansi" }));
    let d = core.ok("drawing.sheet.add", json!({ "name": "Details" }));
    let id = d["id"].as_u64().unwrap();
    let sheets = d["drawing"]["sheets"].as_array().unwrap();
    assert_eq!(
        sheets[1]["size"]["type"], "ansi_b",
        "copied from the last sheet"
    );
    assert_eq!(sheets[1]["projection"], "third");
    let d = core.ok("drawing.sheet.update", json!({ "id": id, "index": 0 }));
    assert_eq!(d["drawing"]["sheets"][0]["name"], "Details");
    let d = core.ok("drawing.sheet.delete", json!({ "ids": [1] }));
    assert_eq!(d["drawing"]["sheets"].as_array().unwrap().len(), 1);
    let e = core
        .call("drawing.sheet.delete", json!({ "ids": [id] }))
        .unwrap_err();
    assert!(e.contains("at least one sheet"), "{e}");
}

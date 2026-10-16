//! Sheet metal over `openforge serve` and the CLI: properties and flat DXF.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
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
        let d = std::env::temp_dir().join(format!("openforge-sheet-{name}-{}", std::process::id()));
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

/// An 80 × 30 × 1.5 plate with a Ø8 hole, as a base flange.
fn plate(core: &mut Core) {
    core.ok("doc.new", json!({}));
    let sketch = core.ok("sketch.new", json!({"plane": {"type": "front_plane"}}))["id"].clone();
    for op in [
        json!({ "op": "rectangle", "a": [0, 0], "b": [80, 30] }),
        json!({ "op": "add_circle", "center": { "at": [20, 15] }, "radius": 4 }),
    ] {
        core.ok("sketch.edit", json!({"id": sketch, "op": op}));
    }
    core.ok("sketch.close", json!({}));
    let r = core.ok(
        "feature.add",
        json!({"feature": {"type": "base_flange", "sketch": sketch,
            "sheet": {"thickness": 1.5, "radius": 1, "allowance": {"type": "k_factor", "k": 0.5}}}}),
    );
    for f in r["features"].as_array().unwrap() {
        assert!(f["error"].is_null(), "{}", f["error"]);
    }
}

#[test]
fn sheet_metal_info_and_flat_dxf() {
    let dir = Dir::new("flat");
    let mut core = Core::start();
    // No sheet metal yet: nothing to flatten.
    core.ok("doc.new", json!({}));
    assert_eq!(core.ok("doc.sheet_metal", json!({}))["bodies"], json!([]));

    plate(&mut core);
    let info = core.ok("doc.sheet_metal", json!({ "outline": true }));
    let body = &info["bodies"][0];
    assert!(body["error"].is_null(), "{}", body["error"]);
    let props = &body["properties"];
    let near = |v: &Value, x: f64| (v.as_f64().unwrap() - x).abs() < 1e-6;
    assert!(near(&props["flat_length"], 80.0) && near(&props["flat_width"], 30.0));
    assert!(near(&props["thickness"], 1.5));
    assert_eq!(props["cutouts"], json!(1));
    assert!(near(&props["inner_cut"], std::f64::consts::PI * 8.0));
    assert_eq!(body["outline"]["loops"].as_array().unwrap().len(), 2);
    assert_eq!(body["name"], json!("Body 1"));
    let id = body["body"].clone();

    // Export through the core, then by an unknown body.
    let dxf = dir.path("flat.dxf");
    let r = core.ok(
        "export",
        json!({ "path": dxf, "bodies": [id], "dxf_version": "r12" }),
    );
    assert_eq!(r["files"], json!([dxf]));
    let text = std::fs::read_to_string(&dxf).unwrap();
    assert!(text.contains("AC1009") && text.contains("CUT"));
    let e = core
        .call("export", json!({ "path": dxf, "bodies": ["b9:nope"] }))
        .unwrap_err();
    assert!(e.contains("not a sheet metal body"), "{e}");

    // The CLI writes the same from the saved part.
    let part = dir.path("plate.ofp");
    core.ok("doc.save", json!({ "path": part }));
    let out_file = dir.path("cli.dxf");
    let out = Command::new(env!("CARGO_BIN_EXE_openforge"))
        .arg("export")
        .arg(&part)
        .arg(&out_file)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = std::fs::read_to_string(&out_file).unwrap();
    assert!(text.contains("AC1032") && text.contains("CUT"));
}

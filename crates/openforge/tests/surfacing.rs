//! Surfaces, meshes, analysis and saved bodies over `openforge serve`.

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
        let d =
            std::env::temp_dir().join(format!("openforge-surfacing-{name}-{}", std::process::id()));
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

/// A 10 mm cube.
fn block(core: &mut Core) -> Value {
    core.ok("doc.new", json!({}));
    let sketch = core.ok("sketch.new", json!({"plane": {"type": "front_plane"}}))["id"].clone();
    core.ok(
        "sketch.edit",
        json!({"id": sketch, "op": { "op": "rectangle", "a": [0, 0], "b": [10, 10] }}),
    );
    core.ok("sketch.close", json!({}));
    let r = core.ok(
        "feature.add",
        json!({"feature": {"type": "extrude", "sketch": sketch, "end1": {"type": "blind", "depth": 10}}}),
    );
    for f in r["features"].as_array().unwrap() {
        assert!(f["error"].is_null(), "{}", f["error"]);
    }
    r
}

#[test]
fn analysis_surfaces_meshes_and_saved_bodies() {
    let dir = Dir::new("all");
    let mut core = Core::start();
    block(&mut core);
    let draft = core.ok(
        "doc.analysis",
        json!({"analysis": {"type": "draft", "pull": {"type": "front_plane"}, "angle": 0.01}}),
    );
    assert_eq!(draft["counts"]["requires_draft"], 4);
    assert_eq!(draft["counts"]["positive"], 1);
    assert_eq!(draft["counts"]["negative"], 1);
    let undercut = core.ok(
        "doc.analysis",
        json!({"analysis": {"type": "undercut", "pull": {"type": "front_plane"}}}),
    );
    assert!(
        undercut["faces"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["class"] == "none")
    );

    // A surface beside the solid, listed as a surface body.
    let top = core.ok("doc.topology", json!({}))["faces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["centroid"][2].as_f64().unwrap() > 9.9)
        .unwrap()["ref"]
        .clone();
    let r = core.ok(
        "feature.add",
        json!({"feature": {"type": "offset_surface", "faces": [top], "distance": 5}}),
    );
    let info = core.ok("doc.info", json!({}));
    let kinds: Vec<&str> = info["bodies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, vec!["solid", "surface"]);
    let _ = r;

    // Flatten the offset surface: the part's DXF holds its outline.
    let lifted = core.ok("doc.topology", json!({}))["faces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["centroid"][2].as_f64().unwrap() > 14.9)
        .unwrap()["ref"]
        .clone();
    core.ok(
        "feature.add",
        json!({"feature": {"type": "surface_flatten", "faces": [lifted]}}),
    );
    let dxf = dir.path("flat.dxf");
    core.ok("export", json!({ "path": dxf }));
    let text = std::fs::read_to_string(&dxf).unwrap();
    assert!(text.contains("CUT"), "{}", &text[..text.len().min(400)]);
    // Back to the part as it was, for the rest of the test.
    core.ok("doc.undo", json!({}));

    // An STL beside it, as a graphics mesh body.
    let stl = dir.path("cube.stl");
    core.ok("export", json!({ "path": stl }));
    let scene = core.ok("import", json!({ "path": stl }));
    assert_eq!(
        scene["scene"]["meshes"].as_array().unwrap().len(),
        1,
        "{}",
        scene["scene"]["meshes"]
    );
    let mesh = scene["scene"]["meshes"][0]["id"].clone();
    // STL export holds the mesh body beside the B-rep it came from.
    let count = |path: &std::path::Path| {
        let b = std::fs::read(path).unwrap();
        u32::from_le_bytes([b[80], b[81], b[82], b[83]])
    };
    let both = dir.path("both.stl");
    core.ok("export", json!({ "path": both }));
    assert_eq!(count(&both), 2 * count(&stl));
    let solid = info["bodies"][0]["id"].clone();
    // The export held the block and the offset surface: measured against
    // everything it matches; against the block alone, the surface's
    // vertices are 5 mm off.
    let deviation = core.ok(
        "doc.analysis",
        json!({"analysis": {"type": "deviation", "mesh": mesh, "limits": [0.01]}}),
    );
    assert!(deviation["max"].as_f64().unwrap() < 0.01, "{deviation}");
    let block_only = core.ok(
        "doc.analysis",
        json!({"analysis": {"type": "deviation", "mesh": mesh, "body": solid}}),
    );
    assert!((block_only["max"].as_f64().unwrap() - 5.0).abs() < 1e-6);

    // Saved bodies and a mirrored part, linked to the saved part.
    let part = dir.path("part.ofp");
    core.ok("doc.save", json!({ "path": part }));
    let saved = core.ok(
        "bodies.save",
        json!({"folder": dir.path("bodies"), "bodies": [solid], "assembly": "bodies"}),
    );
    assert!(dir.path("bodies").join("Body 1.ofp").exists(), "{saved}");
    assert!(dir.path("bodies").join("bodies.ofa").exists());
    let mirrored = dir.path("left.ofp");
    core.ok("part.mirror", json!({ "path": mirrored }));
    core.ok("doc.open", json!({ "path": mirrored }));
    let props = core.ok("mass_properties", json!({}));
    assert!(
        (props["volume"].as_f64().unwrap() - 1000.0).abs() < 1e-6,
        "{props}"
    );
}

//! Assemblies over `openforge serve`, the way the desktop app drives them.

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
        let d = std::env::temp_dir().join(format!("openforge-asm-{name}-{}", std::process::id()));
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

/// The first face of a component whose picked geometry matches.
fn find_face(core: &mut Core, key: &str, scene: &Value, test: impl Fn(&Value) -> bool) -> Value {
    let inst = scene["instances"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["key"] == json!([key]))
        .unwrap();
    let part = inst["part"].as_str().unwrap();
    let faces = scene["parts"][part]["face_count"].as_u64().unwrap();
    for i in 0..faces {
        let r = core.ok(
            "asm.pick",
            json!({"component": [key], "kind": "face", "index": i}),
        );
        if test(&r["geometry"]) {
            return r["ref"].clone();
        }
    }
    panic!("no matching face on {key}");
}

fn translation(info: &Value, key: &str) -> [f64; 3] {
    let i = info["scene"]["instances"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["key"] == json!([key]))
        .unwrap();
    let m = i["transform"].as_array().unwrap();
    [
        m[12].as_f64().unwrap(),
        m[13].as_f64().unwrap(),
        m[14].as_f64().unwrap(),
    ]
}

fn component<'a>(info: &'a Value, key: &str) -> &'a Value {
    info["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["key"] == json!([key]))
        .unwrap()
}

#[test]
fn assemblies_over_rpc() {
    let dir = Dir::new("rpc");
    let mut core = Core::start();
    let info = core.ok("core.info", json!({}));
    assert!(
        info["capabilities"]
            .as_array()
            .unwrap()
            .contains(&json!("assemblies"))
    );
    assert_eq!(info["assembly_file_version"], 1);
    assert!(
        core.call("asm.insert", json!({"path": "x.ofp"}))
            .unwrap_err()
            .contains("needs an assembly")
    );

    let plate = dir.path("plate.ofp");
    let pin = dir.path("pin.ofp");
    part(
        &mut core,
        &plate,
        &[
            json!({"op": "rectangle", "a": [0, 0], "b": [100, 60]}),
            json!({"op": "add_circle", "center": {"at": [30, 30]}, "radius": 5}),
        ],
        10.0,
    );
    part(
        &mut core,
        &pin,
        &[json!({"op": "add_circle", "center": {"at": [0, 0]}, "radius": 5})],
        40.0,
    );

    let a = core.ok("asm.new", json!({}));
    assert_eq!(a["kind"], "assembly");
    assert!(
        core.call("sketch.new", json!({"plane": {"type": "front_plane"}}))
            .unwrap_err()
            .contains("needs a part")
    );
    let p = core.ok("asm.insert", json!({"path": plate}))["key"]
        .as_str()
        .unwrap()
        .to_string();
    let r = core.ok(
        "asm.insert",
        json!({"path": pin, "transform": {"translation": [70, -20, 30], "rotation": [0, 0, 0, 1]}}),
    );
    let s = r["key"].as_str().unwrap().to_string();
    assert_eq!(r["components"].as_array().unwrap().len(), 2);
    assert_eq!(component(&r, &p)["fixed"], true);
    assert_eq!(r["scene"]["parts"].as_object().unwrap().len(), 2);
    let scene = r["scene"].clone();

    let hole = find_face(&mut core, &p, &scene, |g| g["type"] == "cylinder");
    let shaft = find_face(&mut core, &s, &scene, |g| g["type"] == "cylinder");
    let top = find_face(&mut core, &p, &scene, |g| {
        g["type"] == "plane"
            && g["normal"][2].as_f64() > Some(0.9)
            && g["origin"][2].as_f64() > Some(9.0)
    });
    let bottom = find_face(&mut core, &s, &scene, |g| {
        g["type"] == "plane" && g["normal"][2].as_f64() < Some(-0.9)
    });
    let m = core.ok(
        "asm.mate.add",
        json!({"mate": {"type": "concentric", "a": hole, "b": shaft}}),
    );
    assert_eq!(component(&m, &s)["dof"], 2);
    let m = core.ok(
        "asm.mate.add",
        json!({"mate": {"type": "coincident", "a": top, "b": bottom}}),
    );
    assert_eq!(component(&m, &s)["dof"], 1);
    assert_eq!(m["mates"].as_array().unwrap().len(), 2);
    let t = translation(&m, &s);
    assert!((t[0] - 30.0).abs() < 1e-6 && (t[1] - 30.0).abs() < 1e-6 && (t[2] - 10.0).abs() < 1e-6);

    // A conflicting mate is refused and changes nothing.
    let rev = m["revision"].as_u64().unwrap();
    let err = core
        .call(
            "asm.mate.add",
            json!({"mate": {"type": "distance", "a": top, "b": bottom, "distance": 4}}),
        )
        .unwrap_err();
    assert!(err.contains("Coincident1"), "{err}");
    assert_eq!(
        core.ok("doc.info", json!({}))["revision"].as_u64().unwrap(),
        rev
    );

    // Undo and redo the second mate.
    let u = core.ok("doc.undo", json!({}));
    assert_eq!(u["mates"].as_array().unwrap().len(), 1);
    let u = core.ok("doc.redo", json!({}));
    assert_eq!(u["mates"].as_array().unwrap().len(), 2);

    // Rotating the pin is the one freedom left; moving it sideways isn't.
    let moved = core.ok(
        "asm.move",
        json!({"component": [s], "motion": {"mode": "delta", "translation": [20, 0, 0]}}),
    );
    assert!((translation(&moved, &s)[0] - 30.0).abs() < 1e-6);
    let preview = core.ok(
        "asm.move",
        json!({"component": [s], "motion": {"mode": "rotate", "angles": [0, 0, 0.5]}, "phase": "move"}),
    );
    assert!(preview["instances"].is_array() && preview.get("components").is_none());
    core.ok(
        "asm.move",
        json!({"component": [s], "motion": {"mode": "rotate", "angles": [0, 0, 0.5]}, "phase": "end"}),
    );

    // Pattern the pin, explode it, read the BOM and mass, export STEP.
    let pat = core.ok(
        "asm.pattern.add",
        json!({"pattern": {"type": "linear", "direction1": {"reference": {"component": [], "entity": {"type": "right_plane"}}, "spacing": 20, "count": 3}}, "seeds": [s]}),
    );
    assert_eq!(pat["scene"]["instances"].as_array().unwrap().len(), 4);
    let ex = core.ok(
        "asm.explode.add",
        json!({"components": [[s]], "motion": {"type": "translate", "direction": [0, 0, 1], "distance": 50}}),
    );
    assert!((translation(&ex, &s)[2] - 60.0).abs() < 1e-6);
    assert_eq!(ex["scene"]["explode_lines"].as_array().unwrap().len(), 1);
    let collapsed = core.ok("asm.explode.show", json!({"steps": null}));
    assert!((translation(&collapsed, &s)[2] - 10.0).abs() < 1e-6);

    let bom = core.ok(
        "asm.bom",
        json!({"type": "parts_only", "csv": dir.path("bom.csv")}),
    );
    let rows = bom["rows"].as_array().unwrap();
    assert_eq!(
        rows.iter().find(|r| r["part_number"] == "pin").unwrap()["quantity"],
        3
    );
    assert!(
        std::fs::read_to_string(dir.path("bom.csv"))
            .unwrap()
            .contains("pin")
    );
    let mass = core.ok("mass_properties", json!({}));
    assert!(mass["mass"].as_f64().unwrap() > 0.0);
    let step = dir.path("asm.step");
    core.ok("export", json!({"path": step}));
    assert!(
        std::fs::read_to_string(&step)
            .unwrap()
            .contains("NEXT_ASSEMBLY_USAGE_OCCURRENCE")
    );
    core.ok("export", json!({"path": dir.path("asm.stl")}));

    // Interference: nothing overlaps; the seated pin touches the plate.
    let i = core.ok("asm.interference", json!({}));
    assert!(i["interferences"].as_array().unwrap().is_empty());
    let i = core.ok("asm.interference", json!({"treat_coincidence": true}));
    assert!(!i["interferences"].as_array().unwrap().is_empty());

    // Save, reopen through doc.open, and return to parts with doc.new.
    let file = dir.path("pinned.ofa");
    let saved = core.ok("doc.save", json!({"path": file}));
    assert_eq!(saved["dirty"], false);
    core.ok("doc.new", json!({}));
    let reopened = core.ok("doc.open", json!({"path": file}));
    assert_eq!(reopened["kind"], "assembly");
    assert_eq!(reopened["mates"].as_array().unwrap().len(), 2);
    assert_eq!(reopened["patterns"].as_array().unwrap().len(), 1);
    let back = core.ok("doc.new", json!({}));
    assert!(back.get("kind").is_none() || back["kind"] != "assembly");

    // STEP import builds an assembly from the export.
    let imported = core.ok(
        "asm.import_step",
        json!({"path": step, "folder": dir.path("imported")}),
    );
    assert_eq!(imported["components"].as_array().unwrap().len(), 4);
    assert!(imported["dirty"].as_bool().unwrap());

    // Hardware: a catalog, and a screw from it in a new assembly.
    let cat = core.ok("asm.hardware_catalog", json!({}));
    assert!(cat["catalog"].as_array().unwrap().len() >= 7);
    core.ok("asm.new", json!({}));
    let hw = core.ok(
        "asm.insert_hardware",
        json!({"hardware": {"kind": "socket_head_cap_screw", "size": "M4", "length": 12}, "folder": dir.path("hw")}),
    );
    assert!(Path::new(hw["path"].as_str().unwrap()).exists());
}

#[test]
fn cli_info_and_export_read_assemblies() {
    let dir = Dir::new("cli");
    let mut core = Core::start();
    let block = dir.path("block.ofp");
    part(
        &mut core,
        &block,
        &[json!({"op": "rectangle", "a": [0, 0], "b": [10, 10]})],
        10.0,
    );
    core.ok("asm.new", json!({}));
    core.ok("asm.insert", json!({"path": block}));
    core.ok(
        "asm.insert",
        json!({"path": block, "transform": {"translation": [20, 0, 0], "rotation": [0, 0, 0, 1]}}),
    );
    let file = dir.path("two.ofa");
    core.ok("doc.save", json!({"path": file}));
    drop(core);
    let out = Command::new(env!("CARGO_BIN_EXE_openforge"))
        .args(["info", file.to_str().unwrap()])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{text}");
    assert!(text.contains("block<1>") && text.contains("block<2>") && text.contains("fixed"));
    let step = dir.path("two.step");
    let out = Command::new(env!("CARGO_BIN_EXE_openforge"))
        .args(["export", file.to_str().unwrap(), step.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(step.exists());
}

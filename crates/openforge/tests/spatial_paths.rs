//! Spatial path edits through the same process/RPC boundary used by the desktop.
use of_model::{
    geom::{DVec2, DVec3},
    sketch::{
        Sketch,
        spatial::{Axis3, Geometry3, Relation3, Sketch3D},
    },
    *,
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

struct Core {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next: u32,
}
impl Core {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_openforge"))
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            input,
            output,
            next: 0,
        }
    }
    fn call(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        writeln!(
            self.input,
            "{}",
            json!({"jsonrpc":"2.0","id":self.next,"method":method,"params":params})
        )
        .unwrap();
        self.input.flush().unwrap();
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        let response: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"));
        assert_eq!(response["id"], self.next);
        assert!(response["error"].is_null(), "{method}: {response}");
        response["result"].clone()
    }
    fn volume(&mut self) -> f64 {
        self.call("mass_properties", json!({}))["volume"]
            .as_f64()
            .unwrap()
    }
}
impl Drop for Core {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}
fn valid(snapshot: &Value) {
    assert!(
        snapshot["features"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["error"].is_null()),
        "{snapshot}"
    );
}

#[test]
fn spatial_dimension_edit_undo_redo_suppression_rollback_reopen_and_export() {
    let mut doc = Document::new();
    let mut profile = Sketch::new();
    profile.add_circle_at(DVec2::ZERO, 1.);
    let profile = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: profile,
    }));
    let mut spatial = Sketch3D::default();
    let a = spatial.add_point(DVec3::ZERO);
    let b = spatial.add_point(DVec3::Z * 10.);
    let line = spatial.add(Geometry3::Line { a, b });
    spatial.constrain(Relation3::Fixed {
        point: a,
        at: DVec3::ZERO,
    });
    spatial.constrain(Relation3::Axis {
        line,
        axis: Axis3::Z,
    });
    let dimension = spatial.constrain(Relation3::Distance { a, b, value: 10. });
    let source = doc.add(FeatureKind::Sketch3d(spatial.clone()));
    doc.add(FeatureKind::Sweep(Sweep {
        profile: Some(profile.into()),
        inner_profiles: vec![],
        circle_diameter: None,
        twist: 0.0,
        guide: None,
        guide_scale: false,
        guide2: None,
        thin: None,
        path: Ref::Feature { id: source },
        reverse_path: false,
        orientation: SweepOrientation::MinimumTwist,
        cut: false,
        merge: true,
    }));
    let stem = std::env::temp_dir().join(format!("openforge-spatial-rpc-{}", std::process::id()));
    let part = stem.with_extension("ofp");
    let step = stem.with_extension("step");
    file::save(&doc, &part).unwrap();
    let mut core = Core::start();
    let opened = core.call("doc.open", json!({"path":part}));
    valid(&opened);
    assert_eq!(opened["scene"]["sketches3d"][0]["id"], source);
    assert_eq!(opened["scene"]["sketches3d"][0]["report"]["dof"], 0);
    assert!((core.volume() - std::f64::consts::PI * 10.).abs() < 1e-5);
    let constraint = spatial
        .constraints
        .iter_mut()
        .find(|c| c.id == dimension)
        .unwrap();
    constraint.relation = Relation3::Distance { a, b, value: 20. };
    let solved = core.call(
        "sketch3d.solve",
        json!({"sketch":spatial,"drag":{"point":b,"to":[0,0,30]}}),
    );
    let solved_sketch: Sketch3D = serde_json::from_value(solved["sketch"].clone()).unwrap();
    assert!((solved_sketch.point(b).unwrap().z - 20.).abs() < 1e-6);
    let updated = core.call(
        "feature.update",
        json!({"id":source,"feature":FeatureKind::Sketch3d(solved_sketch.clone())}),
    );
    valid(&updated);
    assert!((core.volume() - std::f64::consts::PI * 20.).abs() < 1e-5);
    valid(&core.call("doc.undo", json!({})));
    assert!((core.volume() - std::f64::consts::PI * 10.).abs() < 1e-5);
    valid(&core.call("doc.redo", json!({})));
    assert!((core.volume() - std::f64::consts::PI * 20.).abs() < 1e-5);
    let suppressed = core.call("feature.suppress", json!({"id":source,"suppressed":true}));
    assert!(suppressed["features"][2]["error"].is_string());
    assert!(suppressed["scene"]["body"].is_null());
    valid(&core.call("doc.undo", json!({})));
    let rolled = core.call("doc.rollback", json!({"index":1}));
    assert_eq!(rolled["features"][1]["rolled_back"], true);
    assert!(rolled["scene"]["body"].is_null());
    valid(&core.call("doc.undo", json!({})));
    let mut conflicting = solved_sketch.clone();
    conflicting.constrain(Relation3::Fixed {
        point: b,
        at: DVec3::Z * 30.,
    });
    let failed = core.call(
        "feature.update",
        json!({"id":source,"feature":FeatureKind::Sketch3d(conflicting)}),
    );
    assert!(failed["features"][1]["error"].is_string());
    assert!(failed["features"][2]["error"].is_string());
    valid(&core.call("doc.undo", json!({})));
    core.call("doc.save", json!({"path":part}));
    valid(&core.call("doc.open", json!({"path":part})));
    assert!((core.volume() - std::f64::consts::PI * 20.).abs() < 1e-5);
    core.call("export", json!({"path":step}));
    let shape = geom::Shape::read_step(step.to_str().unwrap()).unwrap();
    assert!(shape.is_valid());
    assert!((shape.mass_props().unwrap().volume - std::f64::consts::PI * 20.).abs() < 1e-5);
    std::fs::remove_file(part).unwrap();
    std::fs::remove_file(step).unwrap();
}

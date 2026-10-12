use of_model::{
    geom::{DVec2, Kind},
    sketch::{Sketch, tools},
    *,
};

fn volume(m: &Model) -> f64 {
    m.state()
        .body
        .as_ref()
        .unwrap()
        .shape
        .mass_props()
        .unwrap()
        .volume
}

fn blind(sketch: FeatureId, depth: f64) -> FeatureKind {
    FeatureKind::Extrude(Extrude {
        sketch,
        cut: false,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    })
}

/// A block with a boss on top, then a fillet, with the block's depth driven by a
/// variable and the boss depth driven by the fillet radius (a later feature).
fn part() -> (Model, FeatureId, FeatureId, FeatureId) {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(40.0, 20.0));
    let s1 = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    let block = doc.add(blind(s1, 10.0));
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::new(5.0, 5.0), DVec2::new(15.0, 15.0));
    let s2 = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    let boss = doc.add(blind(s2, 20.0));
    let m = Model::new(doc);
    let body = m.state().body.as_ref().unwrap();
    let edge = (0..body.shape.count(Kind::Edge))
        .find(|i| {
            let e = body.shape.edge_info(*i).unwrap();
            (e.mid.z - 20.0).abs() < 1e-9
        })
        .unwrap();
    let edge = m.state().ref_for(Kind::Edge, edge).unwrap();
    let mut doc = m.doc.clone();
    let fillet = doc.add(FeatureKind::Fillet(Fillet {
        edges: vec![edge],
        faces: vec![],
        radius: 1.0,
    }));
    doc.variables.insert("height".into(), "10mm".into());
    doc.equations
        .insert(format!("f{block}.end1.depth"), "height".into());
    doc.equations.insert(
        format!("f{boss}.end1.depth"),
        format!("f{fillet}.radius * 20"),
    );
    (Model::new(doc), block, boss, fillet)
}

fn fresh(m: &Model) -> f64 {
    volume(&Model::new(m.doc.clone()))
}

#[test]
fn edits_rebuild_only_what_their_inputs_change() {
    let (mut m, _, _, fillet) = part();
    let n = m.doc.features.len();
    assert!(m.statuses().iter().all(|s| s.error.is_none()));

    // Nothing changed: nothing rebuilds.
    let before = m.build_count();
    m.rebuild();
    assert_eq!(m.build_count(), before);

    // A variable that drives the first extrude rebuilds from it.
    m.doc.variables.insert("height".into(), "12mm".into());
    let before = m.build_count();
    m.rebuild();
    assert_eq!(m.build_count() - before, n - 1);
    assert!((volume(&m) - fresh(&m)).abs() < 1e-9);

    // Changing the fillet radius also drives the boss, an earlier feature, so the
    // rebuild starts at the boss even though only the fillet was invalidated.
    if let FeatureKind::Fillet(f) = &mut m.doc.feature_mut(fillet).unwrap().kind {
        f.radius = 1.5;
    }
    m.invalidate(fillet);
    let before = m.build_count();
    m.rebuild();
    assert_eq!(m.build_count() - before, 2, "the boss and the fillet");
    assert!((volume(&m) - fresh(&m)).abs() < 1e-9);
    // The boss rises from the front plane to 1.5 * 20 mm.
    let top = m.state().body.as_ref().unwrap().shape.bbox().unwrap().max.z;
    assert!((top - 30.0).abs() < 1e-5, "{top}");

    // A configuration that overrides only the fillet rebuilds from the boss too.
    let mut config = configurations::Configuration::new(2, "Big fillet".into());
    config.parameters.insert(format!("f{fillet}.radius"), 2.0);
    m.doc.configurations.items.push(config);
    m.doc.configurations.next_id = 3;
    m.doc.configurations.active = 2;
    let before = m.build_count();
    m.rebuild();
    assert_eq!(m.build_count() - before, 2);
    assert!((volume(&m) - fresh(&m)).abs() < 1e-9);
}

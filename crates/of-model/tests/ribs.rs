use of_model::{
    geom::{DVec2, DVec3, Kind, SurfaceKind},
    sketch::{Sketch, tools},
    *,
};

/// A 40 x 20 x 10 box on the front plane, shelled 2 mm with its top face removed:
/// a cavity x 2..38, y 2..18, z 2..10.
fn housing() -> Model {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(40.0, 20.0));
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::Extrude(Extrude {
        sketch: s,
        cut: false,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth: 10.0 },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    let mut m = Model::new(doc);
    let top = face(&m, DVec3::Z);
    m.doc.add(FeatureKind::Shell(ShellDef {
        faces: vec![top],
        thickness: 2.0,
        outward: false,
    }));
    m.rebuild();
    valid(&m);
    assert_close(volume(&m), 8000.0 - 36.0 * 16.0 * 8.0);
    m
}

fn face(m: &Model, n: DVec3) -> Ref {
    let body = m.state().body.as_ref().unwrap();
    let i = (0..body.shape.count(Kind::Face))
        .find(|&i| {
            let f = body.shape.face_info(i).unwrap();
            f.kind == SurfaceKind::Plane && f.normal.dot(n) > 0.999
        })
        .unwrap();
    m.state().ref_for(Kind::Face, i).unwrap()
}

fn valid(m: &Model) {
    for s in m.statuses() {
        assert!(s.error.is_none(), "feature {} failed: {:?}", s.id, s.error);
    }
    assert!(m.state().body.as_ref().unwrap().shape.is_valid());
}

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

fn assert_close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-4 * b.abs().max(1.0), "{a} != {b}");
}

/// A sketch holding one line, given in world coordinates on `plane`.
fn line_sketch(m: &mut Model, plane: Ref, a: DVec3, b: DVec3) -> FeatureId {
    let id = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: plane.clone(),
        sketch: Sketch::new(),
    }));
    m.rebuild();
    let frame = m.resolve(&plane).unwrap().plane().unwrap();
    let mut sk = Sketch::new();
    sk.add_line_at(frame.to_local(a), frame.to_local(b));
    if let FeatureKind::Sketch(s) = &mut m.doc.feature_mut(id).unwrap().kind {
        s.sketch = sk;
    }
    m.invalidate(id);
    id
}

fn rib(m: &mut Model, sketch: FeatureId, direction: RibDirection) -> FeatureId {
    let id = m.doc.add(FeatureKind::Rib(Rib {
        sketch,
        thickness: 2.0,
        side: RibSide::Both,
        direction,
        flip: false,
        draft: None,
    }));
    m.rebuild();
    id
}

fn set_rib(m: &mut Model, id: FeatureId, edit: impl FnOnce(&mut Rib)) {
    if let FeatureKind::Rib(r) = &mut m.doc.feature_mut(id).unwrap().kind {
        edit(r);
    }
    m.invalidate(id);
    m.rebuild();
}

fn offset_plane(m: &mut Model, reference: Ref, distance: f64) -> Ref {
    let id = m.doc.add(FeatureKind::Plane(PlaneDef::Offset {
        reference,
        distance,
        flip: false,
    }));
    Ref::Feature { id }
}

const HOUSING: f64 = 8000.0 - 36.0 * 16.0 * 8.0;
/// A 2 mm web across the cavity's width, from the floor to the rim.
const WEB: f64 = 2.0 * 16.0 * 8.0;

#[test]
fn normal_rib_extends_to_walls_and_floor() {
    let mut m = housing();
    let rim = offset_plane(&mut m, Ref::FrontPlane, 10.0);
    // A short line in the middle of the opening; the rib extends to both walls.
    let s = line_sketch(
        &mut m,
        rim,
        DVec3::new(20.0, 6.0, 10.0),
        DVec3::new(20.0, 14.0, 10.0),
    );
    let r = rib(&mut m, s, RibDirection::NormalToSketch);
    valid(&m);
    assert_close(volume(&m), HOUSING + WEB);
    let b = m.state().body.as_ref().unwrap().shape.bbox().unwrap();
    assert!((b.max.z - 10.0).abs() < 1e-6);

    // Thickness on one side keeps the volume and moves the web off the line.
    for side in [RibSide::First, RibSide::Second] {
        set_rib(&mut m, r, |p| p.side = side);
        valid(&m);
        assert_close(volume(&m), HOUSING + WEB);
    }

    // Growing away from the part never meets material.
    set_rib(&mut m, r, |p| {
        p.side = RibSide::Both;
        p.flip = true;
    });
    let error = m.status(r).unwrap().error.unwrap();
    assert!(error.contains("does not meet"), "{error}");
    assert_close(volume(&m), HOUSING);
}

#[test]
fn parallel_rib_grows_in_its_plane() {
    let mut m = housing();
    let mid = offset_plane(&mut m, Ref::RightPlane, 20.0);
    let s = line_sketch(
        &mut m,
        mid,
        DVec3::new(20.0, 6.0, 10.0),
        DVec3::new(20.0, 14.0, 10.0),
    );
    let r = rib(&mut m, s, RibDirection::ParallelToSketch);
    valid(&m);
    assert_close(volume(&m), HOUSING + WEB);

    // A thicker rib from an upstream edit rebuilds.
    set_rib(&mut m, r, |p| p.thickness = 3.0);
    valid(&m);
    assert_close(volume(&m), HOUSING + 3.0 * 16.0 * 8.0);

    let text = file::to_string(&m.doc);
    let reopened = Model::new(file::from_str(&text).unwrap());
    valid(&reopened);
    assert_close(volume(&reopened), volume(&m));
}

#[test]
fn rib_rejects_invalid_input() {
    let mut m = housing();
    let rim = offset_plane(&mut m, Ref::FrontPlane, 10.0);
    let s = line_sketch(
        &mut m,
        rim.clone(),
        DVec3::new(20.0, 6.0, 10.0),
        DVec3::new(20.0, 14.0, 10.0),
    );
    let r = rib(&mut m, s, RibDirection::NormalToSketch);
    set_rib(&mut m, r, |p| p.thickness = 0.0);
    assert!(m.status(r).unwrap().error.unwrap().contains("thickness"));

    // Closed sketches are profiles, not ribs.
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::new(5.0, 5.0), DVec2::new(10.0, 10.0));
    let closed = m.doc.add(FeatureKind::Sketch(SketchFeature {
        plane: rim,
        sketch: sk,
    }));
    let r2 = rib(&mut m, closed, RibDirection::NormalToSketch);
    assert!(m.status(r2).unwrap().error.unwrap().contains("open"));

    // Without existing material there is nothing to meet.
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    sk.add_line_at(DVec2::ZERO, DVec2::new(10.0, 0.0));
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    let mut m = Model::new(doc);
    let r = rib(&mut m, s, RibDirection::ParallelToSketch);
    assert!(m.status(r).unwrap().error.unwrap().contains("material"));
}

#[test]
fn drafted_ribs_thicken_toward_the_floor() {
    // 5 degrees on each thickness face over the 8 mm drop from rim to floor.
    let base = 2.0 + 2.0 * 8.0 * 5f64.to_radians().tan();
    let expected = HOUSING + 16.0 * 8.0 * (2.0 + base) / 2.0;
    for direction in [RibDirection::NormalToSketch, RibDirection::ParallelToSketch] {
        let mut m = housing();
        let plane = if direction == RibDirection::NormalToSketch {
            offset_plane(&mut m, Ref::FrontPlane, 10.0)
        } else {
            offset_plane(&mut m, Ref::RightPlane, 20.0)
        };
        let s = line_sketch(
            &mut m,
            plane,
            DVec3::new(20.0, 6.0, 10.0),
            DVec3::new(20.0, 14.0, 10.0),
        );
        let r = rib(&mut m, s, direction);
        set_rib(&mut m, r, |p| p.draft = Some(5f64.to_radians()));
        valid(&m);
        assert_close(volume(&m), expected);
    }
}

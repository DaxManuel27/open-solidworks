use of_model::{
    geom::{DVec2, DVec3},
    sketch::{Sketch, spatial::Geometry3, spatial::Sketch3D},
    *,
};
use std::f64::consts::PI;

fn circles(doc: &mut Document, radii: &[f64]) -> (FeatureId, Vec<u32>) {
    let mut sketch = Sketch::new();
    let ids = radii
        .iter()
        .map(|&r| sketch.add_circle_at(DVec2::ZERO, r))
        .collect();
    let id = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch,
    }));
    (id, ids)
}

/// An L-shaped path on the top plane: 20 mm along +Z, then 15 mm along +X.
fn l_path(doc: &mut Document) -> (FeatureId, Vec<u32>) {
    let mut sketch = Sketch::new();
    let start = sketch.add_point(DVec2::ZERO);
    let corner = sketch.add_point(DVec2::new(0., -20.));
    let end = sketch.add_point(DVec2::new(15., -20.));
    let first = sketch.add_line(start, corner).unwrap();
    let second = sketch.add_line(corner, end).unwrap();
    let id = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::TopPlane,
        sketch,
    }));
    (id, vec![first, second])
}

fn sweep(doc: &mut Document, profile: ProfileSelection, path: Ref) -> FeatureId {
    doc.add(FeatureKind::Sweep(Sweep {
        profile: Some(profile),
        inner_profiles: vec![],
        circle_diameter: None,
        twist: 0.0,
        guide: None,
        guide_scale: false,
        guide2: None,
        thin: None,
        path,
        reverse_path: false,
        orientation: SweepOrientation::MinimumTwist,
        cut: false,
        merge: true,
    }))
}

fn volume(model: &Model) -> f64 {
    assert!(
        model.statuses().iter().all(|s| s.error.is_none()),
        "{:?}",
        model.statuses()
    );
    let shape = &model.state().body.as_ref().unwrap().shape;
    assert!(shape.is_valid());
    shape.mass_props().unwrap().volume
}

#[test]
fn sketch_path_with_a_sharp_corner_is_mitred() {
    let mut doc = Document::new();
    let (profile, _) = circles(&mut doc, &[1.]);
    let (path, _) = l_path(&mut doc);
    sweep(&mut doc, profile.into(), Ref::Feature { id: path });
    let model = Model::new(doc);
    assert!((volume(&model) - PI * 35.).abs() < 1e-3);
}

#[test]
fn composite_path_with_a_sharp_corner_is_mitred() {
    let mut doc = Document::new();
    let (profile, _) = circles(&mut doc, &[1.]);
    let (path, lines) = l_path(&mut doc);
    let curve = doc.add(FeatureKind::CompositeCurve(CompositeCurve {
        segments: lines
            .iter()
            .map(|&entity| CurveSelection {
                reference: Ref::SketchEntity {
                    sketch: path,
                    entity,
                },
                reverse: false,
            })
            .collect(),
    }));
    sweep(&mut doc, profile.into(), Ref::Feature { id: curve });
    let model = Model::new(doc);
    assert!((volume(&model) - PI * 35.).abs() < 1e-3);
}

#[test]
fn spatial_path_with_sharp_corners_is_mitred() {
    let mut doc = Document::new();
    let (profile, _) = circles(&mut doc, &[1.]);
    let mut path = Sketch3D::default();
    let points: Vec<_> = [
        DVec3::ZERO,
        DVec3::Z * 20.,
        DVec3::new(15., 0., 20.),
        DVec3::new(15., 10., 20.),
    ]
    .into_iter()
    .map(|p| path.add_point(p))
    .collect();
    for pair in points.windows(2) {
        path.add(Geometry3::Line {
            a: pair[0],
            b: pair[1],
        });
    }
    let path = doc.add(FeatureKind::Sketch3d(path));
    sweep(&mut doc, profile.into(), Ref::Feature { id: path });
    let model = Model::new(doc);
    assert!((volume(&model) - PI * 45.).abs() < 1e-3);
}

#[test]
fn hollow_sweep_turns_a_sharp_corner() {
    let mut doc = Document::new();
    let (profile, ids) = circles(&mut doc, &[3., 2.]);
    let (path, _) = l_path(&mut doc);
    let contour = |entity| ContourSelection {
        sketch: profile,
        entities: vec![entity],
        reverse: false,
        start: Some(entity),
        seam: None,
    };
    doc.add(FeatureKind::Sweep(Sweep {
        profile: Some(ProfileSelection::Contour(contour(ids[0]))),
        inner_profiles: vec![contour(ids[1])],
        circle_diameter: None,
        twist: 0.0,
        guide: None,
        guide_scale: false,
        guide2: None,
        thin: None,
        path: Ref::Feature { id: path },
        reverse_path: false,
        orientation: SweepOrientation::MinimumTwist,
        cut: false,
        merge: true,
    }));
    let model = Model::new(doc);
    assert!((volume(&model) - PI * 5. * 35.).abs() < 1e-3);
}

#[test]
fn self_intersecting_sweep_reports_an_error() {
    let mut doc = Document::new();
    let (profile, _) = circles(&mut doc, &[30.]);
    let mut path = Sketch::new();
    let start = path.add_point(DVec2::ZERO);
    let corner = path.add_point(DVec2::new(0., -30.));
    path.add_line(start, corner).unwrap();
    let center = path.add_point(DVec2::new(10., -30.));
    let end = path.add_point(DVec2::new(10., -40.));
    path.add_arc(center, corner, end).unwrap();
    let path = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::TopPlane,
        sketch: path,
    }));
    let id = sweep(&mut doc, profile.into(), Ref::Feature { id: path });
    let model = Model::new(doc);
    let error = model
        .statuses()
        .into_iter()
        .find(|s| s.id == id)
        .and_then(|s| s.error)
        .expect("the sweep must fail");
    assert!(error.contains("intersects itself"), "{error}");
}

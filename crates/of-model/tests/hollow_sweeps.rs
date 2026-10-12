use of_model::{
    geom::{DVec2, Kind},
    sketch::{Geometry, Sketch},
    *,
};
use std::f64::consts::PI;

fn contour(sketch: FeatureId, entity: u32) -> ContourSelection {
    ContourSelection {
        sketch,
        entities: vec![entity],
        reverse: false,
        start: Some(entity),
        seam: None,
    }
}
fn fixture(curved: bool, cut: bool) -> (Model, FeatureId, FeatureId, u32) {
    let mut doc = Document::new();
    if cut {
        let mut sk = Sketch::new();
        sk.add_circle_at(DVec2::ZERO, 5.);
        let sketch = doc.add(FeatureKind::Sketch(SketchFeature {
            plane: Ref::FrontPlane,
            sketch: sk,
        }));
        doc.add(FeatureKind::Extrude(Extrude {
            sketch,
            cut: false,
            start_offset: 0.,
            end1: EndCondition::Blind { depth: 10. },
            reverse: false,
            end2: None,
            thin: None,
            flip_side_to_cut: false,
            merge: true,
        }));
    }
    let mut sk = Sketch::new();
    let outer = sk.add_circle_at(DVec2::ZERO, 3.);
    let inner = sk.add_circle_at(DVec2::ZERO, 2.);
    let profile = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    let mut path = Sketch::new();
    if curved {
        let center = path.add_point(DVec2::new(10., 0.));
        let start = path.add_point(DVec2::ZERO);
        let end = path.add_point(DVec2::new(10., -10.));
        path.add_arc(center, start, end).unwrap();
    } else {
        path.add_line_at(DVec2::ZERO, DVec2::new(0., -10.));
    }
    let path = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::TopPlane,
        sketch: path,
    }));
    let sweep = doc.add(FeatureKind::Sweep(Sweep {
        profile: Some(ProfileSelection::Contour(contour(profile, outer))),
        inner_profiles: vec![contour(profile, inner)],
        circle_diameter: None,
        twist: 0.0,
        guide: None,
        guide_scale: false,
        guide2: None,
        thin: None,
        path: Ref::Feature { id: path },
        reverse_path: false,
        orientation: SweepOrientation::MinimumTwist,
        cut,
        merge: true,
    }));
    (Model::new(doc), profile, sweep, inner)
}
fn valid(model: &Model) {
    assert!(
        model.statuses().iter().all(|s| s.error.is_none()),
        "{:?}",
        model.statuses()
    );
    assert!(model.state().body.as_ref().unwrap().shape.is_valid());
}
fn volume(model: &Model) -> f64 {
    model
        .state()
        .body
        .as_ref()
        .unwrap()
        .shape
        .mass_props()
        .unwrap()
        .volume
}

#[test]
fn hollow_sweep_has_measured_volume_and_stable_inner_faces_after_edit_and_reload() {
    for curved in [false, true] {
        let (mut model, profile, sweep, inner) = fixture(curved, false);
        valid(&model);
        let length = if curved { 5. * PI } else { 10. };
        assert!((volume(&model) - 5. * PI * length).abs() < 1e-4);
        let inner_face = Ref::Face {
            name: format!("{sweep}.s{profile}.e{inner}"),
        };
        assert!(model.resolve(&inner_face).is_ok());
        if let FeatureKind::Sketch(s) = &mut model.doc.feature_mut(profile).unwrap().kind {
            let Geometry::Circle { radius, .. } =
                &mut s.sketch.entities.get_mut(&inner).unwrap().geo
            else {
                unreachable!()
            };
            *radius = 1.;
        }
        model.invalidate(profile);
        model.rebuild();
        valid(&model);
        let expected = 8. * PI * length;
        assert!((volume(&model) - expected).abs() < 1e-4);
        assert!(model.resolve(&inner_face).is_ok());
        let params = model.doc.feature(sweep).unwrap().kind.clone();
        let preview = model.preview(&params, Some(sweep)).unwrap();
        assert!((preview.body.unwrap().shape.mass_props().unwrap().volume - expected).abs() < 1e-4);
        let reopened = Model::new(file::from_str(&file::to_string(&model.doc)).unwrap());
        valid(&reopened);
        assert!((volume(&reopened) - expected).abs() < 1e-4);
        assert!(reopened.resolve(&inner_face).is_ok());
    }
}

#[test]
fn hollow_swept_cut_removes_only_annular_material() {
    let (model, _, _, _) = fixture(false, true);
    valid(&model);
    assert!((volume(&model) - 200. * PI).abs() < 1e-5);
    assert_eq!(
        model
            .state()
            .body
            .as_ref()
            .unwrap()
            .shape
            .count(Kind::Solid),
        2
    );
}

#[test]
fn outside_duplicate_nested_and_replaced_holes_are_rejected() {
    for mode in ["outside", "duplicate", "outer", "nested", "replaced"] {
        let (mut model, profile, sweep, inner) = fixture(false, false);
        if mode == "duplicate" || mode == "outer" {
            if let FeatureKind::Sweep(p) = &mut model.doc.feature_mut(sweep).unwrap().kind {
                if mode == "duplicate" {
                    p.inner_profiles.push(p.inner_profiles[0].clone());
                } else {
                    let Some(ProfileSelection::Contour(outer)) = &p.profile else {
                        unreachable!()
                    };
                    p.inner_profiles = vec![outer.clone()];
                }
            }
        } else {
            let FeatureKind::Sketch(s) = &mut model.doc.feature_mut(profile).unwrap().kind else {
                unreachable!()
            };
            if mode == "nested" {
                let nested = s.sketch.add_circle_at(DVec2::ZERO, 1.);
                if let FeatureKind::Sweep(p) = &mut model.doc.feature_mut(sweep).unwrap().kind {
                    p.inner_profiles.push(contour(profile, nested));
                }
            } else if mode == "outside" {
                let Geometry::Circle { center, .. } = s.sketch.entities[&inner].geo else {
                    unreachable!()
                };
                s.sketch.entities.get_mut(&center).unwrap().geo =
                    Geometry::Point { p: DVec2::X * 10. };
            } else {
                s.sketch.delete(&[inner]);
                s.sketch.add_circle_at(DVec2::ZERO, 2.);
            }
        }
        model.rebuild_all();
        let error = model.status(sweep).unwrap().error.unwrap();
        assert!(
            match mode {
                "outside" => error.contains("strictly inside"),
                "duplicate" => error.contains("only once"),
                "outer" => error.contains("cannot also be"),
                "nested" => error.contains("cannot be nested"),
                _ => error.contains("selected contour changed"),
            },
            "{mode}: {error}"
        );
        assert!(model.state().body.is_none());
    }
}

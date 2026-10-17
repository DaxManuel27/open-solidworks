//! Weldments: profiles, the Weldment feature, structural members, trims,
//! end caps, gussets, weld beads and cut lists.

use of_model::geom::DVec2;
use of_model::sketch::{Sketch, tools};
use of_model::weldment::*;
use of_model::*;

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() < tol, "{a} != {b}");
}

#[test]
fn custom_profiles_come_from_a_part_sketch() {
    let dir = std::env::temp_dir().join(format!("of-weld-profile-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("tee.ofp");
    // A 30 × 20 rectangle with a 10 × 6 slot cut from it, off center.
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::new(100.0, 50.0), DVec2::new(130.0, 70.0));
    tools::corner_rectangle(&mut sk, DVec2::new(110.0, 57.0), DVec2::new(120.0, 63.0));
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    of_model::file::save(&doc, &path).unwrap();
    let section = section(&ProfileSource::Custom {
        path: path.to_string_lossy().into_owned(),
        sketch: s,
    })
    .unwrap();
    close(section.size.x, 30.0, 1e-9);
    close(section.size.y, 20.0, 1e-9);
    let (area, centroid, _, _) = profiles::properties(&section);
    close(area, 600.0 - 60.0, 1e-6);
    // Centered on its bounding box.
    close(centroid.x, 0.0, 1e-6);
    close(centroid.y, 0.0, 1e-6);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_part_has_one_weldment_feature() {
    let mut doc = Document::new();
    doc.add(FeatureKind::Weldment(WeldmentDef::default()));
    let second = doc.add(FeatureKind::Weldment(WeldmentDef::default()));
    let m = Model::new(doc);
    let status = m.statuses().into_iter().find(|s| s.id == second).unwrap();
    assert!(status.error.unwrap().contains("one Weldment"));
    assert!(m.state().weldment.is_some());
}

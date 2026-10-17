//! Surface-body kernel operations: knit, solidify, offset, thicken, delete
//! and patch faces, and filled patches.

use of_geom::{Continuity, DVec3, FillEdge, Kind, Shape};

fn faces(s: &Shape) -> Vec<Shape> {
    (0..s.count(Kind::Face))
        .map(|i| s.sub_shape(Kind::Face, i).unwrap())
        .collect()
}

#[test]
fn knit_box_faces_into_a_solid() {
    let b = Shape::make_box(10.0, 20.0, 30.0).unwrap();
    // Copies, so the faces share no edges and must really be sewn.
    let loose: Vec<Shape> = faces(&b)
        .iter()
        .map(|f| f.offset_shape(0.0).unwrap().shape)
        .collect();
    let knitted = Shape::compound(&loose).sew(1e-4).unwrap();
    assert_eq!(knitted.free_edges, 0);
    assert_eq!(knitted.result.shape.count(Kind::Face), 6);
    assert_eq!(knitted.result.shape.count(Kind::Edge), 12);
    assert_eq!(knitted.result.shape.count(Kind::Solid), 0);
    let solid = knitted.result.shape.solidify().unwrap();
    assert_eq!(solid.shape.count(Kind::Solid), 1);
    let v = solid.shape.mass_props().unwrap().volume;
    assert!((v - 6000.0).abs() < 1e-6, "volume {v}");
    // Every face is traced through the sew and the solidify.
    let traced = solid
        .history
        .iter()
        .filter(|h| h.from_kind == Kind::Face)
        .count();
    assert_eq!(traced, 6);
}

#[test]
fn an_open_shell_reports_free_edges_and_wont_solidify() {
    let b = Shape::make_box(10.0, 10.0, 10.0).unwrap();
    let top = (0..6)
        .find(|i| b.face_info(*i).unwrap().normal.z > 0.5)
        .unwrap();
    let open = b.remove_faces(&[top]).unwrap().shape;
    assert_eq!(open.count(Kind::Solid), 0);
    assert_eq!(open.count(Kind::Face), 5);
    assert!(open.has_open_faces());
    assert_eq!(open.free_edges().unwrap().count(Kind::Edge), 4);
    let err = open
        .solidify()
        .err()
        .expect("an open shell can't be a solid");
    assert!(err.0.contains("gaps"), "{}", err.0);
}

#[test]
fn offset_and_thicken_a_face() {
    let b = Shape::make_box(10.0, 10.0, 10.0).unwrap();
    let top = (0..6)
        .find(|i| b.face_info(*i).unwrap().normal.z > 0.5)
        .unwrap();
    let face = b.sub_shape(Kind::Face, top).unwrap();
    let up = face.offset_shape(3.0).unwrap().shape;
    let info = up.face_info(0).unwrap();
    assert!((info.centroid.z - 13.0).abs() < 1e-6, "{:?}", info.centroid);
    let thick = face.thicken(2.0).unwrap().shape;
    assert_eq!(thick.count(Kind::Solid), 1);
    let m = thick.mass_props().unwrap();
    assert!((m.volume - 200.0).abs() < 1e-6, "volume {}", m.volume);
    assert!((m.center.z - 11.0).abs() < 1e-6, "center {:?}", m.center);
    let down = face.thicken(-2.0).unwrap().shape;
    assert!((down.mass_props().unwrap().center.z - 9.0).abs() < 1e-6);
}

#[test]
fn defeature_removes_a_fillet() {
    let b = Shape::make_box(20.0, 20.0, 20.0).unwrap();
    let rounded = b.fillet(&[0], 3.0).unwrap().shape;
    let fillet = (0..rounded.count(Kind::Face))
        .find(|i| rounded.face_info(*i).unwrap().kind == of_geom::SurfaceKind::Cylinder)
        .unwrap();
    let healed = rounded.defeature(&[fillet]).unwrap();
    assert_eq!(healed.shape.count(Kind::Face), 6);
    let v = healed.shape.mass_props().unwrap().volume;
    assert!((v - 8000.0).abs() < 1e-6, "volume {v}");
}

#[test]
fn fill_closes_a_square_hole() {
    let b = Shape::make_box(10.0, 10.0, 10.0).unwrap();
    let top = (0..6)
        .find(|i| b.face_info(*i).unwrap().normal.z > 0.5)
        .unwrap();
    let open = b.remove_faces(&[top]).unwrap().shape;
    let rim = open.free_edges().unwrap();
    let edges: Vec<Shape> = (0..rim.count(Kind::Edge))
        .map(|i| rim.sub_shape(Kind::Edge, i).unwrap())
        .collect();
    let boundary: Vec<FillEdge> = edges
        .iter()
        .map(|e| FillEdge {
            edge: e,
            support: None,
            continuity: Continuity::Contact,
        })
        .collect();
    let (patch, error) = Shape::fill(&boundary, &[]).unwrap();
    assert!(error < 1e-4, "boundary error {error}");
    let info = patch.face_info(0).unwrap();
    assert!((info.centroid - DVec3::new(5.0, 5.0, 10.0)).length() < 1e-4);
    assert!((info.area - 100.0).abs() < 1e-3, "area {}", info.area);
    // The patch closes the box again.
    let closed = Shape::compound([&open, &patch]).sew(1e-4).unwrap();
    assert_eq!(closed.free_edges, 0);
    let v = closed
        .result
        .shape
        .solidify()
        .unwrap()
        .shape
        .mass_props()
        .unwrap()
        .volume;
    assert!((v - 1000.0).abs() < 1e-3, "volume {v}");
}

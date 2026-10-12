use of_geom::{DAffine3, DVec3, Kind, Plane, Shape};

#[test]
fn splitter_partitions_by_solids_faces_and_planes_with_tool_history() {
    let source = Shape::make_box(10.0, 10.0, 10.0).unwrap();
    let tool = Shape::make_box(2.0, 20.0, 20.0)
        .unwrap()
        .transform(&DAffine3::from_translation(DVec3::new(4.0, -5.0, -5.0)))
        .unwrap()
        .shape;
    let result = source.split_tools(&tool).unwrap();
    assert!(result.shape.is_valid());
    assert_eq!(result.shape.count(Kind::Solid), 3);
    assert!((result.shape.mass_props().unwrap().volume - 1000.0).abs() < 1e-7);
    assert!(
        result
            .history
            .iter()
            .any(|h| h.input == 1 && h.from_kind == Kind::Face && h.to_kind == Kind::Face)
    );
    let plane = Shape::plane_face(&Plane::right().offset(4.0)).unwrap();
    let result = source.split_tools(&plane).unwrap();
    assert_eq!(result.shape.count(Kind::Solid), 2);
    let boundary = result
        .history
        .iter()
        .find(|h| h.input == 1 && h.to_kind == Kind::Face)
        .unwrap()
        .to;
    let mut orientations = Vec::new();
    for solid in result.shape.solids() {
        let indices = solid.indices_in(&result.shape, Kind::Face).unwrap();
        let local = indices.iter().position(|&i| i == boundary).unwrap() as u32;
        orientations.push(solid.face_orientation(local).unwrap());
    }
    orientations.sort();
    assert_eq!(orientations, [-1, 1]);
    let face = tool.sub_shape(Kind::Face, 0).unwrap();
    assert_eq!(
        source.split_tools(&face).unwrap().shape.count(Kind::Solid),
        2
    );
    let outside = Shape::plane_face(&Plane::right().offset(14.0)).unwrap();
    assert_eq!(
        source
            .split_tools(&outside)
            .unwrap()
            .shape
            .count(Kind::Solid),
        1
    );
}

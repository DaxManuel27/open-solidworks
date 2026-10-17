//! Mesh bodies: import, convert, decimate, segment, prep, facet references,
//! surfaces and solids fitted to meshes, and sections.

use base64::Engine;
use of_model::meshes::*;
use of_model::{
    geom::{DVec2, DVec3, Kind},
    sketch::{Sketch, tools},
    *,
};

fn ok(m: &Model) {
    for s in m.statuses() {
        assert!(s.error.is_none(), "feature {} failed: {:?}", s.id, s.error);
    }
}

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() < tol, "{a} != {b}");
}

/// A 10 mm cube as triangles, outward.
fn cube(size: f64) -> of_mesh::TriMesh {
    let p = (0..8)
        .map(|i| DVec3::new((i & 1) as f64, ((i >> 1) & 1) as f64, ((i >> 2) & 1) as f64) * size)
        .collect();
    let quads = [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ];
    let t = quads
        .iter()
        .flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]])
        .collect();
    of_mesh::TriMesh::new(p, t)
}

fn import(format: ImportFormat, bytes: &[u8], mode: MeshImportMode) -> Model {
    let mut doc = Document::new();
    doc.add(FeatureKind::Import(Import {
        format,
        source: "scan".into(),
        data: base64::engine::general_purpose::STANDARD.encode(bytes),
        mesh: Some(MeshImport {
            mode,
            ..Default::default()
        }),
    }));
    let m = Model::new(doc);
    ok(&m);
    m
}

fn with(m: &Model, kind: FeatureKind) -> Model {
    let mut doc = m.doc.clone();
    doc.add(kind);
    let m = Model::new(doc);
    ok(&m);
    m
}

fn volume(m: &Model) -> f64 {
    m.state()
        .body
        .as_ref()
        .map_or(0.0, |b| b.shape.mass_props().unwrap().volume)
}

/// A solid: a `w × w × h` block, optionally with a hole of `r` through it.
fn block(w: f64, h: f64, hole: Option<f64>) -> Model {
    let mut doc = Document::new();
    let mut sk = Sketch::new();
    tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(w, w));
    if let Some(r) = hole {
        let c = sk.add_point(DVec2::splat(w / 2.0));
        sk.add_circle(c, r).unwrap();
    }
    let s = doc.add(FeatureKind::Sketch(SketchFeature {
        plane: Ref::FrontPlane,
        sketch: sk,
    }));
    doc.add(FeatureKind::Extrude(Extrude {
        sketch: s,
        cut: false,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth: h },
        reverse: false,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }));
    let m = Model::new(doc);
    ok(&m);
    m
}

fn as_mesh(m: &Model, chord: f64) -> Model {
    let body = m.state().bodies[0].id.clone();
    with(
        m,
        FeatureKind::ConvertToMesh(ConvertToMesh {
            body,
            keep: false,
            chord,
            angle: 0.3,
        }),
    )
}

#[test]
fn an_stl_imports_as_graphics_surface_or_solid() {
    let stl = of_mesh::io::write_stl(&cube(10.0));
    let g = import(ImportFormat::Stl, &stl, MeshImportMode::Graphics);
    assert_eq!(g.state().meshes.len(), 1);
    assert_eq!(g.state().meshes[0].mesh.triangles.len(), 12);
    assert!(g.state().bodies.is_empty());
    let s = import(ImportFormat::Stl, &stl, MeshImportMode::Surface);
    assert_eq!(s.state().surfaces.len(), 1);
    let solid = import(ImportFormat::Stl, &stl, MeshImportMode::Solid);
    close(volume(&solid), 1000.0, 1e-3);
    // Flat runs of triangles become one face each.
    assert_eq!(
        solid.state().body.as_ref().unwrap().shape.count(Kind::Face),
        6
    );
    // Facets kept as they are; an inside-out mesh still makes the solid.
    let mut inverted = cube(10.0);
    for t in &mut inverted.triangles {
        t.swap(1, 2);
    }
    for m in [cube(10.0), inverted] {
        let mut doc = Document::new();
        doc.add(FeatureKind::Import(Import {
            format: ImportFormat::Stl,
            source: "scan".into(),
            data: base64::engine::general_purpose::STANDARD.encode(of_mesh::io::write_stl(&m)),
            mesh: Some(MeshImport {
                mode: MeshImportMode::Solid,
                group_facets: false,
                ..Default::default()
            }),
        }));
        let facets = Model::new(doc);
        ok(&facets);
        close(volume(&facets), 1000.0, 1e-6);
        let body = facets.state().body.as_ref().unwrap();
        assert_eq!(body.shape.count(Kind::Face), 12);
        assert!(body.shape.is_valid());
    }
}

#[test]
fn a_point_cloud_becomes_a_mesh() {
    let n = 2000;
    let golden = std::f64::consts::PI * (3.0 - 5f64.sqrt());
    let text: String = (0..n)
        .map(|i| {
            let y = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
            let r = (1.0 - y * y).sqrt();
            let a = golden * i as f64;
            format!(
                "{} {} {}\n",
                a.cos() * r * 10.0,
                y * 10.0,
                a.sin() * r * 10.0
            )
        })
        .collect();
    let m = import(
        ImportFormat::Points,
        text.as_bytes(),
        MeshImportMode::Graphics,
    );
    assert!(m.state().meshes[0].mesh.triangles.len() > 500);
}

#[test]
fn convert_decimate_and_segment() {
    let m = as_mesh(&block(20.0, 10.0, Some(4.0)), 0.01);
    assert!(m.state().bodies.is_empty());
    let mesh = &m.state().meshes[0].mesh;
    let expected = 4000.0 - std::f64::consts::PI * 16.0 * 10.0;
    close(mesh.volume(), expected, expected * 0.01);
    let before = mesh.triangles.len();
    let body = m.state().meshes[0].id.clone();
    let d = with(
        &m,
        FeatureKind::DecimateMesh(DecimateMesh {
            body: body.clone(),
            target: DecimateTarget::Fraction { fraction: 0.5 },
            preserve_boundary: true,
            facets: vec![],
        }),
    );
    assert!(d.state().meshes[0].mesh.triangles.len() < before);
    let seg = with(
        &m,
        FeatureKind::SegmentMesh(SegmentMesh {
            body,
            angle: 20f64.to_radians(),
            min_area: 0.0,
        }),
    );
    let groups = &seg.state().meshes[0].mesh.groups;
    // Six flat sides and the hole's wall.
    assert_eq!(groups.iter().copied().max().unwrap() + 1, 7);
}

#[test]
fn planes_and_axes_fit_facets_and_surfaces_fit_regions() {
    let m = as_mesh(&block(20.0, 10.0, Some(4.0)), 0.005);
    let mesh = m.state().meshes[0].mesh.clone();
    let body = m.state().meshes[0].id.clone();
    let top: Vec<u32> = (0..mesh.triangles.len() as u32)
        .filter(|t| mesh.normal(*t as usize).z > 0.99)
        .collect();
    let wall: Vec<u32> = (0..mesh.triangles.len() as u32)
        .filter(|t| {
            let n = mesh.normal(*t as usize);
            let [a, b, c] = mesh.corners(*t as usize);
            let mid = (a + b + c) / 3.0;
            n.z.abs() < 0.1 && (DVec2::new(mid.x, mid.y) - DVec2::splat(10.0)).length() < 5.0
        })
        .collect();
    let mut doc = m.doc.clone();
    let plane = doc.add(FeatureKind::Plane(PlaneDef::Facets {
        body: body.clone(),
        facets: top,
    }));
    let axis = doc.add(FeatureKind::Axis(AxisDef::Facets {
        body: body.clone(),
        facets: wall.clone(),
    }));
    doc.add(FeatureKind::SurfaceFromMesh(SurfaceFromMesh {
        body,
        facets: wall,
        group: None,
        fit: FitKind::Auto,
        extend: 0.0,
    }));
    let f = Model::new(doc);
    ok(&f);
    let Datum::Plane(p) = f.state().datums[&plane] else {
        panic!()
    };
    close(p.normal.z.abs(), 1.0, 1e-9);
    close(p.origin.z, 10.0, 1e-6);
    let Datum::Axis { origin, dir } = f.state().datums[&axis] else {
        panic!()
    };
    close(dir.z.abs(), 1.0, 1e-6);
    close(
        (DVec2::new(origin.x, origin.y) - DVec2::splat(10.0)).length(),
        0.0,
        1e-3,
    );
    let face = f.state().surfaces[0].geometry.shape.face_info(0).unwrap();
    assert_eq!(face.kind, of_model::geom::SurfaceKind::Cylinder);
    close(face.radius, 4.0, 1e-2);
}

#[test]
fn a_closed_scan_becomes_a_solid() {
    let m = as_mesh(&block(20.0, 10.0, Some(4.0)), 0.005);
    let body = m.state().meshes[0].id.clone();
    let s = with(
        &m,
        FeatureKind::MeshToSolid(MeshToSolid {
            body,
            angle: 20f64.to_radians(),
            min_area: 0.0,
        }),
    );
    let expected = 4000.0 - std::f64::consts::PI * 16.0 * 10.0;
    close(volume(&s), expected, expected * 0.002);
    let faces = s.state().body.as_ref().unwrap().shape.count(Kind::Face);
    assert_eq!(faces, 7, "six flat faces and a cylinder");
}

#[test]
fn sections_and_prep() {
    let m = as_mesh(&block(20.0, 10.0, None), 0.01);
    let body = m.state().meshes[0].id.clone();
    let s = with(
        &m,
        FeatureKind::MeshSections(MeshSections {
            body: body.clone(),
            plane: Ref::FrontPlane,
            count: 3,
            spacing: 3.0,
            offset: 2.0,
        }),
    );
    let id = s.doc.features.last().unwrap().id;
    assert_eq!(s.state().curves[&id].segments.len(), 3);
    // A cube missing two triangles, healed.
    let mut holed = cube(10.0);
    holed.triangles.truncate(10);
    let stl = of_mesh::io::write_stl(&holed);
    let g = import(ImportFormat::Stl, &stl, MeshImportMode::Graphics);
    let id = g.state().meshes[0].id.clone();
    let healed = with(
        &g,
        FeatureKind::MeshPrep(MeshPrep {
            body: id,
            steps: vec![
                PrepStep::FillHoles { max_edges: 10 },
                PrepStep::Smooth {
                    iterations: 1,
                    boundary: false,
                },
            ],
        }),
    );
    assert!(healed.state().meshes[0].mesh.is_closed());
}

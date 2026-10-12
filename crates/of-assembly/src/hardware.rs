//! Standard hardware: fasteners generated as ordinary parametric parts from
//! dimension tables in the published standards (ISO 4762, ISO 4014,
//! ISO 4017, ISO 4032, ISO 7089, ISO 8734, ASME B18.3).
//!
//! Every part shares one convention so it mates predictably: the axis is Z,
//! the bearing face (under the head, or the nut's or washer's base) lies on
//! the XY plane, heads extend toward +Z and shanks toward −Z.

use std::path::{Path, PathBuf};

use of_model::geom::{DVec2, Kind, SurfaceKind};
use of_model::properties::Property;
use of_model::sketch::{Sketch, tools};
use of_model::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Kinds of standard hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HardwareKind {
    /// ISO 4762 hexagon socket head cap screw (metric).
    SocketHeadCapScrew,
    /// ASME B18.3 hexagon socket head cap screw (inch).
    SocketHeadCapScrewInch,
    /// ISO 4014 hexagon head bolt, partly threaded.
    HexBolt,
    /// ISO 4017 hexagon head screw, threaded to the head.
    HexScrew,
    /// ISO 4032 hexagon nut.
    HexNut,
    /// ISO 7089 plain washer, normal series.
    Washer,
    /// ISO 8734 parallel dowel pin.
    DowelPin,
}

/// What to generate: a kind, a size from its table and, for screws and
/// pins, a length in mm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HardwareSpec {
    pub kind: HardwareKind,
    /// Size designation from the catalog, such as `M6` or `1/4-20`.
    pub size: String,
    /// Length under the head (screws) or overall (pins), mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<f64>,
}

/// Screw dimensions, mm: nominal diameter, pitch, head diameter or across
/// flats, head height, socket across flats, socket depth.
struct Screw {
    size: &'static str,
    d: f64,
    pitch: f64,
    head: f64,
    k: f64,
    socket: f64,
    depth: f64,
}

const fn screw(
    size: &'static str,
    d: f64,
    pitch: f64,
    head: f64,
    k: f64,
    socket: f64,
    depth: f64,
) -> Screw {
    Screw {
        size,
        d,
        pitch,
        head,
        k,
        socket,
        depth,
    }
}

/// ISO 4762: d, P, dk, k, s, t.
const ISO_4762: &[Screw] = &[
    screw("M3", 3.0, 0.5, 5.5, 3.0, 2.5, 1.3),
    screw("M4", 4.0, 0.7, 7.0, 4.0, 3.0, 2.0),
    screw("M5", 5.0, 0.8, 8.5, 5.0, 4.0, 2.5),
    screw("M6", 6.0, 1.0, 10.0, 6.0, 5.0, 3.0),
    screw("M8", 8.0, 1.25, 13.0, 8.0, 6.0, 4.0),
    screw("M10", 10.0, 1.5, 16.0, 10.0, 8.0, 5.0),
    screw("M12", 12.0, 1.75, 18.0, 12.0, 10.0, 6.0),
    screw("M16", 16.0, 2.0, 24.0, 16.0, 14.0, 8.0),
    screw("M20", 20.0, 2.5, 30.0, 20.0, 17.0, 10.0),
];

const IN: f64 = 25.4;

/// ASME B18.3: d, 1/TPI, head diameter, head height, socket, socket depth (in).
const ASME_B18_3: &[Screw] = &[
    screw(
        "#10-24",
        0.190 * IN,
        IN / 24.0,
        0.312 * IN,
        0.190 * IN,
        0.15625 * IN,
        0.095 * IN,
    ),
    screw(
        "1/4-20",
        0.250 * IN,
        IN / 20.0,
        0.375 * IN,
        0.250 * IN,
        0.1875 * IN,
        0.120 * IN,
    ),
    screw(
        "5/16-18",
        0.3125 * IN,
        IN / 18.0,
        0.469 * IN,
        0.3125 * IN,
        0.25 * IN,
        0.151 * IN,
    ),
    screw(
        "3/8-16",
        0.375 * IN,
        IN / 16.0,
        0.5625 * IN,
        0.375 * IN,
        0.3125 * IN,
        0.182 * IN,
    ),
    screw(
        "1/2-13",
        0.5 * IN,
        IN / 13.0,
        0.75 * IN,
        0.5 * IN,
        0.375 * IN,
        0.245 * IN,
    ),
];

/// ISO 4014 / ISO 4017 heads: d, P, s (across flats), k. ISO 4032 nut height
/// in the socket column.
const ISO_HEX: &[Screw] = &[
    screw("M3", 3.0, 0.5, 5.5, 2.0, 2.4, 0.0),
    screw("M4", 4.0, 0.7, 7.0, 2.8, 3.2, 0.0),
    screw("M5", 5.0, 0.8, 8.0, 3.5, 4.7, 0.0),
    screw("M6", 6.0, 1.0, 10.0, 4.0, 5.2, 0.0),
    screw("M8", 8.0, 1.25, 13.0, 5.3, 6.8, 0.0),
    screw("M10", 10.0, 1.5, 16.0, 6.4, 8.4, 0.0),
    screw("M12", 12.0, 1.75, 18.0, 7.5, 10.8, 0.0),
    screw("M16", 16.0, 2.0, 24.0, 10.0, 14.8, 0.0),
    screw("M20", 20.0, 2.5, 30.0, 12.5, 18.0, 0.0),
];

/// ISO 7089: size, inner diameter, outer diameter, thickness.
const ISO_7089: &[(&str, f64, f64, f64)] = &[
    ("M3", 3.2, 7.0, 0.5),
    ("M4", 4.3, 9.0, 0.8),
    ("M5", 5.3, 10.0, 1.0),
    ("M6", 6.4, 12.0, 1.6),
    ("M8", 8.4, 16.0, 1.6),
    ("M10", 10.5, 20.0, 2.0),
    ("M12", 13.0, 24.0, 2.5),
    ("M16", 17.0, 30.0, 3.0),
    ("M20", 21.0, 37.0, 3.0),
];

/// ISO 8734 diameters.
const ISO_8734: &[(&str, f64)] = &[
    ("2", 2.0),
    ("3", 3.0),
    ("4", 4.0),
    ("5", 5.0),
    ("6", 6.0),
    ("8", 8.0),
    ("10", 10.0),
    ("12", 12.0),
];

/// Preferred lengths, mm.
const LENGTHS: &[f64] = &[
    6.0, 8.0, 10.0, 12.0, 16.0, 20.0, 25.0, 30.0, 35.0, 40.0, 45.0, 50.0, 55.0, 60.0, 65.0, 70.0,
    80.0, 90.0, 100.0, 110.0, 120.0, 130.0, 140.0, 150.0,
];

/// Preferred inch lengths, mm.
const INCH_LENGTHS: &[f64] = &[
    0.25 * IN,
    0.375 * IN,
    0.5 * IN,
    0.625 * IN,
    0.75 * IN,
    IN,
    1.25 * IN,
    1.5 * IN,
    1.75 * IN,
    2.0 * IN,
    2.5 * IN,
    3.0 * IN,
];

/// One catalog entry for pickers.
#[derive(Debug, Clone, Serialize)]
pub struct CatalogEntry {
    pub kind: HardwareKind,
    pub standard: &'static str,
    pub name: &'static str,
    pub sizes: Vec<&'static str>,
    /// Preferred lengths in mm; empty when the part has no length.
    pub lengths: Vec<f64>,
}

pub fn catalog() -> Vec<CatalogEntry> {
    use HardwareKind as K;
    let sizes = |t: &[Screw]| t.iter().map(|s| s.size).collect::<Vec<_>>();
    vec![
        CatalogEntry {
            kind: K::SocketHeadCapScrew,
            standard: "ISO 4762",
            name: "Socket head cap screw",
            sizes: sizes(ISO_4762),
            lengths: LENGTHS.to_vec(),
        },
        CatalogEntry {
            kind: K::SocketHeadCapScrewInch,
            standard: "ASME B18.3",
            name: "Socket head cap screw (inch)",
            sizes: sizes(ASME_B18_3),
            lengths: INCH_LENGTHS.to_vec(),
        },
        CatalogEntry {
            kind: K::HexBolt,
            standard: "ISO 4014",
            name: "Hex head bolt",
            sizes: sizes(ISO_HEX),
            lengths: LENGTHS.iter().copied().filter(|l| *l >= 25.0).collect(),
        },
        CatalogEntry {
            kind: K::HexScrew,
            standard: "ISO 4017",
            name: "Hex head screw",
            sizes: sizes(ISO_HEX),
            lengths: LENGTHS.to_vec(),
        },
        CatalogEntry {
            kind: K::HexNut,
            standard: "ISO 4032",
            name: "Hex nut",
            sizes: sizes(ISO_HEX),
            lengths: vec![],
        },
        CatalogEntry {
            kind: K::Washer,
            standard: "ISO 7089",
            name: "Plain washer",
            sizes: ISO_7089.iter().map(|w| w.0).collect(),
            lengths: vec![],
        },
        CatalogEntry {
            kind: K::DowelPin,
            standard: "ISO 8734",
            name: "Dowel pin",
            sizes: ISO_8734.iter().map(|p| p.0).collect(),
            lengths: LENGTHS.iter().copied().filter(|l| *l <= 100.0).collect(),
        },
    ]
}

fn sketch_on(doc: &mut Document, plane: Ref, build: impl FnOnce(&mut Sketch)) -> FeatureId {
    let mut sk = Sketch::new();
    build(&mut sk);
    doc.add(FeatureKind::Sketch(SketchFeature { plane, sketch: sk }))
}

fn extrude(
    doc: &mut Document,
    sketch: FeatureId,
    depth: f64,
    reverse: bool,
    cut: bool,
) -> FeatureId {
    doc.add(FeatureKind::Extrude(Extrude {
        sketch,
        cut,
        start_offset: 0.0,
        end1: EndCondition::Blind { depth },
        reverse,
        end2: None,
        thin: None,
        flip_side_to_cut: false,
        merge: true,
    }))
}

fn hexagon(sk: &mut Sketch, across_flats: f64) {
    let r = across_flats / 3f64.sqrt();
    tools::polygon(sk, DVec2::ZERO, DVec2::new(r, 0.0), 6).expect("a hexagon is valid");
}

/// The cylindrical face of radius `r` whose axis is Z and that lies below
/// (`below`) or above the XY plane.
fn cylinder(doc: &Document, r: f64, below: bool) -> Result<Ref, String> {
    let mut m = Model::new(doc.clone());
    m.rebuild();
    let state = m.state();
    let shape = &state.body.as_ref().ok_or("the hardware has no body")?.shape;
    for i in 0..shape.count(Kind::Face) {
        let f = shape.face_info(i).map_err(|e| e.0)?;
        if f.kind == SurfaceKind::Cylinder
            && (f.radius - r).abs() < 1e-6
            && ((f.centroid.z < 0.0) == below)
        {
            return state
                .ref_for(Kind::Face, i)
                .ok_or_else(|| "could not name the thread face".into());
        }
    }
    Err("could not find the thread face".into())
}

fn thread(doc: &mut Document, face: Ref, d: f64, pitch: f64, length: f64, internal: bool) {
    doc.add(FeatureKind::CosmeticThread(CosmeticThread {
        face,
        diameter: d,
        pitch,
        length,
        start_offset: 0.0,
        start_angle: 0.0,
        reverse: false,
        left_handed: false,
        internal,
    }));
}

fn lookup<'a>(table: &'a [Screw], size: &str) -> Result<&'a Screw, String> {
    table
        .iter()
        .find(|s| s.size.eq_ignore_ascii_case(size))
        .ok_or_else(|| format!("no size {size} in this standard"))
}

fn length_of(spec: &HardwareSpec, d: f64) -> Result<f64, String> {
    let l = spec.length.ok_or("choose a length")?;
    if !(l > 0.0 && l <= 500.0) {
        return Err("the length must be between 0 and 500 mm".into());
    }
    if l < d {
        return Err(format!("the length must be at least the diameter ({d} mm)"));
    }
    Ok(l)
}

fn fmt(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A hardware part document and the name for its file.
pub fn generate(spec: &HardwareSpec) -> Result<(Document, String), String> {
    use HardwareKind as K;
    let mut doc = Document::new();
    let (standard, name, description) = match spec.kind {
        K::SocketHeadCapScrew | K::SocketHeadCapScrewInch => {
            let (table, standard) = if spec.kind == K::SocketHeadCapScrew {
                (ISO_4762, "ISO 4762")
            } else {
                (ASME_B18_3, "ASME B18.3")
            };
            let s = lookup(table, &spec.size)?;
            let l = length_of(spec, s.d)?;
            let head = sketch_on(&mut doc, Ref::FrontPlane, |sk| {
                sk.add_circle_at(DVec2::ZERO, s.head / 2.0);
            });
            extrude(&mut doc, head, s.k, false, false);
            let shank = sketch_on(&mut doc, Ref::FrontPlane, |sk| {
                sk.add_circle_at(DVec2::ZERO, s.d / 2.0);
            });
            extrude(&mut doc, shank, l, true, false);
            let top = doc.add(FeatureKind::Plane(PlaneDef::Offset {
                reference: Ref::FrontPlane,
                distance: s.k,
                flip: false,
            }));
            let socket = sketch_on(&mut doc, Ref::Feature { id: top }, |sk| {
                hexagon(sk, s.socket)
            });
            extrude(&mut doc, socket, s.depth, true, true);
            let face = cylinder(&doc, s.d / 2.0, true)?;
            thread(&mut doc, face, s.d, s.pitch, l, false);
            let size = if spec.kind == K::SocketHeadCapScrew {
                format!("{}x{}", s.size, fmt(l))
            } else {
                format!("{} x {}\"", s.size, fmt(l / IN))
            };
            (
                standard,
                size.clone(),
                format!("Socket head cap screw {size}"),
            )
        }
        K::HexBolt | K::HexScrew => {
            let s = lookup(ISO_HEX, &spec.size)?;
            let l = length_of(spec, s.d)?;
            let head = sketch_on(&mut doc, Ref::FrontPlane, |sk| hexagon(sk, s.head));
            extrude(&mut doc, head, s.k, false, false);
            let shank = sketch_on(&mut doc, Ref::FrontPlane, |sk| {
                sk.add_circle_at(DVec2::ZERO, s.d / 2.0);
            });
            extrude(&mut doc, shank, l, true, false);
            let face = cylinder(&doc, s.d / 2.0, true)?;
            // ISO 4014 thread length b = 2d + 6 for lengths up to 125 mm.
            let b = if spec.kind == K::HexBolt {
                let b = if l <= 125.0 {
                    2.0 * s.d + 6.0
                } else {
                    2.0 * s.d + 12.0
                };
                b.min(l)
            } else {
                l
            };
            thread(&mut doc, face, s.d, s.pitch, b, false);
            let (standard, what) = if spec.kind == K::HexBolt {
                ("ISO 4014", "Hex head bolt")
            } else {
                ("ISO 4017", "Hex head screw")
            };
            let size = format!("{}x{}", s.size, fmt(l));
            (standard, size.clone(), format!("{what} {size}"))
        }
        K::HexNut => {
            let s = lookup(ISO_HEX, &spec.size)?;
            let m = s.socket;
            let body = sketch_on(&mut doc, Ref::FrontPlane, |sk| {
                hexagon(sk, s.head);
                sk.add_circle_at(DVec2::ZERO, s.d / 2.0);
            });
            extrude(&mut doc, body, m, false, false);
            let face = cylinder(&doc, s.d / 2.0, false)?;
            thread(&mut doc, face, s.d, s.pitch, m, true);
            let size = s.size.to_string();
            ("ISO 4032", size.clone(), format!("Hex nut {size}"))
        }
        K::Washer => {
            let (size, d1, d2, h) = *ISO_7089
                .iter()
                .find(|w| w.0.eq_ignore_ascii_case(&spec.size))
                .ok_or_else(|| format!("no size {} in ISO 7089", spec.size))?;
            let body = sketch_on(&mut doc, Ref::FrontPlane, |sk| {
                sk.add_circle_at(DVec2::ZERO, d2 / 2.0);
                sk.add_circle_at(DVec2::ZERO, d1 / 2.0);
            });
            extrude(&mut doc, body, h, false, false);
            ("ISO 7089", size.to_string(), format!("Plain washer {size}"))
        }
        K::DowelPin => {
            let (size, d) = *ISO_8734
                .iter()
                .find(|p| p.0 == spec.size.trim_start_matches(['d', 'D', 'Ø']))
                .ok_or_else(|| format!("no diameter {} in ISO 8734", spec.size))?;
            let l = length_of(spec, d)?;
            let body = sketch_on(&mut doc, Ref::FrontPlane, |sk| {
                sk.add_circle_at(DVec2::ZERO, d / 2.0);
            });
            extrude(&mut doc, body, l, true, false);
            let size = format!("{size}x{}", fmt(l));
            ("ISO 8734", size.clone(), format!("Dowel pin {size}"))
        }
    };
    doc.material = Some(materials::Material {
        name: "Plain Carbon Steel".into(),
        density: 7800.0,
    });
    let part_no = format!("{standard} {name}");
    for (k, v) in [
        ("PartNo", part_no.clone()),
        ("Description", description),
        ("Standard", standard.to_string()),
        ("Size", spec.size.clone()),
    ] {
        doc.properties.insert(k.into(), Property::text(v));
    }
    doc.properties.insert(
        "Hardware".into(),
        Property::text(serde_json::to_string(spec).expect("specs serialize")),
    );
    let file = part_no
        .replace(['/', '\\', '"', ':'], "-")
        .replace('#', "No");
    Ok((doc, file))
}

/// The library file for a spec in `dir`, generated on first use.
pub fn library_part(dir: &Path, spec: &HardwareSpec) -> Result<PathBuf, String> {
    let (doc, name) = generate(spec)?;
    std::fs::create_dir_all(dir).map_err(|e| {
        format!(
            "could not create the hardware folder {}: {e}",
            dir.display()
        )
    })?;
    let path = dir.join(format!("{name}.ofp"));
    let tag = doc.properties.get("Hardware").map(|p| p.value.clone());
    let current = of_model::file::load(&path)
        .ok()
        .and_then(|d| d.properties.get("Hardware").map(|p| p.value.clone()));
    if current.is_none() || current != tag {
        of_model::file::save(&doc, &path)?;
    }
    Ok(path)
}

//! Structural profiles for weldments, generated from the dimension tables of
//! the published standards: EN 10365 (IPE, HEA), EN 10279 (UPN), EN 10056-1
//! (equal angles), EN 10219-2 (square, rectangular and circular hollow
//! sections), AISC (W, C, L, rectangular and round HSS) and ASME B36.10
//! (pipe). Only the facts of the tables are used; the outlines are drawn here.
//!
//! A profile is drawn in its own 2D frame: x across the section, y up, with
//! the bounding box centered on the origin. Fillets follow the tables' root
//! and toe radii; inch sizes are converted to millimetres.

use std::f64::consts::PI;

use of_geom::{Curve2, DVec2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const IN: f64 = 25.4;

/// A profile family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    /// EN 10365 IPE I-beam.
    Ipe,
    /// EN 10365 HEA wide flange beam.
    Hea,
    /// EN 10279 UPN channel (8 % tapered flanges).
    Upn,
    /// EN 10056-1 equal leg angle.
    Angle,
    /// EN 10219-2 square and rectangular hollow section.
    Rhs,
    /// EN 10219-2 circular hollow section.
    Chs,
    /// AISC W shape.
    W,
    /// AISC C channel (1:6 tapered flanges).
    C,
    /// AISC equal leg angle.
    L,
    /// AISC rectangular HSS (design wall 0.93 × nominal).
    HssRect,
    /// AISC round HSS (design wall 0.93 × nominal).
    HssRound,
    /// ASME B36.10 pipe, standard (STD) and extra strong (XS).
    Pipe,
}

impl Family {
    pub const ALL: [Family; 12] = [
        Family::Ipe,
        Family::Hea,
        Family::Upn,
        Family::Angle,
        Family::Rhs,
        Family::Chs,
        Family::W,
        Family::C,
        Family::L,
        Family::HssRect,
        Family::HssRound,
        Family::Pipe,
    ];

    /// The standard: `iso` or `ansi`.
    pub fn standard(self) -> &'static str {
        match self {
            Family::Ipe | Family::Hea | Family::Upn | Family::Angle | Family::Rhs | Family::Chs => {
                "iso"
            }
            _ => "ansi",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Family::Ipe => "IPE beam",
            Family::Hea => "HEA beam",
            Family::Upn => "UPN channel",
            Family::Angle => "Equal angle",
            Family::Rhs => "Square/rectangular tube",
            Family::Chs => "Round tube",
            Family::W => "W shape",
            Family::C => "C channel",
            Family::L => "Angle",
            Family::HssRect => "Rectangular HSS",
            Family::HssRound => "Round HSS",
            Family::Pipe => "Pipe",
        }
    }
}

/// I and H sections, mm: depth, flange width, web, flange, root radius.
const IPE: &[(&str, f64, f64, f64, f64, f64)] = &[
    ("IPE 80", 80.0, 46.0, 3.8, 5.2, 5.0),
    ("IPE 100", 100.0, 55.0, 4.1, 5.7, 7.0),
    ("IPE 120", 120.0, 64.0, 4.4, 6.3, 7.0),
    ("IPE 140", 140.0, 73.0, 4.7, 6.9, 7.0),
    ("IPE 160", 160.0, 82.0, 5.0, 7.4, 9.0),
    ("IPE 180", 180.0, 91.0, 5.3, 8.0, 9.0),
    ("IPE 200", 200.0, 100.0, 5.6, 8.5, 12.0),
    ("IPE 220", 220.0, 110.0, 5.9, 9.2, 12.0),
    ("IPE 240", 240.0, 120.0, 6.2, 9.8, 15.0),
    ("IPE 270", 270.0, 135.0, 6.6, 10.2, 15.0),
    ("IPE 300", 300.0, 150.0, 7.1, 10.7, 15.0),
];

const HEA: &[(&str, f64, f64, f64, f64, f64)] = &[
    ("HEA 100", 96.0, 100.0, 5.0, 8.0, 12.0),
    ("HEA 120", 114.0, 120.0, 5.0, 8.0, 12.0),
    ("HEA 140", 133.0, 140.0, 5.5, 8.5, 12.0),
    ("HEA 160", 152.0, 160.0, 6.0, 9.0, 15.0),
    ("HEA 180", 171.0, 180.0, 6.0, 9.5, 15.0),
    ("HEA 200", 190.0, 200.0, 6.5, 10.0, 18.0),
    ("HEA 220", 210.0, 220.0, 7.0, 11.0, 18.0),
    ("HEA 240", 230.0, 240.0, 7.5, 12.0, 21.0),
];

/// AISC W, inches: depth, flange width, web, flange, k (design).
const W: &[(&str, f64, f64, f64, f64, f64)] = &[
    ("W6x15", 5.99, 5.99, 0.230, 0.260, 0.510),
    ("W8x31", 8.00, 8.00, 0.285, 0.435, 0.829),
    ("W10x33", 9.73, 7.96, 0.290, 0.435, 0.935),
    ("W12x26", 12.2, 6.49, 0.230, 0.380, 0.680),
    ("W14x22", 13.7, 5.00, 0.230, 0.335, 0.735),
];

/// Channels: depth, flange width, web, flange (at half the flange width),
/// root radius, toe radius, flange slope. UPN in mm; AISC C in inches.
type Channel = (&'static str, f64, f64, f64, f64, f64, f64, f64);
const UPN: &[Channel] = &[
    ("UPN 80", 80.0, 45.0, 6.0, 8.0, 8.0, 4.0, 0.08),
    ("UPN 100", 100.0, 50.0, 6.0, 8.5, 8.5, 4.5, 0.08),
    ("UPN 120", 120.0, 55.0, 7.0, 9.0, 9.0, 4.5, 0.08),
    ("UPN 140", 140.0, 60.0, 7.0, 10.0, 10.0, 5.0, 0.08),
    ("UPN 160", 160.0, 65.0, 7.5, 10.5, 10.5, 5.5, 0.08),
    ("UPN 180", 180.0, 70.0, 8.0, 11.0, 11.0, 5.5, 0.08),
    ("UPN 200", 200.0, 75.0, 8.5, 11.5, 11.5, 6.0, 0.08),
];
const C: &[Channel] = &[
    ("C3x4.1", 3.00, 1.41, 0.170, 0.273, 0.25, 0.0, 1.0 / 6.0),
    ("C4x5.4", 4.00, 1.58, 0.184, 0.296, 0.25, 0.0, 1.0 / 6.0),
    ("C6x8.2", 6.00, 1.92, 0.200, 0.343, 0.3125, 0.0, 1.0 / 6.0),
    ("C6x10.5", 6.00, 2.03, 0.314, 0.343, 0.3125, 0.0, 1.0 / 6.0),
    ("C8x11.5", 8.00, 2.26, 0.220, 0.390, 0.375, 0.0, 1.0 / 6.0),
    ("C10x15.3", 10.0, 2.60, 0.240, 0.436, 0.375, 0.0, 1.0 / 6.0),
];

/// Equal angles: leg, thickness, root radius, toe radius. EN in mm; AISC in
/// inches.
const ANGLE: &[(&str, f64, f64, f64, f64)] = &[
    ("L 20x20x3", 20.0, 3.0, 3.5, 2.0),
    ("L 25x25x3", 25.0, 3.0, 3.5, 2.0),
    ("L 30x30x3", 30.0, 3.0, 5.0, 2.5),
    ("L 40x40x4", 40.0, 4.0, 6.0, 3.0),
    ("L 50x50x5", 50.0, 5.0, 7.0, 3.5),
    ("L 60x60x6", 60.0, 6.0, 8.0, 4.0),
    ("L 70x70x7", 70.0, 7.0, 9.0, 4.5),
    ("L 80x80x8", 80.0, 8.0, 10.0, 5.0),
    ("L 100x100x10", 100.0, 10.0, 12.0, 6.0),
];
const L: &[(&str, f64, f64, f64, f64)] = &[
    ("L2x2x1/4", 2.0, 0.25, 0.25, 0.0),
    ("L3x3x1/4", 3.0, 0.25, 0.3125, 0.0),
    ("L3x3x3/8", 3.0, 0.375, 0.3125, 0.0),
    ("L4x4x3/8", 4.0, 0.375, 0.375, 0.0),
    ("L4x4x1/2", 4.0, 0.5, 0.375, 0.0),
];

/// Rectangular hollow sections, mm or nominal inches: width, height, wall.
const RHS: &[(&str, f64, f64, f64)] = &[
    ("SHS 20x20x2", 20.0, 20.0, 2.0),
    ("SHS 30x30x3", 30.0, 30.0, 3.0),
    ("SHS 40x40x3", 40.0, 40.0, 3.0),
    ("SHS 40x40x4", 40.0, 40.0, 4.0),
    ("SHS 50x50x3", 50.0, 50.0, 3.0),
    ("SHS 50x50x4", 50.0, 50.0, 4.0),
    ("SHS 60x60x4", 60.0, 60.0, 4.0),
    ("SHS 80x80x5", 80.0, 80.0, 5.0),
    ("SHS 100x100x5", 100.0, 100.0, 5.0),
    ("RHS 60x40x3", 60.0, 40.0, 3.0),
    ("RHS 80x40x3", 80.0, 40.0, 3.0),
    ("RHS 100x50x4", 100.0, 50.0, 4.0),
    ("RHS 120x60x5", 120.0, 60.0, 5.0),
];
const HSS_RECT: &[(&str, f64, f64, f64)] = &[
    ("HSS2x2x3/16", 2.0, 2.0, 0.1875),
    ("HSS3x3x1/4", 3.0, 3.0, 0.25),
    ("HSS4x4x1/4", 4.0, 4.0, 0.25),
    ("HSS4x2x1/4", 4.0, 2.0, 0.25),
    ("HSS6x4x1/4", 6.0, 4.0, 0.25),
];

/// Circular hollow sections: outside diameter and wall, mm or inches.
const CHS: &[(&str, f64, f64)] = &[
    ("CHS 21.3x2", 21.3, 2.0),
    ("CHS 26.9x2", 26.9, 2.0),
    ("CHS 33.7x3", 33.7, 3.0),
    ("CHS 42.4x3", 42.4, 3.0),
    ("CHS 48.3x3", 48.3, 3.0),
    ("CHS 60.3x4", 60.3, 4.0),
    ("CHS 76.1x4", 76.1, 4.0),
    ("CHS 88.9x5", 88.9, 5.0),
];
const HSS_ROUND: &[(&str, f64, f64)] = &[
    ("HSS2.375x0.154", 2.375, 0.154),
    ("HSS3.500x0.216", 3.5, 0.216),
    ("HSS4.500x0.237", 4.5, 0.237),
];
const PIPE: &[(&str, f64, f64)] = &[
    ("Pipe 1/2 STD", 0.840, 0.109),
    ("Pipe 3/4 STD", 1.050, 0.113),
    ("Pipe 1 STD", 1.315, 0.133),
    ("Pipe 1-1/4 STD", 1.660, 0.140),
    ("Pipe 1-1/2 STD", 1.900, 0.145),
    ("Pipe 2 STD", 2.375, 0.154),
    ("Pipe 3 STD", 3.500, 0.216),
    ("Pipe 4 STD", 4.500, 0.237),
    ("Pipe 1 XS", 1.315, 0.179),
    ("Pipe 2 XS", 2.375, 0.218),
];

/// The sizes of a family, in table order.
pub fn sizes(family: Family) -> Vec<&'static str> {
    match family {
        Family::Ipe => IPE.iter().map(|r| r.0).collect(),
        Family::Hea => HEA.iter().map(|r| r.0).collect(),
        Family::W => W.iter().map(|r| r.0).collect(),
        Family::Upn => UPN.iter().map(|r| r.0).collect(),
        Family::C => C.iter().map(|r| r.0).collect(),
        Family::Angle => ANGLE.iter().map(|r| r.0).collect(),
        Family::L => L.iter().map(|r| r.0).collect(),
        Family::Rhs => RHS.iter().map(|r| r.0).collect(),
        Family::HssRect => HSS_RECT.iter().map(|r| r.0).collect(),
        Family::Chs => CHS.iter().map(|r| r.0).collect(),
        Family::HssRound => HSS_ROUND.iter().map(|r| r.0).collect(),
        Family::Pipe => PIPE.iter().map(|r| r.0).collect(),
    }
}

/// A section outline: closed loops (the first is the outside), in mm.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub outer: Vec<Curve2>,
    pub holes: Vec<Vec<Curve2>>,
    /// Width and height of the bounding box, centered on the origin.
    pub size: DVec2,
    /// Wall or flange thickness, mm (for end caps and gussets).
    pub wall: f64,
}

/// A corner of a polygon and the fillet radius there (0 for sharp).
type Corner = (DVec2, f64);

/// A closed polygon's curves with tangent fillets, walked counter-clockwise.
/// Concave corners get arcs too; each arc is stored counter-clockwise, so
/// the loop runs through it backward there.
fn filleted(corners: &[Corner]) -> Vec<Curve2> {
    let n = corners.len();
    let area: f64 = (0..n)
        .map(|i| corners[i].0.perp_dot(corners[(i + 1) % n].0))
        .sum();
    let pts: Vec<Corner> = if area < 0.0 {
        corners.iter().rev().copied().collect()
    } else {
        corners.to_vec()
    };
    // Each corner's tangent points (where its two sides now end) and arc.
    let mut ends: Vec<(DVec2, DVec2, Option<Curve2>)> = Vec::with_capacity(n);
    for i in 0..n {
        let (p, r) = pts[i];
        let (prev, next) = (pts[(i + n - 1) % n].0, pts[(i + 1) % n].0);
        if r <= 0.0 {
            ends.push((p, p, None));
            continue;
        }
        let (u, v) = ((prev - p).normalize(), (next - p).normalize());
        let half = u.angle_to(v).abs() / 2.0;
        let d = r / half.tan();
        let (t1, t2) = (p + u * d, p + v * d);
        let center = p + (u + v).normalize() * (r / half.sin());
        // Left turn (convex): counter-clockwise from t1 to t2.
        let convex = (p - prev).perp_dot(next - p) > 0.0;
        let arc = if convex {
            Curve2::Arc {
                center,
                start: t1,
                end: t2,
            }
        } else {
            Curve2::Arc {
                center,
                start: t2,
                end: t1,
            }
        };
        ends.push((t1, t2, Some(arc)));
    }
    let mut out = Vec::new();
    for i in 0..n {
        let (_, t2, arc) = &ends[i];
        if let Some(arc) = arc {
            out.push(arc.clone());
        }
        let next = ends[(i + 1) % n].0;
        if (next - *t2).length() > 1e-9 {
            out.push(Curve2::Line { a: *t2, b: next });
        }
    }
    out
}

fn rect_loop(w: f64, h: f64, r: f64) -> Vec<Curve2> {
    let (x, y) = (w / 2.0, h / 2.0);
    filleted(&[
        (DVec2::new(-x, -y), r),
        (DVec2::new(x, -y), r),
        (DVec2::new(x, y), r),
        (DVec2::new(-x, y), r),
    ])
}

/// An I section: depth, flange width, web, flange, root radius (mm).
fn i_section(h: f64, b: f64, tw: f64, tf: f64, r: f64) -> Section {
    let (x, y, w) = (b / 2.0, h / 2.0, tw / 2.0);
    let p = |x: f64, y: f64, r: f64| (DVec2::new(x, y), r);
    let outer = filleted(&[
        p(-x, -y, 0.0),
        p(x, -y, 0.0),
        p(x, -y + tf, 0.0),
        p(w, -y + tf, r),
        p(w, y - tf, r),
        p(x, y - tf, 0.0),
        p(x, y, 0.0),
        p(-x, y, 0.0),
        p(-x, y - tf, 0.0),
        p(-w, y - tf, r),
        p(-w, -y + tf, r),
        p(-x, -y + tf, 0.0),
    ]);
    Section {
        outer,
        holes: Vec::new(),
        size: DVec2::new(b, h),
        wall: tf,
    }
}

/// A channel with its web on the left: the flanges' inner faces slope by
/// `slope`, with `tf` at half the flange width from the web's back.
fn channel(h: f64, b: f64, tw: f64, tf: f64, r1: f64, r2: f64, slope: f64) -> Section {
    let (x0, y) = (-b / 2.0, h / 2.0);
    // The inner flange face is tf from the outside at half the flange width
    // (measured from the web's back); `mid` is that point's distance from
    // the web's inner face.
    let mid = b / 2.0 - tw;
    let at_web = tf + slope * mid;
    let at_toe = tf - slope * ((b - tw) - mid);
    let p = |x: f64, y: f64, r: f64| (DVec2::new(x, y), r);
    let outer = filleted(&[
        p(x0, -y, 0.0),
        p(x0 + b, -y, 0.0),
        p(x0 + b, -y + at_toe, r2),
        p(x0 + tw, -y + at_web, r1),
        p(x0 + tw, y - at_web, r1),
        p(x0 + b, y - at_toe, r2),
        p(x0 + b, y, 0.0),
        p(x0, y, 0.0),
    ]);
    Section {
        outer,
        holes: Vec::new(),
        size: DVec2::new(b, h),
        wall: tw,
    }
}

/// An equal angle: legs along +x and +y from the heel.
fn angle(a: f64, t: f64, r1: f64, r2: f64) -> Section {
    let o = -a / 2.0;
    let p = |x: f64, y: f64, r: f64| (DVec2::new(o + x, o + y), r);
    let outer = filleted(&[
        p(0.0, 0.0, 0.0),
        p(a, 0.0, 0.0),
        p(a, t, r2),
        p(t, t, r1),
        p(t, a, r2),
        p(0.0, a, 0.0),
    ]);
    Section {
        outer,
        holes: Vec::new(),
        size: DVec2::new(a, a),
        wall: t,
    }
}

/// A hollow rectangle: outer corner radius 2t, inner t.
fn tube(b: f64, h: f64, t: f64) -> Section {
    Section {
        outer: rect_loop(b, h, 2.0 * t),
        holes: vec![rect_loop(b - 2.0 * t, h - 2.0 * t, t)],
        size: DVec2::new(b, h),
        wall: t,
    }
}

fn round(d: f64, t: f64) -> Section {
    Section {
        outer: vec![Curve2::Circle {
            center: DVec2::ZERO,
            radius: d / 2.0,
        }],
        holes: vec![vec![Curve2::Circle {
            center: DVec2::ZERO,
            radius: d / 2.0 - t,
        }]],
        size: DVec2::new(d, d),
        wall: t,
    }
}

/// The section of a standard size.
pub fn section(family: Family, size: &str) -> Result<Section, String> {
    let missing = || format!("{} has no size {size}", family.label());
    Ok(match family {
        Family::Ipe | Family::Hea => {
            let table = if family == Family::Ipe { IPE } else { HEA };
            let r = table.iter().find(|r| r.0 == size).ok_or_else(missing)?;
            i_section(r.1, r.2, r.3, r.4, r.5)
        }
        Family::W => {
            let r = W.iter().find(|r| r.0 == size).ok_or_else(missing)?;
            // The fillet is k (design) less the flange.
            i_section(r.1 * IN, r.2 * IN, r.3 * IN, r.4 * IN, (r.5 - r.4) * IN)
        }
        Family::Upn => {
            let r = UPN.iter().find(|r| r.0 == size).ok_or_else(missing)?;
            channel(r.1, r.2, r.3, r.4, r.5, r.6, r.7)
        }
        Family::C => {
            let r = C.iter().find(|r| r.0 == size).ok_or_else(missing)?;
            channel(
                r.1 * IN,
                r.2 * IN,
                r.3 * IN,
                r.4 * IN,
                r.5 * IN,
                r.6 * IN,
                r.7,
            )
        }
        Family::Angle => {
            let r = ANGLE.iter().find(|r| r.0 == size).ok_or_else(missing)?;
            angle(r.1, r.2, r.3, r.4)
        }
        Family::L => {
            let r = L.iter().find(|r| r.0 == size).ok_or_else(missing)?;
            angle(r.1 * IN, r.2 * IN, r.3 * IN, r.4 * IN)
        }
        Family::Rhs => {
            let r = RHS.iter().find(|r| r.0 == size).ok_or_else(missing)?;
            tube(r.1, r.2, r.3)
        }
        Family::HssRect => {
            let r = HSS_RECT.iter().find(|r| r.0 == size).ok_or_else(missing)?;
            tube(r.1 * IN, r.2 * IN, 0.93 * r.3 * IN)
        }
        Family::Chs => {
            let r = CHS.iter().find(|r| r.0 == size).ok_or_else(missing)?;
            round(r.1, r.2)
        }
        Family::HssRound => {
            let r = HSS_ROUND.iter().find(|r| r.0 == size).ok_or_else(missing)?;
            round(r.1 * IN, 0.93 * r.2 * IN)
        }
        Family::Pipe => {
            let r = PIPE.iter().find(|r| r.0 == size).ok_or_else(missing)?;
            round(r.1 * IN, r.2 * IN)
        }
    })
}

/// Section properties of an outline, mm: area and second moments about the
/// centroid's x and y axes, from the outline itself (Green's theorem over
/// finely sampled loops).
pub fn properties(s: &Section) -> (f64, DVec2, f64, f64) {
    let mut pts_loops: Vec<(Vec<DVec2>, f64)> = vec![(sample(&s.outer), 1.0)];
    pts_loops.extend(s.holes.iter().map(|h| (sample(h), -1.0)));
    let (mut a, mut cx, mut cy, mut ixx, mut iyy) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (pts, sign) in &pts_loops {
        // Oriented so the outside counts positive and holes negative.
        let raw: f64 = (0..pts.len())
            .map(|i| pts[i].perp_dot(pts[(i + 1) % pts.len()]))
            .sum::<f64>()
            / 2.0;
        let flip = if raw * sign < 0.0 { -1.0 } else { 1.0 };
        for i in 0..pts.len() {
            let (p, q) = (pts[i], pts[(i + 1) % pts.len()]);
            let c = p.perp_dot(q) * flip;
            a += c / 2.0;
            cx += (p.x + q.x) * c / 6.0;
            cy += (p.y + q.y) * c / 6.0;
            ixx += (p.y * p.y + p.y * q.y + q.y * q.y) * c / 12.0;
            iyy += (p.x * p.x + p.x * q.x + q.x * q.x) * c / 12.0;
        }
    }
    let c = DVec2::new(cx / a, cy / a);
    (a, c, ixx - a * c.y * c.y, iyy - a * c.x * c.x)
}

/// Points along a section's outside, for extents and previews.
pub fn outline_points(s: &Section) -> Vec<DVec2> {
    sample(&s.outer)
}

/// Points along a loop, arcs every 0.5°.
fn sample(curves: &[Curve2]) -> Vec<DVec2> {
    let mut out: Vec<DVec2> = Vec::new();
    let push = |p: DVec2, out: &mut Vec<DVec2>| {
        if out.last().is_none_or(|q| (*q - p).length() > 1e-12) {
            out.push(p);
        }
    };
    for c in curves {
        match c {
            Curve2::Line { a, b } => {
                // Lines may run either way in the loop: start from the end
                // that joins the last point.
                let (from, to) = match out.last() {
                    Some(q) if (*q - *b).length() < (*q - *a).length() => (*b, *a),
                    _ => (*a, *b),
                };
                push(from, &mut out);
                push(to, &mut out);
            }
            Curve2::Arc { center, start, end } => {
                let r = (*start - *center).length();
                let a0 = (*start - *center).to_angle();
                let mut sweep = ((*end - *center).to_angle() - a0).rem_euclid(2.0 * PI);
                if sweep < 1e-12 {
                    sweep = 2.0 * PI;
                }
                let n = ((sweep / 0.5f64.to_radians()).ceil() as usize).max(1);
                let mut arc: Vec<DVec2> = (0..=n)
                    .map(|k| *center + DVec2::from_angle(a0 + sweep * k as f64 / n as f64) * r)
                    .collect();
                // Walk it from the end nearest the loop so far.
                if let Some(q) = out.last()
                    && (*q - *end).length() < (*q - *start).length()
                {
                    arc.reverse();
                }
                for p in arc {
                    push(p, &mut out);
                }
            }
            Curve2::Circle { center, radius } => {
                let n = 720;
                for k in 0..n {
                    out.push(*center + DVec2::from_angle(2.0 * PI * k as f64 / n as f64) * *radius);
                }
            }
            _ => {}
        }
    }
    if out.len() > 1 && (out[0] - *out.last().unwrap()).length() < 1e-9 {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(family: Family, size: &str, area_cm2: f64, i_cm4: Option<(f64, f64)>, tol: f64) {
        let s = section(family, size).unwrap();
        let (a, _, ixx, iyy) = properties(&s);
        let a = a / 100.0;
        assert!(
            (a - area_cm2).abs() < tol * area_cm2,
            "{size}: area {a} vs {area_cm2}"
        );
        if let Some((ix, iy)) = i_cm4 {
            let (ixx, iyy) = (ixx / 1e4, iyy / 1e4);
            assert!((ixx - ix).abs() < tol * ix, "{size}: Ix {ixx} vs {ix}");
            assert!((iyy - iy).abs() < tol * iy, "{size}: Iy {iyy} vs {iy}");
        }
    }

    #[test]
    fn published_section_properties() {
        // EN 10365 (cm², cm⁴): strong then weak axis.
        check(Family::Ipe, "IPE 100", 10.32, Some((171.0, 15.92)), 5e-3);
        check(Family::Ipe, "IPE 200", 28.48, Some((1943.0, 142.4)), 5e-3);
        check(Family::Hea, "HEA 100", 21.24, Some((349.2, 133.8)), 5e-3);
        check(Family::Hea, "HEA 200", 53.83, Some((3692.0, 1336.0)), 5e-3);
        // EN 10279: area and strong axis.
        let s = section(Family::Upn, "UPN 100").unwrap();
        let (a, _, ixx, _) = properties(&s);
        assert!(
            (a / 100.0 - 13.5).abs() < 0.005 * 13.5,
            "UPN 100 area {}",
            a / 100.0
        );
        assert!(
            (ixx / 1e4 - 206.0).abs() < 0.005 * 206.0,
            "UPN 100 Ix {}",
            ixx / 1e4
        );
        // EN 10056-1: L 50x50x5, 4.80 cm², 11.0 cm⁴ about each leg's axis.
        check(Family::Angle, "L 50x50x5", 4.80, Some((11.0, 11.0)), 5e-3);
        // EN 10219-2 SHS 40x40x3: 4.21 cm².
        check(Family::Rhs, "SHS 40x40x3", 4.21, None, 5e-3);
        // AISC W8x31 (in², in⁴ → cm², cm⁴).
        let (in2, in4) = (6.4516, 41.623);
        check(
            Family::W,
            "W8x31",
            9.13 * in2,
            Some((110.0 * in4, 37.1 * in4)),
            5e-3,
        );
        // AISC HSS4x4x1/4: 3.37 in², 7.80 in⁴; pipe 2 STD: 1.07 in².
        check(
            Family::HssRect,
            "HSS4x4x1/4",
            3.37 * in2,
            Some((7.80 * in4, 7.80 * in4)),
            5e-3,
        );
        check(Family::Pipe, "Pipe 2 STD", 1.07 * in2, None, 5e-3);
    }

    #[test]
    fn every_size_builds_a_face_with_its_own_area() {
        for family in Family::ALL {
            for size in sizes(family) {
                let s = section(family, size).unwrap();
                let face = of_geom::Profile {
                    plane: of_geom::Plane::front(),
                    faces: vec![of_geom::ProfileFace {
                        outer: s.outer.clone(),
                        holes: s.holes.clone(),
                    }],
                }
                .to_shape()
                .unwrap_or_else(|e| panic!("{size}: {}", e.0));
                let solid = face.prism(of_geom::DVec3::Z).unwrap().shape;
                let kernel = solid.mass_props().unwrap().volume;
                let (a, _, _, _) = properties(&s);
                assert!(
                    (kernel - a).abs() < 1e-3 * a,
                    "{size}: kernel {kernel} vs outline {a}"
                );
            }
        }
    }
}

//! Flat patterns drawn as 2D: the outline and cut-outs on `CUT`, bend lines on
//! `BEND_UP` and `BEND_DOWN`, forms on `FORM` and bend notes on
//! `BEND_NOTES`. The same drawing goes into flat DXF files (true size) and
//! flat pattern views on sheets.

use of_model::flat::{FlatCurve, FlatOutline};

use crate::dxf::{DxfSheet, DxfVersion, write_version};
use crate::graphics::{Graphics, HAlign, LineStyle, Owner, P2, Pen, Prim};
use crate::standard::Standard;
use crate::text;

pub const CUT: &str = "CUT";
pub const BEND_UP: &str = "BEND_UP";
pub const BEND_DOWN: &str = "BEND_DOWN";
pub const FORM: &str = "FORM";
pub const BEND_NOTES: &str = "BEND_NOTES";

/// Gap between bodies placed side by side in one file, mm.
const GAP: f64 = 20.0;

/// Where a flat goes: flat millimetres are rotated by `angle`, scaled and
/// moved to `origin`.
#[derive(Debug, Clone, Copy)]
pub struct Place {
    pub origin: P2,
    pub scale: f64,
    pub angle: f64,
}

impl Place {
    pub fn at(&self, p: P2) -> P2 {
        let (s, c) = self.angle.sin_cos();
        let (x, y) = (p[0] * self.scale, p[1] * self.scale);
        [
            self.origin[0] + x * c - y * s,
            self.origin[1] + x * s + y * c,
        ]
    }
}

/// A bend note such as `UP 90° R1`.
pub fn bend_note(up: bool, angle: f64, radius: f64) -> String {
    let num = |v: f64, places: usize| {
        let s = format!("{v:.places$}");
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s
        }
    };
    format!(
        "{} {}° R{}",
        if up { "UP" } else { "DOWN" },
        num(angle.to_degrees(), 1),
        num(radius, 2)
    )
}

/// Draw `flat` into `g`.
pub fn draw(
    g: &mut Graphics,
    flat: &FlatOutline,
    place: Place,
    notes: bool,
    standard: &Standard,
    owner: &Owner,
) {
    let cut = Pen::new(LineStyle::Continuous, 0.35);
    for l in &flat.loops {
        for c in &l.curves {
            let prim = match c {
                FlatCurve::Line { a, b } => Prim::Line {
                    a: place.at(*a),
                    b: place.at(*b),
                },
                FlatCurve::Arc {
                    center,
                    radius,
                    start,
                    end,
                } => Prim::Arc {
                    center: place.at(*center),
                    radius: radius * place.scale,
                    start: start + place.angle,
                    end: end + place.angle,
                },
                FlatCurve::Polyline { points } => Prim::Polyline {
                    points: points.iter().map(|p| place.at(*p)).collect(),
                    closed: false,
                },
            };
            g.push(prim, cut, CUT, owner);
        }
    }
    for b in &flat.bend_lines {
        let (layer, color) = if b.up {
            (BEND_UP, [0, 128, 0])
        } else {
            (BEND_DOWN, [0, 0, 255])
        };
        let pen = Pen {
            color,
            ..Pen::new(LineStyle::Center, 0.18)
        };
        let (a, z) = (place.at(b.a), place.at(b.b));
        g.line(a, z, pen, layer, owner);
        if notes {
            let h = standard.text_height();
            let dir = (z[1] - a[1]).atan2(z[0] - a[0]);
            let angle = text::readable(dir);
            let mid = [(a[0] + z[0]) / 2.0, (a[1] + z[1]) / 2.0];
            // Just beside the line, on its left as read.
            let at = [
                mid[0] - angle.sin() * h * 0.5,
                mid[1] + angle.cos() * h * 0.5,
            ];
            for prim in text::layout(
                &bend_note(b.up, b.angle, b.radius),
                at,
                h,
                angle,
                HAlign::Center,
            ) {
                g.push(
                    prim,
                    Pen::new(LineStyle::Continuous, 0.25),
                    BEND_NOTES,
                    owner,
                );
            }
        }
    }
    let form = Pen {
        color: [255, 0, 255],
        ..Pen::new(LineStyle::Continuous, 0.25)
    };
    for m in &flat.marks {
        if m.outline.len() > 1 {
            g.push(
                Prim::Polyline {
                    points: m.outline.iter().map(|p| place.at(*p)).collect(),
                    closed: true,
                },
                form,
                FORM,
                owner,
            );
        }
        // Center mark.
        let c = place.at(m.center);
        let s = 1.5;
        g.line([c[0] - s, c[1]], [c[0] + s, c[1]], form, FORM, owner);
        g.line([c[0], c[1] - s], [c[0], c[1] + s], form, FORM, owner);
    }
}

/// Flats at true size, side by side from the origin, each with its lower
/// left corner on the X axis.
pub fn graphics(flats: &[FlatOutline], notes: bool, standard: &Standard) -> Graphics {
    let owner = Owner::View {
        view: 0,
        entity: None,
    };
    let mut g = Graphics::new(0.0, 0.0);
    let mut x = 0.0;
    for flat in flats {
        let Some((lo, hi)) = flat.extents() else {
            continue;
        };
        let place = Place {
            origin: [x - lo[0], -lo[1]],
            scale: 1.0,
            angle: 0.0,
        };
        draw(&mut g, flat, place, notes, standard, &owner);
        x += hi[0] - lo[0] + GAP;
        g.height = g.height.max(hi[1] - lo[1]);
    }
    g.width = (x - GAP).max(0.0);
    g
}

/// A DXF file of flats at true size in millimetres.
pub fn dxf(
    flats: &[FlatOutline],
    notes: bool,
    standard: &Standard,
    version: DxfVersion,
) -> Result<String, String> {
    if flats.is_empty() {
        return Err("there's no flat pattern to write".into());
    }
    let g = graphics(flats, notes, standard);
    write_version(
        &[DxfSheet {
            graphics: &g,
            name: "Flat".into(),
            scale: 1.0,
            view_annotations: Vec::new(),
        }],
        standard,
        version,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bend_notes_read_naturally() {
        assert_eq!(
            bend_note(true, std::f64::consts::FRAC_PI_2, 1.0),
            "UP 90° R1"
        );
        assert_eq!(
            bend_note(false, std::f64::consts::FRAC_PI_4, 1.25),
            "DOWN 45° R1.25"
        );
        assert_eq!(
            bend_note(true, 2.0f64.to_radians() + 120f64.to_radians(), 0.5),
            "UP 122° R0.5"
        );
    }
}

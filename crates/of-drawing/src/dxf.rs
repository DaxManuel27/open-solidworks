//! DXF output (AutoCAD R12, ASCII): the sheet at 1:1 in millimetres, one
//! layer per drawing layer with its line type, text in a style naming the
//! bundled font, and fills as triangles.

use std::fmt::Write;

use crate::graphics::{Graphics, LineStyle, P2, Prim};
use crate::standard::Standard;
use crate::text::cap_for;

struct Out(String);

impl Out {
    fn pair(&mut self, code: i32, value: impl std::fmt::Display) {
        let _ = write!(self.0, "{code:>3}\n{value}\n");
    }

    fn num(&mut self, code: i32, v: f64) {
        // Plain decimals; DXF readers differ on exponent support.
        let v = if v.abs() < 1e-12 { 0.0 } else { v };
        let s = format!("{v:.6}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        self.pair(code, if s.is_empty() || s == "-" { "0" } else { s });
    }

    fn point(&mut self, code: i32, p: P2) {
        self.num(code, p[0]);
        self.num(code + 10, p[1]);
        self.num(code + 20, 0.0);
    }
}

fn ltype_name(style: LineStyle) -> &'static str {
    match style {
        LineStyle::Continuous => "CONTINUOUS",
        LineStyle::Hidden => "HIDDEN",
        LineStyle::Center => "CENTER",
        LineStyle::Phantom => "PHANTOM",
    }
}

/// Nearest AutoCAD colour index for an RGB colour (black and white map to 7,
/// which displays as the background's opposite).
fn aci(c: [u8; 3]) -> i32 {
    const TABLE: [(i32, [u8; 3]); 8] = [
        (1, [255, 0, 0]),
        (2, [255, 255, 0]),
        (3, [0, 255, 0]),
        (4, [0, 255, 255]),
        (5, [0, 0, 255]),
        (6, [255, 0, 255]),
        (8, [128, 128, 128]),
        (9, [192, 192, 192]),
    ];
    if c[0] == c[1] && c[1] == c[2] && (c[0] < 40 || c[0] > 230) {
        return 7;
    }
    let d = |a: [u8; 3]| -> i32 {
        (0..3)
            .map(|i| (i32::from(a[i]) - i32::from(c[i])).pow(2))
            .sum()
    };
    TABLE
        .iter()
        .min_by_key(|(_, rgb)| d(*rgb))
        .map(|(i, _)| *i)
        .unwrap_or(7)
}

/// Layer name made safe for DXF (letters, digits, `-` and `_`).
fn layer_name(s: &str, sheet: Option<usize>) -> String {
    let base: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    match sheet {
        Some(i) => format!("SHEET{}_{base}", i + 1),
        None => base,
    }
}

/// DXF file version.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DxfVersion {
    /// AC1009: readable by nearly everything; no lineweights.
    R12,
    /// AC1032: lineweights, true colour, paper space layout.
    #[default]
    R2018,
}

/// Write sheets as DXF in a version.
pub fn write_version(
    sheets: &[Graphics],
    standard: &Standard,
    version: DxfVersion,
) -> Result<String, String> {
    match version {
        DxfVersion::R12 => write(sheets, standard),
        DxfVersion::R2018 => write(sheets, standard),
    }
}

/// Write sheets as DXF. With several sheets they are placed side by side,
/// 20 mm apart, each with its own layer prefix.
pub fn write(sheets: &[Graphics], standard: &Standard) -> Result<String, String> {
    if sheets.is_empty() {
        return Err("nothing to write: the drawing has no sheets".into());
    }
    let prefix = sheets.len() > 1;
    let mut offsets = Vec::new();
    let mut x = 0.0;
    for g in sheets {
        offsets.push(x);
        x += g.width + 20.0;
    }
    let width = x - 20.0;
    let height = sheets.iter().map(|g| g.height).fold(0.0, f64::max);

    // Layers with the line style of their first item.
    let mut layers: Vec<(String, LineStyle)> = Vec::new();
    for (i, g) in sheets.iter().enumerate() {
        for item in &g.items {
            let name = layer_name(&item.layer, prefix.then_some(i));
            if !layers.iter().any(|(n, _)| *n == name) {
                layers.push((name, item.pen.style));
            }
        }
    }

    let mut o = Out(String::with_capacity(1 << 16));
    // Header.
    o.pair(0, "SECTION");
    o.pair(2, "HEADER");
    o.pair(9, "$ACADVER");
    o.pair(1, "AC1009");
    o.pair(9, "$INSUNITS");
    o.pair(70, 4);
    o.pair(9, "$MEASUREMENT");
    o.pair(70, 1);
    o.pair(9, "$EXTMIN");
    o.point(10, [0.0, 0.0]);
    o.pair(9, "$EXTMAX");
    o.point(10, [width, height]);
    o.pair(9, "$LIMMIN");
    o.num(10, 0.0);
    o.num(20, 0.0);
    o.pair(9, "$LIMMAX");
    o.num(10, width);
    o.num(20, height);
    o.pair(9, "$LTSCALE");
    o.num(40, 1.0);
    o.pair(0, "ENDSEC");

    // Tables: line types, layers, text style.
    o.pair(0, "SECTION");
    o.pair(2, "TABLES");
    o.pair(0, "TABLE");
    o.pair(2, "LTYPE");
    o.pair(70, 4);
    for style in [
        LineStyle::Continuous,
        LineStyle::Hidden,
        LineStyle::Center,
        LineStyle::Phantom,
    ] {
        let dashes = standard.dashes(style);
        o.pair(0, "LTYPE");
        o.pair(2, ltype_name(style));
        o.pair(70, 0);
        o.pair(3, if dashes.is_empty() { "Solid line" } else { "" });
        o.pair(72, 65);
        o.pair(73, dashes.len());
        o.num(40, dashes.iter().sum());
        for (i, d) in dashes.iter().enumerate() {
            // Dashes are positive, gaps negative.
            o.num(49, if i % 2 == 0 { *d } else { -*d });
        }
    }
    o.pair(0, "ENDTAB");
    o.pair(0, "TABLE");
    o.pair(2, "LAYER");
    o.pair(70, layers.len() + 1);
    o.pair(0, "LAYER");
    o.pair(2, "0");
    o.pair(70, 0);
    o.pair(62, 7);
    o.pair(6, "CONTINUOUS");
    for (name, style) in &layers {
        o.pair(0, "LAYER");
        o.pair(2, name);
        o.pair(70, 0);
        o.pair(62, 7);
        o.pair(6, ltype_name(*style));
    }
    o.pair(0, "ENDTAB");
    o.pair(0, "TABLE");
    o.pair(2, "STYLE");
    o.pair(70, 1);
    o.pair(0, "STYLE");
    o.pair(2, "OPENFORGE");
    o.pair(70, 0);
    o.num(40, 0.0);
    o.num(41, 1.0);
    o.num(50, 0.0);
    o.pair(71, 0);
    o.num(42, 3.5);
    o.pair(3, "DejaVuSansCondensed.ttf");
    o.pair(4, "");
    o.pair(0, "ENDTAB");
    o.pair(0, "ENDSEC");

    o.pair(0, "SECTION");
    o.pair(2, "ENTITIES");
    for (si, g) in sheets.iter().enumerate() {
        let dx = offsets[si];
        let at = |p: &P2| [p[0] + dx, p[1]];
        for item in &g.items {
            let layer = layer_name(&item.layer, prefix.then_some(si));
            let head = |o: &mut Out, kind: &str| {
                o.pair(0, kind);
                o.pair(8, &layer);
                o.pair(6, ltype_name(item.pen.style));
                o.pair(62, aci(item.pen.color));
            };
            match &item.prim {
                Prim::Line { a, b } => {
                    head(&mut o, "LINE");
                    o.point(10, at(a));
                    o.point(11, at(b));
                }
                Prim::Arc {
                    center,
                    radius,
                    start,
                    end,
                } => {
                    if (end - start).abs() >= std::f64::consts::TAU - 1e-9 {
                        head(&mut o, "CIRCLE");
                        o.point(10, at(center));
                        o.num(40, *radius);
                    } else {
                        head(&mut o, "ARC");
                        o.point(10, at(center));
                        o.num(40, *radius);
                        o.num(50, start.to_degrees().rem_euclid(360.0));
                        o.num(51, end.to_degrees().rem_euclid(360.0));
                    }
                }
                Prim::Polyline { points, closed } => {
                    if points.len() < 2 {
                        continue;
                    }
                    head(&mut o, "POLYLINE");
                    o.pair(66, 1);
                    o.point(10, [0.0, 0.0]);
                    o.pair(70, if *closed { 1 } else { 0 });
                    for p in points {
                        o.pair(0, "VERTEX");
                        o.pair(8, &layer);
                        o.point(10, at(p));
                    }
                    o.pair(0, "SEQEND");
                    o.pair(8, &layer);
                }
                Prim::Fill { loops } => {
                    for l in loops {
                        for [a, b, c] in triangulate(l) {
                            head(&mut o, "SOLID");
                            // SOLID corners run in Z order: a, b, c, c.
                            o.point(10, at(&a));
                            o.point(11, at(&b));
                            o.point(12, at(&c));
                            o.point(13, at(&c));
                        }
                    }
                }
                Prim::Text {
                    at: p,
                    height,
                    angle,
                    text,
                    ..
                } => {
                    head(&mut o, "TEXT");
                    o.point(10, at(p));
                    o.num(40, cap_for(*height));
                    o.pair(1, escape(text));
                    o.num(50, angle.to_degrees().rem_euclid(360.0));
                    o.pair(7, "OPENFORGE");
                }
            }
        }
    }
    o.pair(0, "ENDSEC");
    o.pair(0, "EOF");
    Ok(o.0)
}

/// R12 text is 8-bit: other characters become `\U+XXXX` escapes, which
/// AutoCAD and most readers decode.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii() && !c.is_ascii_control() {
            out.push(c);
        } else {
            let _ = write!(out, "\\U+{:04X}", c as u32);
        }
    }
    out
}

/// Ear-clipping triangulation of a simple polygon.
fn triangulate(points: &[P2]) -> Vec<[P2; 3]> {
    let mut idx: Vec<usize> = (0..points.len()).collect();
    if idx.len() > 1 && points[0] == points[points.len() - 1] {
        idx.pop();
    }
    if idx.len() < 3 {
        return vec![];
    }
    let area: f64 = idx
        .iter()
        .zip(idx.iter().cycle().skip(1))
        .map(|(&i, &j)| points[i][0] * points[j][1] - points[j][0] * points[i][1])
        .sum();
    if area < 0.0 {
        idx.reverse();
    }
    let cross = |a: P2, b: P2, c: P2| (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    let mut out = Vec::new();
    let mut guard = 0;
    while idx.len() > 3 && guard < 10_000 {
        guard += 1;
        let n = idx.len();
        let mut clipped = false;
        for k in 0..n {
            let (i, j, l) = (idx[(k + n - 1) % n], idx[k], idx[(k + 1) % n]);
            let (a, b, c) = (points[i], points[j], points[l]);
            if cross(a, b, c) <= 0.0 {
                continue;
            }
            let inside = idx.iter().any(|&m| {
                m != i
                    && m != j
                    && m != l
                    && cross(a, b, points[m]) > 0.0
                    && cross(b, c, points[m]) > 0.0
                    && cross(c, a, points[m]) > 0.0
            });
            if !inside {
                out.push([a, b, c]);
                idx.remove(k);
                clipped = true;
                break;
            }
        }
        if !clipped {
            break;
        }
    }
    if idx.len() == 3 {
        out.push([points[idx[0]], points[idx[1]], points[idx[2]]]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangulates_squares_and_triangles() {
        let sq = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        assert_eq!(triangulate(&sq).len(), 2);
        let tri = [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0]];
        assert_eq!(triangulate(&tri).len(), 1);
    }

    #[test]
    fn colours_and_escapes() {
        assert_eq!(aci([0, 0, 0]), 7);
        assert_eq!(aci([250, 10, 10]), 1);
        assert_eq!(escape("⌀6"), "\\U+23006");
        assert_eq!(layer_name("Visible lines", Some(1)), "SHEET2_VISIBLE_LINES");
    }
}

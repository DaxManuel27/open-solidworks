//! Sheet formats: border, zones, title block and fields.
//!
//! A [`Format`] is data (lines and texts in sheet millimetres), so built-in
//! formats and `.ofdt` template files work the same way. Text may hold
//! fields, replaced when the sheet is drawn:
//!
//! | Field | Value |
//! | --- | --- |
//! | `$PRP:"Name"` | custom property of the model in the sheet's properties view |
//! | `$PRPSHEET:"Name"` | drawing property |
//! | `$SHEET`, `$SHEETS` | sheet number and count |
//! | `$SCALE`, `$SIZE` | sheet scale and paper size |
//! | `$DATE` | the drawing's `Date` property, else today |
//! | `$FILENAME` | drawing file name without extension |
//! | `$MATERIAL`, `$MASS` | material and mass of the part in the properties view |

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::document::{Projection, Sheet};
use crate::graphics::{Graphics, HAlign, LineStyle, Owner, P2, Pen, Prim, layers};
use crate::standard::{Standard, StandardBase};
use crate::text;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum FormatItem {
    Line {
        a: P2,
        b: P2,
        #[serde(default)]
        thick: bool,
    },
    Text {
        at: P2,
        /// Capital height, mm.
        height: f64,
        text: String,
        #[serde(default)]
        align: HAlign,
        /// Shrink the text to fit this width, mm.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fit: Option<f64>,
    },
    /// The first- or third-angle projection symbol of the sheet, `size` mm
    /// high, centered at `at`.
    ProjectionSymbol { at: P2, size: f64 },
}

/// A sheet format for one paper size.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Format {
    pub items: Vec<FormatItem>,
    /// Inner border: views should stay inside it.
    pub frame: [P2; 2],
    /// Title block rectangle, kept free of views.
    pub title_block: [P2; 2],
}

pub const FORMAT_FILE: &str = "openforge-sheet-format";

/// Values for fields.
#[derive(Debug, Clone, Default)]
pub struct Fields {
    pub model: BTreeMap<String, String>,
    pub drawing: BTreeMap<String, String>,
    pub sheet: usize,
    pub sheets: usize,
    pub scale: String,
    pub size: String,
    pub file_name: String,
    pub material: String,
    pub mass: String,
}

/// Replace fields in `s`. Unknown properties become empty.
pub fn fill(s: &str, f: &Fields) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('$') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let prop = |prefix: &str| -> Option<(String, usize)> {
            let t = tail.strip_prefix(prefix)?.strip_prefix(":\"")?;
            let end = t.find('"')?;
            Some((t[..end].to_string(), prefix.len() + 2 + end + 1))
        };
        if let Some((name, n)) = prop("$PRPSHEET") {
            out.push_str(f.drawing.get(&name).map(String::as_str).unwrap_or(""));
            rest = &tail[n..];
            continue;
        }
        if let Some((name, n)) = prop("$PRP") {
            out.push_str(f.model.get(&name).map(String::as_str).unwrap_or(""));
            rest = &tail[n..];
            continue;
        }
        let simple: [(&str, String); 9] = [
            ("$SHEETS", f.sheets.to_string()),
            ("$SHEET", f.sheet.to_string()),
            ("$SCALE", f.scale.clone()),
            ("$SIZE", f.size.clone()),
            (
                "$DATE",
                f.drawing.get("Date").cloned().unwrap_or_else(today),
            ),
            ("$FILENAME", f.file_name.clone()),
            ("$MATERIAL", f.material.clone()),
            ("$MASS", f.mass.clone()),
            ("$$", "$".into()),
        ];
        if let Some((k, v)) = simple.iter().find(|(k, _)| tail.starts_with(k)) {
            out.push_str(v);
            rest = &tail[k.len()..];
        } else {
            out.push('$');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

/// Today's date as `YYYY-MM-DD` (UTC).
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = civil(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// Days since 1970-01-01 to a calendar date (proleptic Gregorian).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// Zone labels: letters skip I and O.
fn zone_letter(i: usize) -> String {
    const L: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";
    (L[i % L.len()] as char).to_string()
}

/// The built-in format for a sheet: `iso` (ISO 5457 border with zones and
/// an ISO 7200 title block) or `ansi` (ASME Y14.1).
pub fn builtin(name: &str, sheet: &Sheet) -> Option<Format> {
    let (w, h) = sheet.paper();
    match name {
        "iso" => Some(iso(w, h)),
        "ansi" => Some(ansi(w, h)),
        "none" => Some(Format {
            items: Vec::new(),
            frame: [[0.0, 0.0], [w, h]],
            title_block: [[w, 0.0], [w, 0.0]],
        }),
        _ => None,
    }
}

struct Builder {
    items: Vec<FormatItem>,
}

impl Builder {
    fn line(&mut self, a: P2, b: P2, thick: bool) {
        self.items.push(FormatItem::Line { a, b, thick });
    }

    fn rect(&mut self, a: P2, b: P2, thick: bool) {
        self.line(a, [b[0], a[1]], thick);
        self.line([b[0], a[1]], b, thick);
        self.line(b, [a[0], b[1]], thick);
        self.line([a[0], b[1]], a, thick);
    }

    fn text(&mut self, at: P2, height: f64, text: &str, align: HAlign, fit: Option<f64>) {
        self.items.push(FormatItem::Text {
            at,
            height,
            text: text.into(),
            align,
            fit,
        });
    }

    /// A title block cell: a caption in its top-left corner and a value.
    fn cell(&mut self, a: P2, b: P2, caption: &str, value: &str, height: f64) {
        self.rect(a, b, false);
        self.text([a[0] + 1.0, b[1] - 2.8], 1.8, caption, HAlign::Left, None);
        let w = b[0] - a[0] - 2.0;
        self.text(
            [a[0] + 1.5, a[1] + 1.8],
            height.min(b[1] - a[1] - 5.0).max(1.8),
            value,
            HAlign::Left,
            Some(w),
        );
    }

    /// Zone ticks and labels between the frame `f` and the paper edge.
    fn zones(&mut self, f: [P2; 2], w: f64, h: f64, field: f64, from_top: bool) {
        let (fw, fh) = (f[1][0] - f[0][0], f[1][1] - f[0][1]);
        let nx = ((fw / field).round() as usize).max(2);
        let ny = ((fh / field).round() as usize).max(2);
        let mid_b = f[0][1] / 2.0;
        let mid_t = (h + f[1][1]) / 2.0;
        let mid_l = f[0][0] / 2.0;
        let mid_r = (w + f[1][0]) / 2.0;
        for i in 0..=nx {
            let x = f[0][0] + fw * i as f64 / nx as f64;
            if i > 0 && i < nx {
                self.line([x, f[0][1]], [x, f[0][1] - 5.0], false);
                self.line([x, f[1][1]], [x, f[1][1] + 5.0], false);
            }
            if i < nx {
                let cx = x + fw / nx as f64 / 2.0;
                let n = (i + 1).to_string();
                self.text([cx, mid_b - 1.75], 3.5, &n, HAlign::Center, None);
                self.text([cx, mid_t - 1.75], 3.5, &n, HAlign::Center, None);
            }
        }
        for j in 0..=ny {
            let y = f[0][1] + fh * j as f64 / ny as f64;
            if j > 0 && j < ny {
                self.line([f[0][0], y], [f[0][0] - 5.0, y], false);
                self.line([f[1][0], y], [f[1][0] + 5.0, y], false);
            }
            if j < ny {
                let cy = y + fh / ny as f64 / 2.0 - 1.75;
                let k = if from_top { ny - 1 - j } else { j };
                let l = zone_letter(k);
                self.text([mid_l, cy], 3.5, &l, HAlign::Center, None);
                self.text([mid_r, cy], 3.5, &l, HAlign::Center, None);
            }
        }
    }
}

fn iso(w: f64, h: f64) -> Format {
    let mut b = Builder { items: Vec::new() };
    // ISO 5457: 20 mm filing margin on the left, 10 mm elsewhere.
    let f = [[20.0, 10.0], [w - 10.0, h - 10.0]];
    b.rect(f[0], f[1], true);
    // Centering marks.
    b.line([0.0, h / 2.0], [f[0][0] + 5.0, h / 2.0], true);
    b.line([w, h / 2.0], [f[1][0] - 5.0, h / 2.0], true);
    b.line(
        [(f[0][0] + f[1][0]) / 2.0, 0.0],
        [(f[0][0] + f[1][0]) / 2.0, f[0][1] + 5.0],
        true,
    );
    b.line(
        [(f[0][0] + f[1][0]) / 2.0, h],
        [(f[0][0] + f[1][0]) / 2.0, f[1][1] - 5.0],
        true,
    );
    b.zones(f, w, h, 50.0, true);

    // ISO 7200 title block, 180 mm wide, in the bottom-right corner.
    let tw = 180.0_f64.min(f[1][0] - f[0][0]);
    let x0 = f[1][0] - tw;
    let y0 = f[0][1];
    let col = |frac: f64| x0 + tw * frac;
    let rows = [y0, y0 + 8.0, y0 + 16.0, y0 + 28.0, y0 + 40.0];
    let tb = [[x0, y0], [f[1][0], rows[4]]];
    b.cell(
        [x0, rows[0]],
        [col(0.25), rows[1]],
        "DRAWN BY",
        "$PRPSHEET:\"DrawnBy\"",
        3.5,
    );
    b.cell(
        [col(0.25), rows[0]],
        [col(0.5), rows[1]],
        "CHECKED BY",
        "$PRPSHEET:\"CheckedBy\"",
        3.5,
    );
    b.cell(
        [col(0.5), rows[0]],
        [col(0.7), rows[1]],
        "DATE",
        "$DATE",
        3.5,
    );
    b.cell(
        [col(0.7), rows[0]],
        [col(0.82), rows[1]],
        "SIZE",
        "$SIZE",
        3.5,
    );
    b.cell(
        [col(0.82), rows[0]],
        [col(1.0), rows[1]],
        "SHEET",
        "$SHEET / $SHEETS",
        3.5,
    );
    b.cell(
        [x0, rows[1]],
        [col(0.5), rows[2]],
        "MATERIAL",
        "$MATERIAL",
        3.5,
    );
    b.cell(
        [col(0.5), rows[1]],
        [col(0.7), rows[2]],
        "MASS",
        "$MASS",
        3.5,
    );
    b.cell(
        [col(0.7), rows[1]],
        [col(1.0), rows[2]],
        "SCALE",
        "$SCALE",
        3.5,
    );
    b.cell(
        [x0, rows[2]],
        [col(0.7), rows[3]],
        "DRAWING NUMBER",
        "$PRP:\"PartNo\"",
        5.0,
    );
    b.cell(
        [col(0.7), rows[2]],
        [col(0.82), rows[3]],
        "REV",
        "$PRPSHEET:\"Revision\"",
        5.0,
    );
    b.cell(
        [col(0.82), rows[2]],
        [col(1.0), rows[4]],
        "PROJECTION",
        "",
        3.5,
    );
    b.items.push(FormatItem::ProjectionSymbol {
        at: [
            (col(0.82) + col(1.0)) / 2.0,
            (rows[2] + rows[4]) / 2.0 - 2.0,
        ],
        size: 10.0,
    });
    b.cell(
        [x0, rows[3]],
        [col(0.5), rows[4]],
        "TITLE",
        "$PRP:\"Description\"",
        5.0,
    );
    b.cell(
        [col(0.5), rows[3]],
        [col(0.82), rows[4]],
        "OWNER",
        "$PRPSHEET:\"Company\"",
        3.5,
    );
    b.rect(tb[0], tb[1], true);
    Format {
        items: b.items,
        frame: f,
        title_block: tb,
    }
}

fn ansi(w: f64, h: f64) -> Format {
    let mut b = Builder { items: Vec::new() };
    // ASME Y14.1: 0.5 in margins, zones numbered right to left and lettered
    // bottom to top.
    let m = 12.7;
    let f = [[m, m], [w - m, h - m]];
    b.rect(f[0], f[1], true);
    b.zones(f, w, h, 101.6, false);
    // Zone numbers run right to left: relabel the ones just written.
    let nx = (((f[1][0] - f[0][0]) / 101.6).round() as usize).max(2);
    for item in &mut b.items {
        if let FormatItem::Text { text, at, .. } = item
            && let Ok(n) = text.parse::<usize>()
            && (at[1] < f[0][1] || at[1] > f[1][1])
        {
            *text = (nx + 1 - n).to_string();
        }
    }

    let tw = 177.8_f64.min(f[1][0] - f[0][0]);
    let x0 = f[1][0] - tw;
    let y0 = f[0][1];
    let col = |frac: f64| x0 + tw * frac;
    let rows = [y0, y0 + 8.0, y0 + 17.0, y0 + 28.0, y0 + 40.0];
    let tb = [[x0, y0], [f[1][0], rows[4]]];
    b.cell([x0, rows[0]], [col(0.3), rows[1]], "SCALE", "$SCALE", 3.0);
    b.cell(
        [col(0.3), rows[0]],
        [col(0.55), rows[1]],
        "WEIGHT",
        "$MASS",
        3.0,
    );
    b.cell(
        [col(0.55), rows[0]],
        [col(0.8), rows[1]],
        "MATERIAL",
        "$MATERIAL",
        3.0,
    );
    b.cell(
        [col(0.8), rows[0]],
        [col(1.0), rows[1]],
        "SHEET",
        "$SHEET OF $SHEETS",
        3.0,
    );
    b.cell([x0, rows[1]], [col(0.12), rows[2]], "SIZE", "$SIZE", 4.0);
    b.cell(
        [col(0.12), rows[1]],
        [col(0.85), rows[2]],
        "DWG. NO.",
        "$PRP:\"PartNo\"",
        4.5,
    );
    b.cell(
        [col(0.85), rows[1]],
        [col(1.0), rows[2]],
        "REV",
        "$PRPSHEET:\"Revision\"",
        4.5,
    );
    b.cell(
        [x0, rows[2]],
        [col(1.0), rows[3]],
        "TITLE",
        "$PRP:\"Description\"",
        5.0,
    );
    b.cell(
        [x0, rows[3]],
        [col(0.35), rows[4]],
        "DRAWN",
        "$PRPSHEET:\"DrawnBy\"",
        3.0,
    );
    b.cell(
        [col(0.35), rows[3]],
        [col(0.55), rows[4]],
        "DATE",
        "$DATE",
        3.0,
    );
    b.cell(
        [col(0.55), rows[3]],
        [col(0.8), rows[4]],
        "COMPANY",
        "$PRPSHEET:\"Company\"",
        3.0,
    );
    b.cell([col(0.8), rows[3]], [col(1.0), rows[4]], "", "", 3.0);
    b.items.push(FormatItem::ProjectionSymbol {
        at: [(col(0.8) + col(1.0)) / 2.0, (rows[3] + rows[4]) / 2.0],
        size: 8.0,
    });
    // Tolerance block to the left of the title block.
    let tx = x0 - 70.0;
    if tx > f[0][0] {
        b.rect([tx, y0], [x0, y0 + 28.0], false);
        b.text(
            [tx + 1.5, y0 + 24.0],
            2.0,
            "UNLESS OTHERWISE SPECIFIED:",
            HAlign::Left,
            Some(67.0),
        );
        b.text(
            [tx + 1.5, y0 + 19.0],
            2.0,
            "DIMENSIONS ARE IN MILLIMETERS",
            HAlign::Left,
            Some(67.0),
        );
        b.text(
            [tx + 1.5, y0 + 14.0],
            2.0,
            "$PRPSHEET:\"Tolerance\"",
            HAlign::Left,
            Some(67.0),
        );
    }
    b.rect(tb[0], tb[1], true);
    Format {
        items: b.items,
        frame: f,
        title_block: tb,
    }
}

/// Draw a format onto a sheet.
pub fn draw(
    format: &Format,
    g: &mut Graphics,
    fields: &Fields,
    projection: Projection,
    standard: &Standard,
) {
    let owner = Owner::Format;
    let thick = Pen::new(LineStyle::Continuous, standard.thick_line() * 1.4);
    let thin = Pen::new(LineStyle::Continuous, standard.thin_line());
    for item in &format.items {
        match item {
            FormatItem::Line { a, b, thick: t } => g.line(
                *a,
                *b,
                if *t { thick } else { thin },
                layers::FORMAT,
                &owner,
            ),
            FormatItem::Text {
                at,
                height,
                text: t,
                align,
                fit,
            } => {
                let value = fill(t, fields);
                if value.trim().is_empty() {
                    continue;
                }
                let mut h = *height;
                if let Some(width) = fit {
                    let w = text::line_width(&value, h);
                    if w > *width && w > 0.0 {
                        h *= width / w;
                    }
                }
                for p in text::layout(&value, *at, h, 0.0, *align) {
                    g.push(p, thin, layers::FORMAT, &owner);
                }
            }
            FormatItem::ProjectionSymbol { at, size } => {
                projection_symbol(g, *at, *size, projection, thin)
            }
        }
    }
}

/// ISO 5456-2 projection symbol: a truncated cone's side view and end view;
/// the end view is on the left in first angle and on the right in third.
fn projection_symbol(g: &mut Graphics, at: P2, size: f64, projection: Projection, pen: Pen) {
    let owner = Owner::Format;
    let (h, d) = (size / 2.0, size / 4.0);
    let (end_x, side_x) = match projection {
        Projection::First => (at[0] - size * 0.6, at[0] + size * 0.45),
        Projection::Third => (at[0] + size * 0.6, at[0] - size * 0.45),
    };
    for r in [h, d] {
        g.push(
            Prim::Arc {
                center: [end_x, at[1]],
                radius: r,
                start: 0.0,
                end: std::f64::consts::TAU,
            },
            pen,
            layers::FORMAT,
            &owner,
        );
    }
    // The cone narrows toward the end view.
    let (wide, narrow) = match projection {
        Projection::First => (side_x + size * 0.5, side_x - size * 0.5),
        Projection::Third => (side_x - size * 0.5, side_x + size * 0.5),
    };
    g.push(
        Prim::Polyline {
            points: vec![
                [wide, at[1] - h],
                [wide, at[1] + h],
                [narrow, at[1] + d],
                [narrow, at[1] - d],
            ],
            closed: true,
        },
        pen,
        layers::FORMAT,
        &owner,
    );
    let center = Pen::new(LineStyle::Center, pen.weight);
    g.line(
        [end_x - h - 1.5, at[1]],
        [wide.max(narrow) + 1.5, at[1]],
        center,
        layers::FORMAT,
        &owner,
    );
}

/// Default sheet options for a standard.
pub fn template_for(standard: &Standard) -> &'static str {
    match standard.base {
        StandardBase::Iso => "iso",
        StandardBase::Ansi => "ansi",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_are_replaced() {
        let mut f = Fields {
            sheet: 2,
            sheets: 3,
            scale: "1:2".into(),
            ..Default::default()
        };
        f.model.insert("PartNo".into(), "B-100".into());
        f.drawing.insert("DrawnBy".into(), "AK".into());
        f.drawing.insert("Date".into(), "2026-10-13".into());
        assert_eq!(
            fill(
                "$PRP:\"PartNo\" $PRPSHEET:\"DrawnBy\" $SHEET/$SHEETS $SCALE $DATE $$5",
                &f
            ),
            "B-100 AK 2/3 1:2 2026-10-13 $5"
        );
        assert_eq!(fill("$PRP:\"Missing\"|$UNKNOWN", &f), "|$UNKNOWN");
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20_739), (2026, 10, 13));
        assert_eq!(civil(-1), (1969, 12, 31));
    }

    #[test]
    fn builtin_formats_fit_their_paper() {
        for size in [
            crate::SheetSize::A4,
            crate::SheetSize::A3,
            crate::SheetSize::A0,
            crate::SheetSize::AnsiB,
            crate::SheetSize::AnsiD,
        ] {
            for name in ["iso", "ansi"] {
                let sheet = Sheet {
                    id: 1,
                    name: "Sheet1".into(),
                    size,
                    portrait: false,
                    scale: Default::default(),
                    projection: Projection::First,
                    format: Default::default(),
                    properties_view: None,
                    views: vec![],
                    annotations: vec![],
                };
                let f = builtin(name, &sheet).unwrap();
                let (w, h) = sheet.paper();
                assert!(f.frame[0][0] > 0.0 && f.frame[1][0] < w && f.frame[1][1] < h);
                assert!(
                    f.title_block[0][0] >= f.frame[0][0] - 1e-9,
                    "{name} {size:?}"
                );
                assert!(f.title_block[1][1] < f.frame[1][1]);
            }
        }
    }
}

//! Tables on a sheet: BOM, hole, revision, general and general tolerance
//! (ISO 2768-1). A table is data; its cells are worked out when the sheet is
//! drawn, so BOMs follow the assembly and hole tables follow the part.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::dims::Pick;
use crate::document::ViewRef;
use crate::graphics::{Graphics, HAlign, LineStyle, Owner, P2, Pen, layers};
use crate::standard::Standard;
use crate::text;

/// Which corner of the table `at` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Corner {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Where the rows past `max_rows` continue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Piece {
    /// Sheet ID; the table's own sheet when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet: Option<u32>,
    pub at: [f64; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Table {
    /// Sheet position of the `anchor` corner, mm.
    pub at: [f64; 2],
    #[serde(default)]
    pub anchor: Corner,
    #[serde(flatten)]
    pub kind: TableKind,
    /// Capital height of cell text; the standard's when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_height: Option<f64>,
    /// Rows per piece; the rest continue at `continued`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_rows: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub continued: Vec<Piece>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Numbering {
    /// `3.2` under item 3.
    #[default]
    Detailed,
    /// One sequence through every row.
    Flat,
    /// Only top-level rows are numbered.
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Column {
    Item,
    PartNumber,
    Description,
    Quantity,
    Material,
    Mass,
    Property { name: String },
}

impl Column {
    fn title(&self) -> String {
        match self {
            Column::Item => "ITEM NO.".into(),
            Column::PartNumber => "PART NUMBER".into(),
            Column::Description => "DESCRIPTION".into(),
            Column::Quantity => "QTY.".into(),
            Column::Material => "MATERIAL".into(),
            Column::Mass => "MASS".into(),
            Column::Property { name } => name.to_uppercase(),
        }
    }
}

fn bom_columns() -> Vec<Column> {
    vec![
        Column::Item,
        Column::PartNumber,
        Column::Description,
        Column::Quantity,
    ]
}

fn one() -> u32 {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Tags {
    /// A1, A2 for the first size, B1 for the next.
    #[default]
    Alphabetic,
    /// 1, 2, 3.
    Numeric,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RevisionRow {
    /// Letter or number; given automatically when empty.
    #[serde(default)]
    pub revision: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub approved: String,
    #[serde(default)]
    pub zone: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToleranceClass {
    /// Fine.
    F,
    /// Medium.
    M,
    /// Coarse.
    C,
    /// Very coarse.
    V,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "table")]
pub enum TableKind {
    /// Bill of materials of the assembly shown in `view`.
    Bom {
        view: u32,
        #[serde(default)]
        bom: of_assembly::bom::BomType,
        #[serde(default)]
        numbering: Numbering,
        #[serde(default = "bom_columns")]
        columns: Vec<Column>,
        #[serde(default = "one")]
        start: u32,
        #[serde(default = "one")]
        increment: u32,
    },
    /// Holes of a view measured from an origin vertex.
    Hole {
        origin: Pick,
        holes: Vec<ViewRef>,
        #[serde(default)]
        tags: Tags,
        /// Combine holes of the same size into one row each.
        #[serde(default)]
        combine: bool,
        /// Draw each hole's tag beside it.
        #[serde(default = "yes")]
        show_tags: bool,
    },
    Revision {
        rows: Vec<RevisionRow>,
        /// Number revisions 1, 2, 3 instead of A, B, C.
        #[serde(default)]
        numeric: bool,
    },
    General {
        cells: Vec<Vec<String>>,
        /// The first row is a header.
        #[serde(default)]
        header: bool,
    },
    /// ISO 2768-1 general tolerances for a class.
    GeneralTolerance { class: ToleranceClass },
    /// Bends of the flat pattern shown in `view`, tagged on the view.
    Bend {
        view: u32,
        /// Draw each bend's tag at its bend line.
        #[serde(default = "yes")]
        show_tags: bool,
    },
    /// Forms (punches) of the flat pattern in `view`, measured from an
    /// origin vertex or, without one, the flat's lower left corner.
    Punch {
        view: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        origin: Option<Pick>,
    },
}

fn yes() -> bool {
    true
}

/// Worked-out cells: a header row (maybe empty) and body rows.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cells {
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Column alignment.
    pub align: Vec<HAlign>,
    pub title: Option<String>,
}

impl Cells {
    pub fn csv(&self) -> String {
        let cell = |s: &str| {
            if s.contains([',', '"', '\n']) {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.to_string()
            }
        };
        let mut out = String::new();
        for r in std::iter::once(&self.header)
            .filter(|h| !h.is_empty())
            .chain(&self.rows)
        {
            out += &r.iter().map(|c| cell(c)).collect::<Vec<_>>().join(",");
            out.push('\n');
        }
        out
    }
}

/// Each BOM row's item number and the component key paths it counts.
pub type ItemKeys = Vec<(String, Vec<Vec<String>>)>;

/// BOM rows as table cells.
pub fn bom_cells(
    rows: &[of_assembly::bom::BomRow],
    numbering: Numbering,
    columns: &[Column],
    start: u32,
    increment: u32,
) -> (Cells, ItemKeys) {
    let mut cells = Cells {
        header: columns.iter().map(Column::title).collect(),
        ..Default::default()
    };
    cells.align = columns
        .iter()
        .map(|c| match c {
            Column::Item | Column::Quantity | Column::Mass => HAlign::Center,
            _ => HAlign::Left,
        })
        .collect();
    // Item numbers per row after numbering, and the components behind them.
    let mut items = Vec::new();
    let mut flat = 0u32;
    for r in rows {
        let item = match numbering {
            Numbering::Flat => {
                let n = start + flat * increment;
                flat += 1;
                n.to_string()
            }
            Numbering::Detailed | Numbering::None => {
                let mut parts = r.item.split('.');
                let top: u32 = parts.next().and_then(|t| t.parse().ok()).unwrap_or(1);
                let top = start + (top - 1) * increment;
                let rest: Vec<&str> = parts.collect();
                if rest.is_empty() {
                    top.to_string()
                } else if numbering == Numbering::None {
                    String::new()
                } else {
                    format!("{top}.{}", rest.join("."))
                }
            }
        };
        let row = columns
            .iter()
            .map(|c| match c {
                Column::Item => item.clone(),
                Column::PartNumber => format!("{}{}", "  ".repeat(r.level), r.part_number),
                Column::Description => r.description.clone(),
                Column::Quantity => r.quantity.to_string(),
                Column::Material => r.material.clone(),
                Column::Mass => r.mass.map(|m| format!("{m:.3}")).unwrap_or_default(),
                Column::Property { name } => r.properties.get(name).cloned().unwrap_or_default(),
            })
            .collect();
        cells.rows.push(row);
        items.push((item, r.components.clone()));
    }
    (cells, items)
}

/// Revision table cells with automatic letters or numbers, and the current
/// (last) revision.
pub fn revision_cells(rows: &[RevisionRow], numeric: bool) -> (Cells, Option<String>) {
    let mut cells = Cells {
        header: vec![
            "ZONE".into(),
            "REV.".into(),
            "DESCRIPTION".into(),
            "DATE".into(),
            "APPROVED".into(),
        ],
        align: vec![
            HAlign::Center,
            HAlign::Center,
            HAlign::Left,
            HAlign::Center,
            HAlign::Center,
        ],
        title: Some("REVISIONS".into()),
        ..Default::default()
    };
    let mut current = None;
    for (i, r) in rows.iter().enumerate() {
        let rev = if r.revision.is_empty() {
            if numeric {
                (i + 1).to_string()
            } else {
                crate::viewdraw::label_letters(i)
            }
        } else {
            r.revision.clone()
        };
        cells.rows.push(vec![
            r.zone.clone(),
            rev.clone(),
            r.description.clone(),
            r.date.clone(),
            r.approved.clone(),
        ]);
        current = Some(rev);
    }
    (cells, current)
}

/// ISO 2768-1 general tolerances: linear sizes, radii and chamfers, angles.
pub fn tolerance_cells(class: ToleranceClass) -> Cells {
    use ToleranceClass::*;
    let linear: [&str; 8] = match class {
        F => [
            "±0.05", "±0.05", "±0.1", "±0.15", "±0.2", "±0.3", "±0.5", "-",
        ],
        M => ["±0.1", "±0.1", "±0.2", "±0.3", "±0.5", "±0.8", "±1.2", "±2"],
        C => ["±0.2", "±0.3", "±0.5", "±0.8", "±1.2", "±2", "±3", "±4"],
        V => ["-", "±0.5", "±1", "±1.5", "±2.5", "±4", "±6", "±8"],
    };
    let radii: [&str; 3] = match class {
        F | M => ["±0.2", "±0.5", "±1"],
        C | V => ["±0.4", "±1", "±2"],
    };
    let angles: [&str; 5] = match class {
        F | M => ["±1°", "±0°30'", "±0°20'", "±0°10'", "±0°5'"],
        C => ["±1°30'", "±1°", "±0°30'", "±0°15'", "±0°10'"],
        V => ["±3°", "±2°", "±1°", "±0°30'", "±0°20'"],
    };
    let name = match class {
        F => "f (fine)",
        M => "m (medium)",
        C => "c (coarse)",
        V => "v (very coarse)",
    };
    let mut rows = vec![vec!["LINEAR".to_string(), String::new()]];
    for (r, v) in [
        "0.5-3",
        "3-6",
        "6-30",
        "30-120",
        "120-400",
        "400-1000",
        "1000-2000",
        "2000-4000",
    ]
    .iter()
    .zip(linear)
    {
        rows.push(vec![format!("{r} mm"), v.into()]);
    }
    rows.push(vec!["RADII, CHAMFERS".into(), String::new()]);
    for (r, v) in ["0.5-3", "3-6", ">6"].iter().zip(radii) {
        rows.push(vec![format!("{r} mm"), v.into()]);
    }
    rows.push(vec!["ANGLES (SHORTER SIDE)".into(), String::new()]);
    for (r, v) in ["≤10", "10-50", "50-120", "120-400", ">400"]
        .iter()
        .zip(angles)
    {
        rows.push(vec![format!("{r} mm"), v.into()]);
    }
    Cells {
        header: vec!["SIZE".into(), "TOLERANCE".into()],
        rows,
        align: vec![HAlign::Left, HAlign::Center],
        title: Some(format!("GENERAL TOLERANCES ISO 2768-{name}")),
    }
}

/// Draw table cells as a grid with its `anchor` corner at `at`. Returns the
/// table box.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    g: &mut Graphics,
    cells: &Cells,
    rows: std::ops::Range<usize>,
    at: P2,
    anchor: Corner,
    h: f64,
    std: &Standard,
    owner: &Owner,
) -> [P2; 2] {
    let pad = h * 0.6;
    let row_h = h * 2.2;
    let ncol = cells
        .header
        .len()
        .max(cells.rows.iter().map(Vec::len).max().unwrap_or(0))
        .max(1);
    let mut widths = vec![h * 3.0; ncol];
    for r in std::iter::once(&cells.header).chain(&cells.rows) {
        for (i, c) in r.iter().enumerate() {
            widths[i] = widths[i].max(text::line_width(c, h) + 2.0 * pad);
        }
    }
    // Widen the last column when the title is wider than the columns.
    if let Some(t) = &cells.title {
        let need = text::line_width(t, h) + 2.0 * pad - widths.iter().sum::<f64>();
        if need > 0.0 {
            *widths.last_mut().unwrap() += need;
        }
    }
    let body = &cells.rows[rows.clone()];
    let title_rows = usize::from(cells.title.is_some());
    let header_rows = usize::from(!cells.header.is_empty());
    let total_w: f64 = widths.iter().sum();
    let total_h = row_h * (title_rows + header_rows + body.len()) as f64;
    let (x0, y1) = match anchor {
        Corner::TopLeft => (at[0], at[1]),
        Corner::TopRight => (at[0] - total_w, at[1]),
        Corner::BottomLeft => (at[0], at[1] + total_h),
        Corner::BottomRight => (at[0] - total_w, at[1] + total_h),
    };
    let thin = Pen::new(LineStyle::Continuous, std.thin_line());
    let thick = Pen::new(LineStyle::Continuous, std.thick_line());
    let layer = layers::TABLES;
    g.rect([x0, y1 - total_h], [x0 + total_w, y1], thick, layer, owner);
    let mut y = y1;
    let text_row = |g: &mut Graphics, y: f64, cols: &[String], spans: bool| {
        let mut x = x0;
        for (i, c) in cols.iter().enumerate() {
            let w = if spans { total_w } else { widths[i] };
            let align = if spans {
                HAlign::Center
            } else {
                cells.align.get(i).copied().unwrap_or(HAlign::Left)
            };
            let tx = match align {
                HAlign::Left => x + pad,
                HAlign::Center => x + w / 2.0,
                HAlign::Right => x + w - pad,
            };
            for p in text::layout(c, [tx, y - row_h / 2.0 - h / 2.0], h, 0.0, align) {
                g.push(p, thin, layer, owner);
            }
            x += w;
        }
    };
    if let Some(t) = &cells.title {
        text_row(g, y, std::slice::from_ref(t), true);
        y -= row_h;
        g.line([x0, y], [x0 + total_w, y], thin, layer, owner);
    }
    if !cells.header.is_empty() {
        text_row(g, y, &cells.header, false);
        y -= row_h;
        g.line([x0, y], [x0 + total_w, y], thick, layer, owner);
    }
    for r in body {
        text_row(g, y, r, false);
        y -= row_h;
        g.line([x0, y], [x0 + total_w, y], thin, layer, owner);
    }
    // Column lines below the title.
    let col_top = y1 - row_h * title_rows as f64;
    let mut x = x0;
    for w in &widths[..ncol - 1] {
        x += w;
        g.line([x, y1 - total_h], [x, col_top], thin, layer, owner);
    }
    [[x0, y1 - total_h], [x0 + total_w, y1]]
}

/// Which body rows each piece of a split table shows.
pub fn pieces(n: usize, max_rows: Option<usize>, continued: usize) -> Vec<std::ops::Range<usize>> {
    let Some(m) = max_rows.filter(|m| *m > 0) else {
        return std::iter::once(0..n).collect();
    };
    let mut out = Vec::new();
    let mut s = 0;
    while s < n && out.len() <= continued {
        // The last piece takes whatever is left.
        let e = if out.len() == continued {
            n
        } else {
            (s + m).min(n)
        };
        out.push(s..e);
        s = e;
    }
    if out.is_empty() {
        out.push(0..0);
    }
    out
}

/// Tag of the `i`th hole (0-based) of the `size`th size.
pub fn hole_tag(tags: Tags, size: usize, i: usize, global: usize) -> String {
    match tags {
        Tags::Alphabetic => format!("{}{}", crate::viewdraw::label_letters(size), i + 1),
        Tags::Numeric => (global + 1).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_split_rows() {
        assert_eq!(pieces(10, None, 0), vec![0..10]);
        assert_eq!(pieces(10, Some(4), 1), vec![0..4, 4..10]);
        assert_eq!(pieces(10, Some(4), 5), vec![0..4, 4..8, 8..10]);
    }

    #[test]
    fn revisions_letter_themselves_and_tolerances_tabulate() {
        let rows = vec![
            RevisionRow {
                revision: String::new(),
                description: "First issue".into(),
                date: "2026-10-01".into(),
                approved: "AK".into(),
                zone: String::new(),
            },
            RevisionRow {
                revision: String::new(),
                description: "Hole moved".into(),
                date: String::new(),
                approved: String::new(),
                zone: "B3".into(),
            },
        ];
        let (c, current) = revision_cells(&rows, false);
        assert_eq!(current.as_deref(), Some("B"));
        assert_eq!(c.rows[0][1], "A");
        let t = tolerance_cells(ToleranceClass::M);
        assert!(t.rows.iter().any(|r| r[0] == "30-120 mm" && r[1] == "±0.3"));
        assert!(t.csv().starts_with("SIZE,TOLERANCE\n"));
    }

    #[test]
    fn grid_has_title_header_and_rows() {
        let mut g = Graphics::new(200.0, 200.0);
        let (c, _) = revision_cells(
            &[RevisionRow {
                revision: "A".into(),
                description: "Issue".into(),
                date: String::new(),
                approved: String::new(),
                zone: String::new(),
            }],
            false,
        );
        let b = draw(
            &mut g,
            &c,
            0..1,
            [190.0, 190.0],
            Corner::TopRight,
            2.5,
            &Standard::iso(),
            &Owner::Annotation { id: 1 },
        );
        assert!((b[1][0] - 190.0).abs() < 1e-9);
        assert!((b[1][1] - b[0][1] - 3.0 * 2.5 * 2.2).abs() < 1e-9);
    }
}

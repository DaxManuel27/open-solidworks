//! DXF 2018 (AC1032) output. Views, and the annotations that move with
//! them, go to model space at true size (sheet coordinates divided by the
//! sheet scale; sheets side by side). Each sheet becomes a paper space
//! layout at true paper size with its border, title block, sheet notes and
//! tables, and a viewport showing its part of model space at the sheet
//! scale, so the layout reproduces the sheet exactly. Lines carry their
//! lineweight and colour; text is UTF-8 in a style naming the bundled font.

use crate::dxf::{DxfSheet, Out, aci, layer_name, ltype_name, triangulate};
use crate::graphics::{Item, LineStyle, Owner, P2, Prim};
use crate::standard::Standard;
use crate::text::{FONT_FAMILY, cap_for};

/// Lineweights DXF allows, 1/100 mm.
const LINEWEIGHTS: [i32; 24] = [
    0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120, 140, 158, 200,
    211,
];

/// The allowed lineweight nearest a weight in mm.
pub(crate) fn lineweight(mm: f64) -> i32 {
    let w = (mm * 100.0).round() as i32;
    *LINEWEIGHTS
        .iter()
        .min_by_key(|l| (**l - w).abs())
        .unwrap_or(&25)
}

/// Gap between sheets placed side by side in model space, mm.
const SHEET_GAP: f64 = 50.0;

/// Handles, allocated in order.
struct Handles(u32);

impl Handles {
    fn next(&mut self) -> String {
        self.0 += 1;
        format!("{:X}", self.0)
    }
}

/// Handles of one sheet's layout.
struct LayoutIds {
    record: String,
    layout: String,
    block: String,
    endblk: String,
}

/// Which space an item goes to, by owner.
fn in_model(item: &Item, sheet: &DxfSheet) -> bool {
    match &item.owner {
        Owner::Format => false,
        Owner::View { .. } => true,
        Owner::Annotation { id } => sheet.view_annotations.contains(id),
    }
}

impl Out {
    fn handle(&mut self, h: &str) {
        self.pair(5, h);
    }

    /// Start a symbol table.
    fn table(&mut self, name: &str, h: &str, count: usize) {
        self.pair(0, "TABLE");
        self.pair(2, name);
        self.handle(h);
        self.pair(330, 0);
        self.pair(100, "AcDbSymbolTable");
        self.pair(70, count);
    }

    /// Start a table record.
    fn record(&mut self, kind: &str, h: &str, table: &str, class: &str) {
        self.pair(0, kind);
        self.handle(h);
        self.pair(330, table);
        self.pair(100, "AcDbSymbolTableRecord");
        self.pair(100, class);
    }

    fn xyz(&mut self, code: i32, p: [f64; 3]) {
        self.num(code, p[0]);
        self.num(code + 10, p[1]);
        self.num(code + 20, p[2]);
    }
}

/// Where a sheet's model-space content goes: x offset and scale.
#[derive(Clone, Copy)]
struct Place {
    dx: f64,
    scale: f64,
}

impl Place {
    fn at(&self, p: &P2) -> P2 {
        [p[0] / self.scale + self.dx, p[1] / self.scale]
    }
}

/// Write one primitive. `place` maps sheet mm to the space it goes to;
/// `size` scales lengths (radii, text heights).
#[allow(clippy::too_many_arguments)]
fn entity(
    o: &mut Out,
    h: &mut Handles,
    owner: &str,
    paper: bool,
    item: &Item,
    layer: &str,
    at: &dyn Fn(&P2) -> P2,
    size: f64,
) {
    let head = |o: &mut Out, h: &mut Handles, kind: &str| {
        o.pair(0, kind);
        o.handle(&h.next());
        o.pair(330, owner);
        o.pair(100, "AcDbEntity");
        if paper {
            o.pair(67, 1);
        }
        o.pair(8, layer);
        o.pair(6, ltype_name(item.pen.style));
        let c = item.pen.color;
        o.pair(62, aci(c));
        if c != [0, 0, 0] {
            o.pair(
                420,
                (i32::from(c[0]) << 16) | (i32::from(c[1]) << 8) | i32::from(c[2]),
            );
        }
        o.pair(370, lineweight(item.pen.weight));
    };
    match &item.prim {
        Prim::Line { a, b } => {
            head(o, h, "LINE");
            o.pair(100, "AcDbLine");
            o.point(10, at(a));
            o.point(11, at(b));
        }
        Prim::Arc {
            center,
            radius,
            start,
            end,
        } => {
            let full = (end - start).abs() >= std::f64::consts::TAU - 1e-9;
            head(o, h, if full { "CIRCLE" } else { "ARC" });
            o.pair(100, "AcDbCircle");
            o.point(10, at(center));
            o.num(40, radius * size);
            if !full {
                o.pair(100, "AcDbArc");
                o.num(50, start.to_degrees().rem_euclid(360.0));
                o.num(51, end.to_degrees().rem_euclid(360.0));
            }
        }
        Prim::Polyline { points, closed } => {
            if points.len() < 2 {
                return;
            }
            head(o, h, "LWPOLYLINE");
            o.pair(100, "AcDbPolyline");
            o.pair(90, points.len());
            o.pair(70, if *closed { 1 } else { 0 });
            o.num(43, 0.0);
            for p in points {
                let q = at(p);
                o.num(10, q[0]);
                o.num(20, q[1]);
            }
        }
        Prim::Fill { loops } => {
            for l in loops {
                for [a, b, c] in triangulate(l) {
                    head(o, h, "SOLID");
                    o.pair(100, "AcDbTrace");
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
            head(o, h, "TEXT");
            o.pair(100, "AcDbText");
            o.point(10, at(p));
            o.num(40, cap_for(*height) * size);
            o.pair(1, text.replace(['\n', '\r'], " "));
            o.num(50, angle.to_degrees().rem_euclid(360.0));
            o.pair(7, "OPENFORGE");
            o.pair(100, "AcDbText");
        }
    }
}

/// A viewport entity in paper space.
#[allow(clippy::too_many_arguments)]
fn viewport(
    o: &mut Out,
    h: &mut Handles,
    owner: &str,
    id: i32,
    center: P2,
    size: P2,
    view_center: P2,
    view_height: f64,
) {
    o.pair(0, "VIEWPORT");
    o.handle(&h.next());
    o.pair(330, owner);
    o.pair(100, "AcDbEntity");
    o.pair(67, 1);
    o.pair(8, "VIEWPORTS");
    o.pair(100, "AcDbViewport");
    o.point(10, center);
    o.num(40, size[0]);
    o.num(41, size[1]);
    o.pair(68, id);
    o.pair(69, id);
    o.num(12, view_center[0]);
    o.num(22, view_center[1]);
    o.num(13, 0.0);
    o.num(23, 0.0);
    o.num(14, 10.0);
    o.num(24, 10.0);
    o.num(15, 10.0);
    o.num(25, 10.0);
    o.xyz(16, [0.0, 0.0, 1.0]);
    o.xyz(17, [0.0, 0.0, 0.0]);
    o.num(42, 50.0);
    o.num(43, 0.0);
    o.num(44, 0.0);
    o.num(45, view_height);
    o.num(50, 0.0);
    o.num(51, 0.0);
    o.pair(72, 100);
    // Locked display, so zooming in a layout cannot change the scale.
    o.pair(90, 16384);
    o.pair(1, "");
    o.pair(281, 0);
    o.pair(71, 0);
    o.pair(74, 0);
    o.xyz(110, [0.0, 0.0, 0.0]);
    o.xyz(111, [1.0, 0.0, 0.0]);
    o.xyz(112, [0.0, 1.0, 0.0]);
    o.pair(79, 0);
    o.num(146, 0.0);
}

/// A LAYOUT object.
#[allow(clippy::too_many_arguments)]
fn layout(
    o: &mut Out,
    handle: &str,
    dict: &str,
    name: &str,
    tab: usize,
    paper: P2,
    record: &str,
    model: bool,
) {
    o.pair(0, "LAYOUT");
    o.handle(handle);
    o.pair(102, "{ACAD_REACTORS");
    o.pair(330, dict);
    o.pair(102, "}");
    o.pair(330, dict);
    o.pair(100, "AcDbPlotSettings");
    o.pair(1, "");
    o.pair(4, "");
    o.pair(6, "");
    for code in [40, 41, 42, 43] {
        o.num(code, 0.0);
    }
    o.num(44, paper[0]);
    o.num(45, paper[1]);
    for code in [46, 47, 48, 49, 140, 141] {
        o.num(code, 0.0);
    }
    o.num(142, 1.0);
    o.num(143, 1.0);
    o.pair(70, if model { 1024 } else { 0 });
    // Millimetres, no rotation, layout extents, plot at 1:1.
    o.pair(72, 1);
    o.pair(73, 0);
    o.pair(74, 5);
    o.pair(7, "");
    o.pair(75, 16);
    o.pair(76, 0);
    o.pair(77, 2);
    o.pair(78, 300);
    o.num(147, 1.0);
    o.num(148, 0.0);
    o.num(149, 0.0);
    o.pair(100, "AcDbLayout");
    o.pair(1, name);
    o.pair(70, 1);
    o.pair(71, tab);
    o.num(10, 0.0);
    o.num(20, 0.0);
    o.num(11, paper[0]);
    o.num(21, paper[1]);
    o.xyz(12, [0.0, 0.0, 0.0]);
    o.xyz(14, [0.0, 0.0, 0.0]);
    o.xyz(15, [paper[0], paper[1], 0.0]);
    o.num(146, 0.0);
    o.xyz(13, [0.0, 0.0, 0.0]);
    o.xyz(16, [1.0, 0.0, 0.0]);
    o.xyz(17, [0.0, 1.0, 0.0]);
    o.pair(76, 1);
    o.pair(330, record);
}

/// Unique layout names: DXF layout names must differ (case-insensitively).
fn layout_names(sheets: &[DxfSheet]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (i, s) in sheets.iter().enumerate() {
        let clean: String = s
            .name
            .chars()
            .map(|c| {
                if "<>/\\\":;?*|=`".contains(c) || c.is_control() {
                    '_'
                } else {
                    c
                }
            })
            .collect();
        let mut name = if clean.trim().is_empty() || clean.eq_ignore_ascii_case("model") {
            format!("Sheet{}", i + 1)
        } else {
            clean.trim().to_string()
        };
        let base = name.clone();
        let mut n = 2;
        while out.iter().any(|o| o.eq_ignore_ascii_case(&name)) {
            name = format!("{base} ({n})");
            n += 1;
        }
        out.push(name);
    }
    out
}

pub fn write(sheets: &[DxfSheet], standard: &Standard) -> Result<String, String> {
    if sheets.is_empty() {
        return Err("nothing to write: the drawing has no sheets".into());
    }
    for s in sheets {
        if !(s.scale.is_finite() && s.scale > 0.0) {
            return Err(format!("sheet {} has an invalid scale", s.name));
        }
    }
    let names = layout_names(sheets);
    // Model space: each sheet's content at true size, side by side.
    let mut places = Vec::new();
    let mut dx = 0.0;
    for s in sheets {
        places.push(Place { dx, scale: s.scale });
        dx += s.graphics.width / s.scale + SHEET_GAP;
    }
    let model_hi = [
        dx - SHEET_GAP,
        sheets
            .iter()
            .map(|s| s.graphics.height / s.scale)
            .fold(0.0, f64::max),
    ];

    // Layers with the line style of their first item.
    let mut layers: Vec<(String, LineStyle)> = Vec::new();
    for s in sheets {
        for item in &s.graphics.items {
            let name = layer_name(&item.layer, None);
            if name != "0" && !layers.iter().any(|(n, _)| *n == name) {
                layers.push((name, item.pen.style));
            }
        }
    }

    let mut h = Handles(0);
    let t_vport = h.next();
    let t_ltype = h.next();
    let t_layer = h.next();
    let t_style = h.next();
    let t_view = h.next();
    let t_ucs = h.next();
    let t_appid = h.next();
    let t_dimstyle = h.next();
    let t_record = h.next();
    let d_root = h.next();
    let d_group = h.next();
    let d_layout = h.next();
    let d_plotstyle = h.next();
    let placeholder = h.next();
    let model = LayoutIds {
        record: h.next(),
        layout: h.next(),
        block: h.next(),
        endblk: h.next(),
    };
    let papers: Vec<LayoutIds> = sheets
        .iter()
        .map(|_| LayoutIds {
            record: h.next(),
            layout: h.next(),
            block: h.next(),
            endblk: h.next(),
        })
        .collect();
    // Block record names: the first layout is the active paper space.
    let record_name = |i: usize| {
        if i == 0 {
            "*Paper_Space".to_string()
        } else {
            format!("*Paper_Space{}", i - 1)
        }
    };

    let mut o = Out(String::with_capacity(1 << 16));

    // Classes for the non-entity objects written below.
    o.pair(0, "SECTION");
    o.pair(2, "CLASSES");
    for (name, class) in [
        ("ACDBDICTIONARYWDFLT", "AcDbDictionaryWithDefault"),
        ("ACDBPLACEHOLDER", "AcDbPlaceHolder"),
        ("LAYOUT", "AcDbLayout"),
    ] {
        o.pair(0, "CLASS");
        o.pair(1, name);
        o.pair(2, class);
        o.pair(3, "ObjectDBX Classes");
        o.pair(90, 0);
        o.pair(91, 0);
        o.pair(280, 0);
        o.pair(281, 0);
    }
    o.pair(0, "ENDSEC");

    o.pair(0, "SECTION");
    o.pair(2, "TABLES");
    o.table("VPORT", &t_vport, 1);
    o.record("VPORT", &h.next(), &t_vport, "AcDbViewportTableRecord");
    o.pair(2, "*Active");
    o.pair(70, 0);
    o.num(10, 0.0);
    o.num(20, 0.0);
    o.num(11, 1.0);
    o.num(21, 1.0);
    o.num(12, model_hi[0] / 2.0);
    o.num(22, model_hi[1] / 2.0);
    o.num(13, 0.0);
    o.num(23, 0.0);
    o.num(14, 10.0);
    o.num(24, 10.0);
    o.num(15, 10.0);
    o.num(25, 10.0);
    o.xyz(16, [0.0, 0.0, 1.0]);
    o.xyz(17, [0.0, 0.0, 0.0]);
    o.num(40, model_hi[1].max(1.0) * 1.1);
    o.num(41, (model_hi[0] / model_hi[1].max(1.0)).max(0.1));
    o.num(42, 50.0);
    o.num(43, 0.0);
    o.num(44, 0.0);
    o.num(50, 0.0);
    o.num(51, 0.0);
    o.pair(71, 0);
    o.pair(72, 1000);
    o.pair(73, 1);
    o.pair(74, 3);
    o.pair(75, 0);
    o.pair(76, 0);
    o.pair(77, 0);
    o.pair(78, 0);
    o.pair(281, 0);
    o.pair(65, 0);
    o.num(146, 0.0);
    o.pair(0, "ENDTAB");

    let styles = [
        LineStyle::Continuous,
        LineStyle::Hidden,
        LineStyle::Center,
        LineStyle::Phantom,
    ];
    o.table("LTYPE", &t_ltype, 2 + styles.len());
    for name in ["ByBlock", "ByLayer"] {
        o.record("LTYPE", &h.next(), &t_ltype, "AcDbLinetypeTableRecord");
        o.pair(2, name);
        o.pair(70, 0);
        o.pair(3, "");
        o.pair(72, 65);
        o.pair(73, 0);
        o.num(40, 0.0);
    }
    for style in styles {
        let dashes = standard.dashes(style);
        o.record("LTYPE", &h.next(), &t_ltype, "AcDbLinetypeTableRecord");
        o.pair(2, ltype_name(style));
        o.pair(70, 0);
        o.pair(3, if dashes.is_empty() { "Solid line" } else { "" });
        o.pair(72, 65);
        o.pair(73, dashes.len());
        o.num(40, dashes.iter().sum());
        for (i, d) in dashes.iter().enumerate() {
            // Dashes are positive, gaps negative.
            o.num(49, if i % 2 == 0 { *d } else { -*d });
            o.pair(74, 0);
        }
    }
    o.pair(0, "ENDTAB");

    o.table("LAYER", &t_layer, layers.len() + 2);
    let layer = |o: &mut Out, h: &mut Handles, name: &str, ltype: &str, plot: bool| {
        o.record("LAYER", &h.next(), &t_layer, "AcDbLayerTableRecord");
        o.pair(2, name);
        o.pair(70, 0);
        o.pair(62, 7);
        o.pair(6, ltype);
        if !plot {
            o.pair(290, 0);
        }
        o.pair(370, -3);
        o.pair(390, &placeholder);
    };
    layer(&mut o, &mut h, "0", "CONTINUOUS", true);
    layer(&mut o, &mut h, "VIEWPORTS", "CONTINUOUS", false);
    for (name, style) in &layers {
        layer(&mut o, &mut h, name, ltype_name(*style), true);
    }
    o.pair(0, "ENDTAB");

    o.table("STYLE", &t_style, 2);
    for (name, font) in [
        ("Standard", "txt"),
        ("OPENFORGE", "DejaVuSansCondensed.ttf"),
    ] {
        o.record("STYLE", &h.next(), &t_style, "AcDbTextStyleTableRecord");
        o.pair(2, name);
        o.pair(70, 0);
        o.num(40, 0.0);
        o.num(41, 1.0);
        o.num(50, 0.0);
        o.pair(71, 0);
        o.num(42, 3.5);
        o.pair(3, font);
        o.pair(4, "");
        if name == "OPENFORGE" {
            // The TrueType family, for readers that look fonts up by name.
            o.pair(1001, "ACAD");
            o.pair(1000, FONT_FAMILY);
            o.pair(1071, 34);
        }
    }
    o.pair(0, "ENDTAB");

    o.table("VIEW", &t_view, 0);
    o.pair(0, "ENDTAB");
    o.table("UCS", &t_ucs, 0);
    o.pair(0, "ENDTAB");

    o.table("APPID", &t_appid, 2);
    for name in ["ACAD", "OPENFORGE"] {
        o.record("APPID", &h.next(), &t_appid, "AcDbRegAppTableRecord");
        o.pair(2, name);
        o.pair(70, 0);
    }
    o.pair(0, "ENDTAB");

    o.table("DIMSTYLE", &t_dimstyle, 1);
    o.pair(100, "AcDbDimStyleTable");
    o.pair(0, "DIMSTYLE");
    o.pair(105, h.next());
    o.pair(330, &t_dimstyle);
    o.pair(100, "AcDbSymbolTableRecord");
    o.pair(100, "AcDbDimStyleTableRecord");
    o.pair(2, "Standard");
    o.pair(70, 0);
    o.num(41, standard.arrow_length());
    o.num(42, standard.extension_gap());
    o.num(44, standard.extension_overshoot());
    o.num(140, standard.text_height());
    o.pair(0, "ENDTAB");

    o.table("BLOCK_RECORD", &t_record, 1 + sheets.len());
    let block_record = |o: &mut Out, ids: &LayoutIds, name: &str| {
        o.record(
            "BLOCK_RECORD",
            &ids.record,
            &t_record,
            "AcDbBlockTableRecord",
        );
        o.pair(2, name);
        o.pair(340, &ids.layout);
        o.pair(70, 0);
        o.pair(280, 1);
        o.pair(281, 0);
    };
    block_record(&mut o, &model, "*Model_Space");
    for (i, ids) in papers.iter().enumerate() {
        block_record(&mut o, ids, &record_name(i));
    }
    o.pair(0, "ENDTAB");
    o.pair(0, "ENDSEC");

    // Entities of a sheet's paper space (format, sheet annotations and the
    // viewports showing its model space).
    let paper_entities = |o: &mut Out, h: &mut Handles, i: usize| {
        let s = &sheets[i];
        let owner = &papers[i].record;
        let (w, ht) = (s.graphics.width, s.graphics.height);
        let mid = [w / 2.0, ht / 2.0];
        viewport(o, h, owner, 1, mid, [w, ht], mid, ht);
        let place = places[i];
        viewport(
            o,
            h,
            owner,
            2,
            mid,
            [w, ht],
            place.at(&mid),
            ht / place.scale,
        );
        for item in &s.graphics.items {
            if !in_model(item, s) {
                let name = layer_name(&item.layer, None);
                entity(o, h, owner, true, item, &name, &|p| *p, 1.0);
            }
        }
    };

    o.pair(0, "SECTION");
    o.pair(2, "BLOCKS");
    let block = |o: &mut Out, ids: &LayoutIds, name: &str, paper: bool| {
        o.pair(0, "BLOCK");
        o.handle(&ids.block);
        o.pair(330, &ids.record);
        o.pair(100, "AcDbEntity");
        if paper {
            o.pair(67, 1);
        }
        o.pair(8, "0");
        o.pair(100, "AcDbBlockBegin");
        o.pair(2, name);
        o.pair(70, 0);
        o.xyz(10, [0.0, 0.0, 0.0]);
        o.pair(3, name);
        o.pair(1, "");
    };
    let endblk = |o: &mut Out, ids: &LayoutIds, paper: bool| {
        o.pair(0, "ENDBLK");
        o.handle(&ids.endblk);
        o.pair(330, &ids.record);
        o.pair(100, "AcDbEntity");
        if paper {
            o.pair(67, 1);
        }
        o.pair(8, "0");
        o.pair(100, "AcDbBlockEnd");
    };
    block(&mut o, &model, "*Model_Space", false);
    endblk(&mut o, &model, false);
    for (i, ids) in papers.iter().enumerate() {
        block(&mut o, ids, &record_name(i), true);
        // Inactive layouts keep their entities in their block.
        if i > 0 {
            paper_entities(&mut o, &mut h, i);
        }
        endblk(&mut o, ids, true);
    }
    o.pair(0, "ENDSEC");

    o.pair(0, "SECTION");
    o.pair(2, "ENTITIES");
    for (i, s) in sheets.iter().enumerate() {
        let place = places[i];
        for item in &s.graphics.items {
            if in_model(item, s) {
                let name = layer_name(&item.layer, None);
                entity(
                    &mut o,
                    &mut h,
                    &model.record,
                    false,
                    item,
                    &name,
                    &|p| place.at(p),
                    1.0 / place.scale,
                );
            }
        }
    }
    paper_entities(&mut o, &mut h, 0);
    o.pair(0, "ENDSEC");

    o.pair(0, "SECTION");
    o.pair(2, "OBJECTS");
    o.pair(0, "DICTIONARY");
    o.handle(&d_root);
    o.pair(330, 0);
    o.pair(100, "AcDbDictionary");
    o.pair(281, 1);
    for (name, handle) in [
        ("ACAD_GROUP", &d_group),
        ("ACAD_LAYOUT", &d_layout),
        ("ACAD_PLOTSTYLENAME", &d_plotstyle),
    ] {
        o.pair(3, name);
        o.pair(350, handle);
    }
    o.pair(0, "DICTIONARY");
    o.handle(&d_group);
    o.pair(330, &d_root);
    o.pair(100, "AcDbDictionary");
    o.pair(281, 1);
    o.pair(0, "DICTIONARY");
    o.handle(&d_layout);
    o.pair(330, &d_root);
    o.pair(100, "AcDbDictionary");
    o.pair(281, 1);
    o.pair(3, "Model");
    o.pair(350, &model.layout);
    for (name, ids) in names.iter().zip(&papers) {
        o.pair(3, name);
        o.pair(350, &ids.layout);
    }
    o.pair(0, "ACDBDICTIONARYWDFLT");
    o.handle(&d_plotstyle);
    o.pair(330, &d_root);
    o.pair(100, "AcDbDictionary");
    o.pair(281, 1);
    o.pair(3, "Normal");
    o.pair(350, &placeholder);
    o.pair(100, "AcDbDictionaryWithDefault");
    o.pair(340, &placeholder);
    o.pair(0, "ACDBPLACEHOLDER");
    o.handle(&placeholder);
    o.pair(330, &d_plotstyle);
    let first = &sheets[0].graphics;
    layout(
        &mut o,
        &model.layout,
        &d_layout,
        "Model",
        0,
        [first.width, first.height],
        &model.record,
        true,
    );
    for (i, (name, ids)) in names.iter().zip(&papers).enumerate() {
        let g = &sheets[i].graphics;
        layout(
            &mut o,
            &ids.layout,
            &d_layout,
            name,
            i + 1,
            [g.width, g.height],
            &ids.record,
            false,
        );
    }
    o.pair(0, "ENDSEC");
    o.pair(0, "EOF");

    // The header goes first but needs the next free handle.
    let mut head = Out(String::with_capacity(2048));
    head.pair(0, "SECTION");
    head.pair(2, "HEADER");
    head.pair(9, "$ACADVER");
    head.pair(1, "AC1032");
    head.pair(9, "$DWGCODEPAGE");
    head.pair(3, "ANSI_1252");
    head.pair(9, "$INSBASE");
    head.xyz(10, [0.0, 0.0, 0.0]);
    head.pair(9, "$EXTMIN");
    head.xyz(10, [0.0, 0.0, 0.0]);
    head.pair(9, "$EXTMAX");
    head.xyz(10, [model_hi[0], model_hi[1], 0.0]);
    head.pair(9, "$LIMMIN");
    head.num(10, 0.0);
    head.num(20, 0.0);
    head.pair(9, "$LIMMAX");
    head.num(10, model_hi[0]);
    head.num(20, model_hi[1]);
    head.pair(9, "$LTSCALE");
    head.num(40, 1.0);
    // Line types in viewports are sized on paper.
    head.pair(9, "$PSLTSCALE");
    head.pair(70, 1);
    head.pair(9, "$TILEMODE");
    head.pair(70, 0);
    head.pair(9, "$INSUNITS");
    head.pair(70, 4);
    head.pair(9, "$MEASUREMENT");
    head.pair(70, 1);
    head.pair(9, "$LWDISPLAY");
    head.pair(290, 1);
    head.pair(9, "$HANDSEED");
    head.pair(5, h.next());
    head.pair(0, "ENDSEC");
    head.0.push_str(&o.0);
    Ok(head.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lineweights_snap_to_allowed_values() {
        assert_eq!(lineweight(0.5), 50);
        assert_eq!(lineweight(0.25), 25);
        assert_eq!(lineweight(0.35), 35);
        assert_eq!(lineweight(0.6), 60);
        assert_eq!(lineweight(0.3175), 30);
    }

    #[test]
    fn layout_names_are_unique() {
        let g = crate::graphics::Graphics::new(10.0, 10.0);
        let sheet = |n: &str| DxfSheet {
            graphics: &g,
            name: n.into(),
            scale: 1.0,
            view_annotations: Vec::new(),
        };
        let sheets = [sheet("Plate"), sheet("plate"), sheet("Model"), sheet("a/b")];
        assert_eq!(
            layout_names(&sheets),
            ["Plate", "plate (2)", "Sheet3", "a_b"]
        );
    }
}

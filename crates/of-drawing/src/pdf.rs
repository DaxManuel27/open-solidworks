//! PDF output: one page per sheet at true paper size, vector strokes with
//! line styles and weights, text in an embedded subset of the bundled font
//! (searchable through a ToUnicode map), and drawing layers as optional
//! content groups.

use std::collections::BTreeMap;

use pdf_writer::types::{
    CidFontType, FontFlags, LineCapStyle, LineJoinStyle, SystemInfo, UnicodeCmap,
};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str, TextStr};

use crate::graphics::{Graphics, LineStyle, P2, Prim};
use crate::standard::Standard;
use crate::text::FONT;

const PT: f64 = 72.0 / 25.4;

/// Output options.
#[derive(Debug, Clone, Default)]
pub struct PdfOptions {
    /// Draw everything black, ignoring colours.
    pub monochrome: bool,
    /// Document title.
    pub title: Option<String>,
}

fn deflate(data: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(data, 6)
}

/// Write sheets as a PDF document.
pub fn write(
    sheets: &[Graphics],
    standard: &Standard,
    options: &PdfOptions,
) -> Result<Vec<u8>, String> {
    if sheets.is_empty() {
        return Err("nothing to write: the drawing has no sheets".into());
    }
    let face = ttf_parser::Face::parse(FONT, 0).map_err(|e| format!("font: {e}"))?;
    let units = f64::from(face.units_per_em());

    // Glyphs used anywhere, so one subset serves every page.
    let mut remapper = subsetter::GlyphRemapper::new();
    let mut to_unicode: BTreeMap<u16, char> = BTreeMap::new();
    for g in sheets {
        for i in &g.items {
            if let Prim::Text { text, .. } = &i.prim {
                for c in text.chars() {
                    if let Some(gid) = face.glyph_index(c) {
                        let new = remapper.remap(gid.0);
                        to_unicode.entry(new).or_insert(c);
                    }
                }
            }
        }
    }

    let mut next = 1;
    let mut alloc = || {
        let r = Ref::new(next);
        next += 1;
        r
    };
    let catalog = alloc();
    let tree = alloc();
    let info = alloc();
    let font = alloc();
    let cid = alloc();
    let descriptor = alloc();
    let cmap = alloc();
    let file = alloc();

    let mut layer_names: Vec<String> = Vec::new();
    for g in sheets {
        for l in g.layers() {
            if !layer_names.iter().any(|n| n == l) {
                layer_names.push(l.to_string());
            }
        }
    }
    let layers: Vec<(String, Ref)> = layer_names.into_iter().map(|n| (n, alloc())).collect();
    let pages: Vec<(Ref, Ref)> = sheets.iter().map(|_| (alloc(), alloc())).collect();

    let mut pdf = Pdf::new();
    {
        let mut cat = pdf.catalog(catalog);
        cat.pages(tree);
        let mut oc = cat.insert(Name(b"OCProperties")).dict();
        oc.insert(Name(b"OCGs"))
            .array()
            .items(layers.iter().map(|(_, r)| *r));
        let mut d = oc.insert(Name(b"D")).dict();
        d.insert(Name(b"Order"))
            .array()
            .items(layers.iter().map(|(_, r)| *r));
        d.finish();
        oc.finish();
    }
    pdf.pages(tree)
        .kids(pages.iter().map(|(p, _)| *p))
        .count(pages.len() as i32);
    {
        let mut di = pdf.document_info(info);
        di.producer(TextStr("OpenForge"));
        if let Some(t) = &options.title {
            di.title(TextStr(t));
        }
    }
    for (name, r) in &layers {
        let mut d = pdf.indirect(*r).dict();
        d.pair(Name(b"Type"), Name(b"OCG"));
        d.pair(Name(b"Name"), TextStr(name));
    }

    // The font: Type0 with Identity-H over the subset's glyph IDs.
    let base = Name(b"OFAAAA+DejaVuSansCondensed");
    pdf.type0_font(font)
        .base_font(base)
        .encoding_predefined(Name(b"Identity-H"))
        .descendant_font(cid)
        .to_unicode(cmap);
    let system = SystemInfo {
        registry: Str(b"Adobe"),
        ordering: Str(b"Identity"),
        supplement: 0,
    };
    {
        let mut c = pdf.cid_font(cid);
        c.subtype(CidFontType::Type2)
            .base_font(base)
            .system_info(system)
            .font_descriptor(descriptor)
            .default_width(0.0)
            .cid_to_gid_map_predefined(Name(b"Identity"));
        let mut widths = c.widths();
        for (&new, &ch) in &to_unicode {
            let w = face
                .glyph_index(ch)
                .and_then(|g| face.glyph_hor_advance(g))
                .map(f64::from)
                .unwrap_or(0.0);
            widths.consecutive(new, [(w * 1000.0 / units) as f32]);
        }
    }
    let scale = 1000.0 / units;
    let bbox = face.global_bounding_box();
    pdf.font_descriptor(descriptor)
        .name(base)
        .flags(FontFlags::NON_SYMBOLIC)
        .bbox(Rect::new(
            (f64::from(bbox.x_min) * scale) as f32,
            (f64::from(bbox.y_min) * scale) as f32,
            (f64::from(bbox.x_max) * scale) as f32,
            (f64::from(bbox.y_max) * scale) as f32,
        ))
        .italic_angle(0.0)
        .ascent((f64::from(face.ascender()) * scale) as f32)
        .descent((f64::from(face.descender()) * scale) as f32)
        .cap_height((f64::from(face.capital_height().unwrap_or(1493)) * scale) as f32)
        .stem_v(80.0)
        .font_file2(file);
    let mut uc = UnicodeCmap::new(Name(b"Custom"), system);
    for (&g, &c) in &to_unicode {
        uc.pair(g, c);
    }
    pdf.cmap(cmap, &uc.finish());
    let sub = subsetter::subset(FONT, 0, &remapper).map_err(|e| format!("font subset: {e}"))?;
    pdf.stream(file, &deflate(&sub)).filter(Filter::FlateDecode);

    for (g, (page, content)) in sheets.iter().zip(&pages) {
        let mut p = pdf.page(*page);
        p.media_box(Rect::new(
            0.0,
            0.0,
            (g.width * PT) as f32,
            (g.height * PT) as f32,
        ));
        p.parent(tree);
        p.contents(*content);
        let mut res = p.resources();
        res.fonts().pair(Name(b"F1"), font);
        let mut props = res.insert(Name(b"Properties")).dict();
        for (i, (_, r)) in layers.iter().enumerate() {
            props.pair(Name(format!("L{i}").as_bytes()), *r);
        }
        props.finish();
        res.finish();
        p.finish();

        let data = page_content(g, standard, options, &layers, &face, &remapper);
        pdf.stream(*content, &deflate(&data))
            .filter(Filter::FlateDecode);
    }
    Ok(pdf.finish())
}

fn page_content(
    g: &Graphics,
    standard: &Standard,
    options: &PdfOptions,
    layers: &[(String, Ref)],
    face: &ttf_parser::Face,
    remapper: &subsetter::GlyphRemapper,
) -> Vec<u8> {
    let mut c = Content::new();
    // Work in millimetres.
    c.transform([PT as f32, 0.0, 0.0, PT as f32, 0.0, 0.0]);
    c.set_line_cap(LineCapStyle::RoundCap);
    c.set_line_join(LineJoinStyle::RoundJoin);
    let mut layer: Option<&str> = None;
    for item in &g.items {
        if layer != Some(item.layer.as_str()) {
            if layer.is_some() {
                c.end_marked_content();
            }
            let i = layers
                .iter()
                .position(|(n, _)| *n == item.layer)
                .unwrap_or(0);
            c.begin_marked_content_with_properties(Name(b"OC"))
                .properties_named(Name(format!("L{i}").as_bytes()));
            layer = Some(&item.layer);
        }
        let [r, gr, b] = if !options.monochrome {
            item.pen.color
        } else if matches!(item.prim, Prim::Fill { .. }) {
            // Fills keep their lightness so shading stays readable.
            let [r, g, b] = item.pen.color;
            let l = (0.299 * f64::from(r) + 0.587 * f64::from(g) + 0.114 * f64::from(b)) as u8;
            [l, l, l]
        } else {
            [0, 0, 0]
        };
        let rgb = |v: u8| f32::from(v) / 255.0;
        c.set_stroke_rgb(rgb(r), rgb(gr), rgb(b));
        c.set_fill_rgb(rgb(r), rgb(gr), rgb(b));
        c.set_line_width(item.pen.weight as f32);
        let dashes = standard.dashes(item.pen.style);
        if item.pen.style == LineStyle::Continuous {
            c.set_dash_pattern([], 0.0);
        } else {
            // Dash lengths are for a 0.25 mm line; scale with the weight.
            let k = (item.pen.weight / 0.25).max(0.5);
            c.set_dash_pattern(dashes.iter().map(|d| (d * k) as f32), 0.0);
        }
        match &item.prim {
            Prim::Line { a, b } => {
                c.move_to(a[0] as f32, a[1] as f32);
                c.line_to(b[0] as f32, b[1] as f32);
                c.stroke();
            }
            Prim::Arc {
                center,
                radius,
                start,
                end,
            } => {
                arc_path(&mut c, *center, *radius, *start, *end);
                c.stroke();
            }
            Prim::Polyline { points, closed } => {
                if let Some(p0) = points.first() {
                    c.move_to(p0[0] as f32, p0[1] as f32);
                    for p in &points[1..] {
                        c.line_to(p[0] as f32, p[1] as f32);
                    }
                    if *closed {
                        c.close_path();
                    }
                    c.stroke();
                }
            }
            Prim::Fill { loops } => {
                for l in loops {
                    if let Some(p0) = l.first() {
                        c.move_to(p0[0] as f32, p0[1] as f32);
                        for p in &l[1..] {
                            c.line_to(p[0] as f32, p[1] as f32);
                        }
                        c.close_path();
                    }
                }
                // A stroke in the fill colour closes hairline seams between
                // adjacent fills (shaded faces).
                if item.pen.weight > 0.0 {
                    c.fill_even_odd_and_stroke();
                } else {
                    c.fill_even_odd();
                }
            }
            Prim::Text {
                at,
                height,
                angle,
                text,
                ..
            } => {
                let mut glyphs = Vec::with_capacity(text.len() * 2);
                for ch in text.chars() {
                    let gid = face
                        .glyph_index(ch)
                        .and_then(|g| remapper.get(g.0))
                        .unwrap_or(0);
                    glyphs.extend_from_slice(&gid.to_be_bytes());
                }
                let (s, co) = angle.sin_cos();
                c.begin_text();
                c.set_font(Name(b"F1"), *height as f32);
                c.set_text_matrix([
                    co as f32,
                    s as f32,
                    -s as f32,
                    co as f32,
                    at[0] as f32,
                    at[1] as f32,
                ]);
                c.show(Str(&glyphs));
                c.end_text();
            }
        }
    }
    if layer.is_some() {
        c.end_marked_content();
    }
    c.finish().into_vec()
}

/// A counter-clockwise arc as cubic Béziers of at most 90° each.
fn arc_path(c: &mut Content, center: P2, r: f64, start: f64, end: f64) {
    let sweep = end - start;
    let n = ((sweep.abs() / std::f64::consts::FRAC_PI_2).ceil() as usize).max(1);
    let step = sweep / n as f64;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    let pt = |a: f64| [center[0] + r * a.cos(), center[1] + r * a.sin()];
    let p0 = pt(start);
    c.move_to(p0[0] as f32, p0[1] as f32);
    for i in 0..n {
        let a0 = start + step * i as f64;
        let a1 = a0 + step;
        let (p, q) = (pt(a0), pt(a1));
        let c1 = [p[0] - k * r * a0.sin(), p[1] + k * r * a0.cos()];
        let c2 = [q[0] + k * r * a1.sin(), q[1] - k * r * a1.cos()];
        c.cubic_to(
            c1[0] as f32,
            c1[1] as f32,
            c2[0] as f32,
            c2[1] as f32,
            q[0] as f32,
            q[1] as f32,
        );
    }
}

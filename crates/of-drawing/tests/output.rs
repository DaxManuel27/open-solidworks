//! PDF and DXF output of a sheet with every primitive kind, checked by
//! parsing the files back. Set `OPENFORGE_OUTPUT_DIR` to keep the files for
//! inspection in other viewers.

use of_drawing::graphics::{Graphics, HAlign, LineStyle, Owner, Pen, Prim, layers};
use of_drawing::standard::Standard;
use of_drawing::{dxf, pdf, text};

mod common;
use common::{entities, get, num, pairs};

fn sample() -> Graphics {
    let mut g = Graphics::new(420.0, 297.0);
    let owner = Owner::Format;
    let thick = Pen::new(LineStyle::Continuous, 0.5);
    let thin = Pen::new(LineStyle::Continuous, 0.25);
    g.rect([10.0, 10.0], [410.0, 287.0], thick, layers::FORMAT, &owner);
    g.line(
        [50.0, 150.0],
        [150.0, 150.0],
        thick,
        layers::VISIBLE,
        &owner,
    );
    g.line(
        [50.0, 140.0],
        [150.0, 140.0],
        Pen::new(LineStyle::Hidden, 0.25),
        layers::HIDDEN,
        &owner,
    );
    g.line(
        [50.0, 130.0],
        [150.0, 130.0],
        Pen::new(LineStyle::Center, 0.25),
        layers::CENTER,
        &owner,
    );
    g.push(
        Prim::Arc {
            center: [200.0, 150.0],
            radius: 20.0,
            start: 0.0,
            end: std::f64::consts::TAU,
        },
        thick,
        layers::VISIBLE,
        &owner,
    );
    g.push(
        Prim::Arc {
            center: [260.0, 150.0],
            radius: 20.0,
            start: 0.5,
            end: 2.5,
        },
        thick,
        layers::VISIBLE,
        &owner,
    );
    g.push(
        Prim::Polyline {
            points: vec![[300.0, 130.0], [340.0, 170.0], [380.0, 130.0]],
            closed: true,
        },
        thin,
        layers::VISIBLE,
        &owner,
    );
    g.push(
        Prim::Fill {
            loops: vec![vec![[50.0, 100.0], [56.0, 102.0], [56.0, 98.0]]],
        },
        thin,
        layers::DIMENSIONS,
        &owner,
    );
    for p in text::layout(
        "⌀6.6 THRU ⌴⌀11 ↧6.4\nPART NO. 1234 ±0.1 45°",
        [50.0, 60.0],
        3.5,
        0.0,
        HAlign::Left,
    ) {
        g.push(p, thin, layers::NOTES, &Owner::Annotation { id: 1 });
    }
    for p in text::layout(
        "VERTICAL",
        [30.0, 60.0],
        5.0,
        std::f64::consts::FRAC_PI_2,
        HAlign::Left,
    ) {
        g.push(p, thin, layers::NOTES, &Owner::Annotation { id: 2 });
    }
    g
}

fn keep(name: &str, bytes: &[u8]) {
    if let Ok(dir) = std::env::var("OPENFORGE_OUTPUT_DIR") {
        std::fs::write(std::path::Path::new(&dir).join(name), bytes).unwrap();
    }
}

#[test]
fn pdf_has_one_page_per_sheet_an_embedded_font_and_layers() {
    let g = sample();
    let mut g2 = sample();
    g2.width = 297.0;
    g2.height = 210.0;
    let bytes = pdf::write(&[g, g2], &Standard::iso(), &pdf::PdfOptions::default()).unwrap();
    keep("sample.pdf", &bytes);
    let s = String::from_utf8_lossy(&bytes);
    assert!(s.starts_with("%PDF-"));
    let pages = s.matches("/Type /Page").count() - s.matches("/Type /Pages").count();
    assert_eq!(pages, 2);
    // A3 at true size: 420 x 297 mm in points.
    assert!(
        s.contains("/MediaBox [0 0 1190.5511 841.8898]"),
        "A3 at true size"
    );
    assert!(s.contains("/FontFile2"));
    assert!(s.contains("/ToUnicode"));
    assert!(s.contains("/Identity-H"));
    for l in ["FORMAT", "VISIBLE", "HIDDEN", "CENTER", "NOTES"] {
        assert!(s.contains(&format!("({l})")), "layer {l}");
    }
    // The subset holds only the glyphs used, so it is far smaller than the
    // 680 kB font.
    assert!(bytes.len() < 80_000, "{}", bytes.len());
}

#[test]
fn dxf_round_trips_entities_layers_and_line_types() {
    let g = sample();
    let s = dxf::write(&[g], &Standard::iso()).unwrap();
    keep("sample.dxf", s.as_bytes());
    let pairs: Vec<(i32, &str)> = s
        .lines()
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|c| (c[0].trim().parse().unwrap(), c[1]))
        .collect();
    let count = |kind: &str| pairs.iter().filter(|p| p.0 == 0 && p.1 == kind).count();
    // Three plain lines; the two ⌀ slashes and the ↧ shaft and base.
    assert_eq!(count("LINE"), 3 + 4);
    assert_eq!(count("CIRCLE"), 3, "one circle and the two ⌀ symbols");
    assert_eq!(count("ARC"), 1);
    assert!(count("POLYLINE") >= 4);
    assert_eq!(count("SOLID"), 1);
    assert_eq!(count("TEXT"), 5);
    assert_eq!(pairs.last(), Some(&(0, "EOF")));
    for lt in ["HIDDEN", "CENTER", "PHANTOM"] {
        assert!(
            pairs.iter().any(|p| p.0 == 2 && p.1 == lt),
            "line type {lt}"
        );
    }
    assert!(pairs.contains(&(1, r"PART NO. 1234 \U+00B10.1 45\U+00B0")));
    // Text heights are capital heights: 3.5 mm.
    let heights: Vec<f64> = pairs
        .windows(2)
        .filter(|w| w[0].0 == 40)
        .map(|w| w[0].1.parse::<f64>().unwrap())
        .collect();
    assert!(
        heights.iter().any(|h| (h - 3.5).abs() < 1e-6),
        "{heights:?}"
    );
}

#[test]
fn dxf_2018_puts_views_in_model_space_at_true_size() {
    // A sheet at 1:2: a view line 100 mm long on paper is 200 mm of model.
    let mut g = Graphics::new(420.0, 297.0);
    let view = Owner::View {
        view: 2,
        entity: None,
    };
    g.rect(
        [10.0, 10.0],
        [410.0, 287.0],
        Pen::new(LineStyle::Continuous, 0.5),
        layers::FORMAT,
        &Owner::Format,
    );
    g.line(
        [50.0, 100.0],
        [150.0, 100.0],
        Pen::new(LineStyle::Continuous, 0.5),
        layers::VISIBLE,
        &view,
    );
    let mut red = Pen::new(LineStyle::Hidden, 0.25);
    red.color = [200, 30, 30];
    g.line([50.0, 90.0], [150.0, 90.0], red, layers::HIDDEN, &view);
    for p in text::layout("⌀20 H7", [60.0, 120.0], 3.5, 0.0, HAlign::Left) {
        g.push(
            p,
            Pen::new(LineStyle::Continuous, 0.25),
            layers::DIMENSIONS,
            &Owner::Annotation { id: 7 },
        );
    }
    for p in text::layout("NOTES", [20.0, 40.0], 3.5, 0.0, HAlign::Left) {
        g.push(
            p,
            Pen::new(LineStyle::Continuous, 0.25),
            layers::NOTES,
            &Owner::Annotation { id: 8 },
        );
    }
    let second = Graphics::new(297.0, 210.0);
    let sheets = [
        dxf::DxfSheet {
            graphics: &g,
            name: "Plate".into(),
            scale: 0.5,
            view_annotations: vec![7],
        },
        dxf::DxfSheet {
            graphics: &second,
            name: "Parts".into(),
            scale: 1.0,
            view_annotations: Vec::new(),
        },
    ];
    let text = dxf::write_version(&sheets, &Standard::iso(), dxf::DxfVersion::R2018).unwrap();
    keep("sheet-2018.dxf", text.as_bytes());
    let p = pairs(&text);
    assert!(
        p.windows(2)
            .any(|w| w[0].1 == "$ACADVER" && w[1].1 == "AC1032")
    );
    // Model space: the view lines at twice their paper length, weights and
    // colour kept.
    let lines = entities(&p, "LINE");
    let model: Vec<_> = lines
        .iter()
        .filter(|e| get(e, 67).is_none() && get(e, 8) != Some("DIMENSIONS"))
        .collect();
    assert_eq!(model.len(), 2);
    for e in &model {
        assert_eq!(num(e, 11) - num(e, 10), 200.0);
    }
    assert_eq!(get(model[0], 370), Some("50"));
    assert_eq!(get(model[1], 370), Some("25"));
    assert_eq!(get(model[1], 6), Some("HIDDEN"));
    assert_eq!(
        get(model[1], 420),
        Some(&*format!("{}", (200 << 16) | (30 << 8) | 30))
    );
    // Text attached to a view is in model space (doubled), sheet notes
    // are on paper; text is UTF-8.
    let texts = entities(&p, "TEXT");
    let note = texts.iter().find(|e| get(e, 1) == Some("NOTES")).unwrap();
    assert_eq!(get(note, 67), Some("1"));
    let dim = texts.iter().find(|e| get(e, 1) == Some("20 H7")).unwrap();
    assert!(get(dim, 67).is_none());
    assert!((num(dim, 40) - 7.0).abs() < 1e-3, "{}", num(dim, 40));
    // The format is on paper at 1:1.
    let border = entities(&p, "LWPOLYLINE")
        .into_iter()
        .find(|e| get(e, 67) == Some("1"))
        .unwrap();
    assert_eq!(num(border, 10), 10.0);
    // Two layouts named after the sheets, each with a viewport at its
    // scale: view height = paper height / scale.
    let dict = entities(&p, "DICTIONARY")
        .into_iter()
        .find(|e| get(e, 3) == Some("Model"))
        .unwrap();
    let names: Vec<&str> = dict
        .iter()
        .filter(|(c, _)| *c == 3)
        .map(|(_, v)| v.as_str())
        .collect();
    assert_eq!(names, ["Model", "Plate", "Parts"]);
    let vps: Vec<_> = entities(&p, "VIEWPORT")
        .into_iter()
        .filter(|e| get(e, 69) == Some("2"))
        .collect();
    assert_eq!(vps.len(), 2);
    // Inactive layouts keep their entities in their blocks, written first.
    let plate = vps.iter().find(|e| num(e, 45) == 594.0).unwrap();
    let parts = vps.iter().find(|e| num(e, 45) == 210.0).unwrap();
    assert_eq!(num(plate, 12), 420.0);
    // The second sheet sits beside the first in model space.
    assert_eq!(num(parts, 12), 840.0 + 50.0 + 148.5);
}

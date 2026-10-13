//! PDF and DXF output of a sheet with every primitive kind, checked by
//! parsing the files back. Set `OPENFORGE_OUTPUT_DIR` to keep the files for
//! inspection in other viewers.

use of_drawing::graphics::{Graphics, HAlign, LineStyle, Owner, Pen, Prim, layers};
use of_drawing::standard::Standard;
use of_drawing::{dxf, pdf, text};

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

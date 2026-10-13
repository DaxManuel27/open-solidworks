//! Text layout in the bundled drafting font (DejaVu Sans Condensed) and
//! drafting symbols drawn as strokes.
//!
//! Text height follows drafting practice: it is the height of capital
//! letters. Primitives carry the font's em size, so renderers need no font
//! metrics of their own. Drafting and GD&T symbols are not taken from the
//! font (no permissively licensed font has them all): they are drawn here
//! from strokes, so they look the same on screen, in PDF and in DXF.

use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::sync::OnceLock;

use crate::graphics::{HAlign, P2, Prim};

/// The bundled font file.
pub const FONT: &[u8] = include_bytes!("../assets/fonts/DejaVuSansCondensed.ttf");
/// Family name to use when the font is installed or loaded by name.
pub const FONT_FAMILY: &str = "DejaVu Sans Condensed";

struct Metrics {
    units: f64,
    cap: f64,
}

fn face() -> &'static ttf_parser::Face<'static> {
    static FACE: OnceLock<ttf_parser::Face<'static>> = OnceLock::new();
    FACE.get_or_init(|| ttf_parser::Face::parse(FONT, 0).expect("bundled font parses"))
}

fn metrics() -> &'static Metrics {
    static M: OnceLock<Metrics> = OnceLock::new();
    M.get_or_init(|| {
        let f = face();
        let units = f64::from(f.units_per_em());
        let cap = f
            .capital_height()
            .map(f64::from)
            .filter(|c| *c > 0.0)
            .unwrap_or(units * 0.729);
        Metrics { units, cap }
    })
}

/// Em size for a capital height.
pub fn em_for(cap_height: f64) -> f64 {
    let m = metrics();
    cap_height * m.units / m.cap
}

/// Capital height of text set at em size `em`.
pub fn cap_for(em: f64) -> f64 {
    let m = metrics();
    em * m.cap / m.units
}

/// Advance width of `text` set in the font at em size `em`.
pub fn advance(text: &str, em: f64) -> f64 {
    let f = face();
    let m = metrics();
    text.chars()
        .map(|c| {
            let g = f.glyph_index(c).or_else(|| f.glyph_index('?'));
            g.and_then(|g| f.glyph_hor_advance(g))
                .map(f64::from)
                .unwrap_or(m.units * 0.5)
        })
        .sum::<f64>()
        * em
        / m.units
}

/// Whether the font has a glyph for `c`.
pub fn has_glyph(c: char) -> bool {
    face().glyph_index(c).is_some()
}

/// A stroke of a symbol, in units of the capital height, origin at the
/// symbol's bottom-left.
enum Stroke {
    Line(P2, P2),
    Poly(Vec<P2>),
    Circle(P2, f64),
    Arc(P2, f64, f64, f64),
    /// A letter in the font, centered at a point, at a capital height.
    Letter(char, P2, f64),
}

/// Strokes and advance (in capital heights) of a drafting symbol.
fn symbol(c: char) -> Option<(Vec<Stroke>, f64)> {
    use Stroke::*;
    let circled = |l: char| {
        (
            vec![Circle([0.55, 0.5], 0.55), Letter(l, [0.55, 0.5], 0.6)],
            1.2,
        )
    };
    let arrow = |x: f64, y0: f64, y1: f64| {
        // A 45° arrow from (x, y0) up-right, head at the end.
        let (a, b) = ([x, y0], [x + (y1 - y0), y1]);
        vec![
            Line(a, b),
            Poly(vec![[b[0] - 0.3, b[1]], b, [b[0], b[1] - 0.3]]),
        ]
    };
    Some(match c {
        // Diameter.
        '⌀' => (
            vec![Circle([0.45, 0.5], 0.4), Line([0.05, 0.0], [0.85, 1.0])],
            0.95,
        ),
        // Counterbore.
        '⌴' => (
            vec![Poly(vec![
                [0.05, 0.9],
                [0.05, 0.1],
                [0.85, 0.1],
                [0.85, 0.9],
            ])],
            1.0,
        ),
        // Countersink.
        '⌵' => (vec![Poly(vec![[0.05, 0.9], [0.45, 0.1], [0.85, 0.9]])], 1.0),
        // Depth.
        '↧' => (
            vec![
                Line([0.45, 1.0], [0.45, 0.05]),
                Poly(vec![[0.2, 0.35], [0.45, 0.05], [0.7, 0.35]]),
                Line([0.1, 0.0], [0.8, 0.0]),
            ],
            0.95,
        ),
        // Square.
        '□' => (
            vec![Poly(vec![
                [0.05, 0.0],
                [0.95, 0.0],
                [0.95, 0.9],
                [0.05, 0.9],
                [0.05, 0.0],
            ])],
            1.1,
        ),
        // Straightness.
        '⏤' => (vec![Line([0.05, 0.5], [1.15, 0.5])], 1.2),
        // Flatness.
        '⏥' => (
            vec![Poly(vec![
                [0.05, 0.15],
                [0.85, 0.15],
                [1.15, 0.85],
                [0.35, 0.85],
                [0.05, 0.15],
            ])],
            1.2,
        ),
        // Circularity.
        '○' => (vec![Circle([0.55, 0.5], 0.5)], 1.1),
        // Cylindricity.
        '⌭' => (
            vec![
                Circle([0.6, 0.5], 0.4),
                Line([0.05, 0.0], [0.55, 1.0]),
                Line([0.65, 0.0], [1.15, 1.0]),
            ],
            1.2,
        ),
        // Profile of a line; also the arc length mark.
        '⌒' | '⌢' => (vec![Arc([0.6, 0.15], 0.55, 0.0, PI)], 1.2),
        // Profile of a surface.
        '⌓' => (
            vec![
                Arc([0.6, 0.15], 0.55, 0.0, PI),
                Line([0.05, 0.15], [1.15, 0.15]),
            ],
            1.2,
        ),
        // Angularity.
        '∠' => (vec![Poly(vec![[1.1, 0.0], [0.05, 0.0], [0.95, 0.75]])], 1.2),
        // Perpendicularity.
        '⟂' => (
            vec![
                Line([0.55, 0.0], [0.55, 1.0]),
                Line([0.05, 0.0], [1.05, 0.0]),
            ],
            1.1,
        ),
        // Parallelism.
        '∥' => (
            vec![
                Line([0.05, 0.0], [0.45, 1.0]),
                Line([0.45, 0.0], [0.85, 1.0]),
            ],
            0.95,
        ),
        // Position.
        '⌖' => (
            vec![
                Circle([0.6, 0.5], 0.35),
                Line([0.6, -0.05], [0.6, 1.05]),
                Line([0.05, 0.5], [1.15, 0.5]),
            ],
            1.2,
        ),
        // Concentricity.
        '◎' => (
            vec![Circle([0.55, 0.5], 0.5), Circle([0.55, 0.5], 0.25)],
            1.1,
        ),
        // Symmetry.
        '⌯' => (
            vec![
                Line([0.3, 0.2], [0.9, 0.2]),
                Line([0.05, 0.5], [1.15, 0.5]),
                Line([0.3, 0.8], [0.9, 0.8]),
            ],
            1.2,
        ),
        // Circular runout.
        '↗' => (arrow(0.1, 0.05, 0.95), 1.1),
        // Total runout.
        '⌰' => {
            let mut s = arrow(0.05, 0.05, 0.95);
            s.extend(arrow(0.5, 0.05, 0.95));
            s.push(Line([0.0, 0.05], [0.5, 0.05]));
            (s, 1.5)
        }
        // Conical taper.
        '⌲' => (
            vec![
                Line([0.05, 0.5], [1.15, 0.5]),
                Poly(vec![[0.05, 0.85], [1.15, 0.6], [1.15, 0.4], [0.05, 0.15]]),
            ],
            1.2,
        ),
        // Slope.
        '⌳' => (
            vec![Poly(vec![
                [0.05, 0.0],
                [1.15, 0.0],
                [0.05, 0.7],
                [0.05, 0.0],
            ])],
            1.2,
        ),
        // Circled modifiers: maximum and least material, projected zone,
        // free state, tangent plane, regardless of feature size, unequal.
        'Ⓜ' => circled('M'),
        'Ⓛ' => circled('L'),
        'Ⓟ' => circled('P'),
        'Ⓕ' => circled('F'),
        'Ⓣ' => circled('T'),
        'Ⓢ' => circled('S'),
        'Ⓤ' => circled('U'),
        _ => return None,
    })
}

/// Whether `c` is drawn as a drafting symbol rather than from the font.
pub fn is_symbol(c: char) -> bool {
    symbol(c).is_some()
}

/// Width of one line of text at capital height `h`, with symbols.
pub fn line_width(text: &str, h: f64) -> f64 {
    let em = em_for(h);
    let mut w = 0.0;
    let mut run = String::new();
    for c in text.chars() {
        if let Some((_, adv)) = symbol(c) {
            w += advance(&run, em);
            run.clear();
            w += adv * h;
        } else {
            run.push(c);
        }
    }
    w + advance(&run, em)
}

/// Line pitch for multi-line text, in capital heights.
pub const LINE_PITCH: f64 = 1.6;

/// Size of a laid-out block: width of the longest line and total height
/// from the top of the first line's capitals to the last baseline.
pub fn block_size(text: &str, h: f64) -> (f64, f64) {
    let lines: Vec<&str> = text.split('\n').collect();
    let w = lines.iter().map(|l| line_width(l, h)).fold(0.0, f64::max);
    (w, h + (lines.len() as f64 - 1.0) * h * LINE_PITCH)
}

/// Lay out text with its first line's baseline starting at `at`, rotated by
/// `angle`, each line aligned by `align` (relative to `at`). Further lines go
/// down by [`LINE_PITCH`].
pub fn layout(text: &str, at: P2, h: f64, angle: f64, align: HAlign) -> Vec<Prim> {
    let (s, c) = angle.sin_cos();
    let place = |x: f64, y: f64| [at[0] + x * c - y * s, at[1] + x * s + y * c];
    let em = em_for(h);
    let mut out = Vec::new();
    for (li, line) in text.split('\n').enumerate() {
        let y0 = -(li as f64) * h * LINE_PITCH;
        let width = line_width(line, h);
        let mut x = match align {
            HAlign::Left => 0.0,
            HAlign::Center => -width / 2.0,
            HAlign::Right => -width,
        };
        let mut run = String::new();
        let mut run_x = x;
        let flush = |run: &mut String, run_x: f64, out: &mut Vec<Prim>| {
            if run.trim().is_empty() {
                run.clear();
                return;
            }
            let text: String = run
                .chars()
                .map(|ch| if has_glyph(ch) { ch } else { '?' })
                .collect();
            out.push(Prim::Text {
                at: place(run_x, y0),
                height: em,
                angle,
                width: advance(&text, em),
                text,
            });
            run.clear();
        };
        for ch in line.chars() {
            if let Some((strokes, adv)) = symbol(ch) {
                flush(&mut run, run_x, &mut out);
                for st in strokes {
                    let p = |q: P2| place(x + q[0] * h, y0 + q[1] * h);
                    match st {
                        Stroke::Line(a, b) => out.push(Prim::Line { a: p(a), b: p(b) }),
                        Stroke::Poly(pts) => out.push(Prim::Polyline {
                            points: pts.into_iter().map(p).collect(),
                            closed: false,
                        }),
                        Stroke::Circle(ctr, r) => out.push(Prim::Arc {
                            center: p(ctr),
                            radius: r * h,
                            start: 0.0,
                            end: TAU,
                        }),
                        Stroke::Arc(ctr, r, a0, a1) => out.push(Prim::Arc {
                            center: p(ctr),
                            radius: r * h,
                            start: a0 + angle,
                            end: a1 + angle,
                        }),
                        Stroke::Letter(l, ctr, lh) => {
                            let s = l.to_string();
                            let lem = em_for(lh * h);
                            let lw = advance(&s, lem);
                            let cx = x + ctr[0] * h - lw / 2.0;
                            let cy = y0 + ctr[1] * h - lh * h / 2.0;
                            out.push(Prim::Text {
                                at: place(cx, cy),
                                height: lem,
                                angle,
                                width: lw,
                                text: s,
                            });
                        }
                    }
                }
                x += adv * h;
                run_x = x;
            } else {
                run.push(ch);
                x = run_x + advance(&run, em);
            }
        }
        flush(&mut run, run_x, &mut out);
    }
    out
}

/// Rotation that keeps text readable (never upside down) along a direction.
pub fn readable(angle: f64) -> f64 {
    let a = angle.rem_euclid(TAU);
    if a > FRAC_PI_2 + 1e-9 && a <= 3.0 * FRAC_PI_2 + 1e-9 {
        a - PI
    } else {
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capital_height_sets_the_em_size() {
        let em = em_for(3.5);
        assert!(em > 3.5 && em < 6.0, "{em}");
        assert!(advance("M", em) > 2.0);
        assert!(advance("MM", em) > advance("M", em));
    }

    #[test]
    fn symbols_are_strokes_and_text_is_runs() {
        let prims = layout("⌀6.6 THRU", [0.0, 0.0], 3.5, 0.0, HAlign::Left);
        let texts: Vec<_> = prims
            .iter()
            .filter_map(|p| match p {
                Prim::Text { text, at, .. } => Some((text.clone(), *at)),
                _ => None,
            })
            .collect();
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0].0, "6.6 THRU");
        assert!((texts[0].1[0] - 0.95 * 3.5).abs() < 1e-9);
        assert!(prims.iter().any(|p| matches!(p, Prim::Arc { .. })));
        let w = line_width("⌀6.6 THRU", 3.5);
        assert!((w - (0.95 * 3.5 + advance("6.6 THRU", em_for(3.5)))).abs() < 1e-9);
    }

    #[test]
    fn centered_and_multi_line_layout() {
        let prims = layout("AB\nC", [10.0, 20.0], 5.0, 0.0, HAlign::Center);
        let Prim::Text { at, width, .. } = &prims[0] else {
            panic!()
        };
        assert!((at[0] + width / 2.0 - 10.0).abs() < 1e-9);
        let Prim::Text { at, .. } = &prims[1] else {
            panic!()
        };
        assert!((at[1] - (20.0 - 5.0 * LINE_PITCH)).abs() < 1e-9);
    }

    #[test]
    fn readable_angles_never_point_left() {
        assert!((readable(PI) - 0.0).abs() < 1e-9);
        assert!((readable(FRAC_PI_2) - FRAC_PI_2).abs() < 1e-9);
        assert!((readable(-FRAC_PI_2) - (-FRAC_PI_2).rem_euclid(TAU) + PI).abs() < 1e-9);
    }
}

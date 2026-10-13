//! Hatch patterns for cut faces and area hatches: families of parallel
//! lines clipped to regions with the even-odd rule.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::graphics::P2;

/// One family of parallel lines. Lengths are mm of paper.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Family {
    /// Direction, radians from paper x.
    pub angle: f64,
    pub spacing: f64,
    /// Shift of the family across its lines, as a fraction of the spacing.
    pub offset: f64,
}

/// A named hatch pattern from the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Pattern {
    /// Single 45° lines (ISO 128-50 general, ANSI31 iron).
    #[default]
    General,
    /// Pairs of 45° lines (ANSI32 steel).
    Steel,
    /// 45° lines with a wider second line spacing (ANSI33 bronze, brass, copper).
    Bronze,
    /// Alternating long and short spacing (ANSI38 aluminium).
    Aluminium,
    /// Cross hatch (ANSI37 plastic, rubber).
    Plastic,
    /// Dense cross hatch (insulation, glass).
    Insulation,
    /// A solid fill.
    Solid,
}

impl Pattern {
    /// The pattern for a material name (by keyword, case-insensitive).
    pub fn for_material(name: Option<&str>) -> Pattern {
        let n = name.unwrap_or_default().to_ascii_lowercase();
        let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
        if has(&["steel", "iron", "stainless"]) {
            Pattern::Steel
        } else if has(&["alumin"]) {
            Pattern::Aluminium
        } else if has(&["bronze", "brass", "copper"]) {
            Pattern::Bronze
        } else if has(&[
            "abs", "nylon", "plastic", "acetal", "delrin", "rubber", "polyeth", "polyprop", "pvc",
            "resin",
        ]) {
            Pattern::Plastic
        } else if has(&["glass", "insul", "foam"]) {
            Pattern::Insulation
        } else {
            Pattern::General
        }
    }

    /// Line families at scale 1 and angle 0 (patterns add their own angles).
    pub fn families(&self) -> Vec<Family> {
        use std::f64::consts::FRAC_PI_4 as Q;
        let f = |angle: f64, spacing: f64, offset: f64| Family {
            angle,
            spacing,
            offset,
        };
        match self {
            Pattern::General => vec![f(Q, 3.0, 0.0)],
            Pattern::Steel => vec![f(Q, 6.0, 0.0), f(Q, 6.0, 0.25)],
            Pattern::Bronze => vec![f(Q, 6.0, 0.0), f(Q, 6.0, 0.5)],
            Pattern::Aluminium => vec![f(Q, 6.0, 0.0), f(Q, 6.0, 0.2)],
            Pattern::Plastic => vec![f(Q, 4.0, 0.0), f(-Q, 4.0, 0.0)],
            Pattern::Insulation => vec![f(Q, 2.0, 0.0), f(-Q, 2.0, 0.0)],
            Pattern::Solid => Vec::new(),
        }
    }
}

/// Hatch lines covering `loops` (even-odd), at `scale` times the pattern's
/// spacing and turned by `angle`. All lengths in the loops' units.
pub fn lines(loops: &[Vec<P2>], pattern: Pattern, scale: f64, angle: f64) -> Vec<[P2; 2]> {
    let mut out = Vec::new();
    for fam in pattern.families() {
        let a = fam.angle + angle;
        let (s, c) = a.sin_cos();
        let dir = [c, s];
        let normal = [-s, c];
        let spacing = (fam.spacing * scale).max(1e-6);
        // Range of the loops across the lines.
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for p in loops.iter().flatten() {
            let v = p[0] * normal[0] + p[1] * normal[1];
            lo = lo.min(v);
            hi = hi.max(v);
        }
        if lo > hi || (hi - lo) / spacing > 20_000.0 {
            continue;
        }
        let first = ((lo / spacing) - fam.offset).floor() as i64;
        let last = ((hi / spacing) - fam.offset).ceil() as i64;
        for k in first..=last {
            let v = (k as f64 + fam.offset) * spacing;
            // Crossings of the line {p·normal = v} with every loop edge,
            // as positions along `dir`.
            let mut ts: Vec<f64> = Vec::new();
            for l in loops {
                let n = l.len();
                for i in 0..n {
                    let (p, q) = (l[i], l[(i + 1) % n]);
                    let vp = p[0] * normal[0] + p[1] * normal[1];
                    let vq = q[0] * normal[0] + q[1] * normal[1];
                    if (vp > v) != (vq > v) {
                        let t = (v - vp) / (vq - vp);
                        let x = [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
                        ts.push(x[0] * dir[0] + x[1] * dir[1]);
                    }
                }
            }
            ts.sort_by(f64::total_cmp);
            for pair in ts.chunks_exact(2) {
                let at = |t: f64| [dir[0] * t + normal[0] * v, dir[1] * t + normal[1] * v];
                if pair[1] - pair[0] > 1e-9 {
                    out.push([at(pair[0]), at(pair[1])]);
                }
            }
        }
    }
    out
}

/// Chain 3D-projected edge samples into closed loops: pieces whose ends
/// meet (within `tol`) are joined; open leftovers are dropped.
pub fn chain(mut pieces: Vec<Vec<P2>>, tol: f64) -> Vec<Vec<P2>> {
    let close = |a: P2, b: P2| (a[0] - b[0]).abs() <= tol && (a[1] - b[1]).abs() <= tol;
    let mut loops = Vec::new();
    pieces.retain(|p| p.len() >= 2);
    while let Some(mut cur) = pieces.pop() {
        loop {
            let (s, e) = (cur[0], *cur.last().unwrap());
            if cur.len() > 2 && close(s, e) {
                cur.pop();
                loops.push(cur);
                break;
            }
            let next = pieces.iter().position(|p| {
                let (ps, pe) = (p[0], *p.last().unwrap());
                close(e, ps) || close(e, pe)
            });
            match next {
                Some(i) => {
                    let mut p = pieces.swap_remove(i);
                    if !close(e, p[0]) {
                        p.reverse();
                    }
                    cur.extend(p.into_iter().skip(1));
                }
                None => break,
            }
        }
    }
    loops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hatch_fills_a_square_with_a_hole() {
        let outer = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let hole = vec![[4.0, 4.0], [6.0, 4.0], [6.0, 6.0], [4.0, 6.0]];
        let l = lines(std::slice::from_ref(&outer), Pattern::General, 1.0, 0.0);
        assert!(!l.is_empty());
        let total: f64 = l
            .iter()
            .map(|[a, b]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt())
            .sum();
        // Area / spacing.
        assert!((total - 100.0 / 3.0).abs() < 4.0, "{total}");
        let with_hole = lines(&[outer, hole], Pattern::General, 1.0, 0.0);
        for [a, b] in &with_hole {
            let m = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            assert!(!(m[0] > 4.0 && m[0] < 6.0 && m[1] > 4.0 && m[1] < 6.0));
        }
    }

    #[test]
    fn materials_pick_patterns() {
        assert_eq!(
            Pattern::for_material(Some("Steel AISI 1020")),
            Pattern::Steel
        );
        assert_eq!(
            Pattern::for_material(Some("Aluminum 6061")),
            Pattern::Aluminium
        );
        assert_eq!(Pattern::for_material(None), Pattern::General);
        assert_eq!(Pattern::Plastic.families().len(), 2);
    }

    #[test]
    fn pieces_chain_into_loops() {
        let pieces = vec![
            vec![[0.0, 0.0], [1.0, 0.0]],
            vec![[1.0, 1.0], [1.0, 0.0]],
            vec![[0.0, 1.0], [1.0, 1.0]],
            vec![[0.0, 1.0], [0.0, 0.0]],
        ];
        let loops = chain(pieces, 1e-9);
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].len(), 4);
    }
}

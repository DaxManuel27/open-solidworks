//! ISO 286-2 limits and fits: standard tolerance grades IT5–IT12 and the
//! fundamental deviations of the common hole and shaft positions, for nominal
//! sizes up to 500 mm.
//!
//! Supported positions: holes D E F G H JS K M N P, shafts d e f g h js k m
//! n p. Others report an error rather than a wrong value.

/// Upper bounds (mm) of the nominal size ranges.
const RANGES: [f64; 13] = [
    3.0, 6.0, 10.0, 18.0, 30.0, 50.0, 80.0, 120.0, 180.0, 250.0, 315.0, 400.0, 500.0,
];

/// Standard tolerance values in µm, grades IT5..=IT12, per range.
const IT: [[i32; 13]; 8] = [
    [4, 5, 6, 8, 9, 11, 13, 15, 18, 20, 23, 25, 27],
    [6, 8, 9, 11, 13, 16, 19, 22, 25, 29, 32, 36, 40],
    [10, 12, 15, 18, 21, 25, 30, 35, 40, 46, 52, 57, 63],
    [14, 18, 22, 27, 33, 39, 46, 54, 63, 72, 81, 89, 97],
    [25, 30, 36, 43, 52, 62, 74, 87, 100, 115, 130, 140, 155],
    [40, 48, 58, 70, 84, 100, 120, 140, 160, 185, 210, 230, 250],
    [60, 75, 90, 110, 130, 160, 190, 220, 250, 290, 320, 360, 400],
    [
        100, 120, 150, 180, 210, 250, 300, 350, 400, 460, 520, 570, 630,
    ],
];

/// Upper deviations `es` (µm) of shafts d..h.
const ES: [(&str, [i32; 13]); 5] = [
    (
        "d",
        [
            -20, -30, -40, -50, -65, -80, -100, -120, -145, -170, -190, -210, -230,
        ],
    ),
    (
        "e",
        [
            -14, -20, -25, -32, -40, -50, -60, -72, -85, -100, -110, -125, -135,
        ],
    ),
    (
        "f",
        [
            -6, -10, -13, -16, -20, -25, -30, -36, -43, -50, -56, -62, -68,
        ],
    ),
    (
        "g",
        [-2, -4, -5, -6, -7, -9, -10, -12, -14, -15, -17, -18, -20],
    ),
    ("h", [0; 13]),
];

/// Lower deviations `ei` (µm) of shafts k..p (k for grades 4 to 7).
const EI: [(&str, [i32; 13]); 4] = [
    ("k", [0, 1, 1, 1, 2, 2, 2, 3, 3, 4, 4, 4, 5]),
    ("m", [2, 4, 6, 7, 8, 9, 11, 13, 15, 17, 20, 21, 23]),
    ("n", [4, 8, 10, 12, 15, 17, 20, 23, 27, 31, 34, 37, 40]),
    ("p", [6, 12, 15, 18, 22, 26, 32, 37, 43, 50, 56, 62, 68]),
];

fn range(size: f64) -> Result<usize, String> {
    if !(size > 0.0 && size <= 500.0) {
        return Err(format!("ISO 286 fits cover sizes up to 500 mm, not {size}"));
    }
    Ok(RANGES.iter().position(|r| size <= *r).expect("checked"))
}

/// Standard tolerance IT`grade` in µm.
pub fn it(grade: u8, size: f64) -> Result<i32, String> {
    let r = range(size)?;
    if !(5..=12).contains(&grade) {
        return Err(format!("IT{grade} is not supported (IT5 to IT12)"));
    }
    Ok(IT[(grade - 5) as usize][r])
}

/// A tolerance class such as `H7` or `g6` split into position and grade.
pub fn parse(class: &str) -> Result<(String, u8), String> {
    let class = class.trim();
    let split = class
        .find(|c: char| c.is_ascii_digit())
        .ok_or_else(|| format!("{class:?} has no tolerance grade"))?;
    let (pos, grade) = class.split_at(split);
    let grade: u8 = grade
        .parse()
        .map_err(|_| format!("{class:?} has no valid tolerance grade"))?;
    if pos.is_empty() {
        return Err(format!("{class:?} has no fundamental deviation letter"));
    }
    Ok((pos.to_string(), grade))
}

/// Limit deviations (upper, lower) in mm of a tolerance class at a size.
/// Upper-case positions are holes, lower-case shafts.
pub fn deviations(class: &str, size: f64) -> Result<(f64, f64), String> {
    let (pos, grade) = parse(class)?;
    let r = range(size)?;
    let t = it(grade, size)?;
    let um = |v: i32| f64::from(v) / 1000.0;
    let hole = pos.chars().next().is_some_and(|c| c.is_ascii_uppercase());
    let lower = pos.to_ascii_lowercase();
    if lower == "js" {
        return Ok((um(t) / 2.0, -um(t) / 2.0));
    }
    let shaft_es = ES.iter().find(|(p, _)| *p == lower).map(|(_, v)| v[r]);
    let shaft_ei = EI.iter().find(|(p, _)| *p == lower).map(|(_, v)| {
        // k has no deviation above grade 7 or at 3 mm and below.
        if lower == "k" && !(4..=7).contains(&grade) {
            0
        } else {
            v[r]
        }
    });
    if !hole {
        return match (shaft_es, shaft_ei) {
            (Some(es), _) => Ok((um(es), um(es - t))),
            (_, Some(ei)) => Ok((um(ei + t), um(ei))),
            _ => Err(format!("shaft position {pos} is not supported")),
        };
    }
    // Holes: A–H mirror the shafts; K–P use the shaft value plus Δ.
    if let Some(es) = shaft_es {
        let ei = -es;
        return Ok((um(ei + t), um(ei)));
    }
    if let Some(ei) = shaft_ei {
        let delta = if grade > 5 && grade <= 8 && size > 3.0 {
            t - it(grade - 1, size)?
        } else {
            0
        };
        let es = match lower.as_str() {
            "k" if grade <= 8 => -ei + delta,
            "k" => 0,
            "m" if grade <= 8 => -ei + delta,
            "m" => -ei,
            "n" if grade <= 8 => -ei + delta,
            "n" => 0,
            _ if grade <= 7 => -ei + delta,
            _ => -ei,
        };
        return Ok((um(es), um(es - t)));
    }
    Err(format!("hole position {pos} is not supported"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: (f64, f64), b: (f64, f64)) {
        assert!(
            (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9,
            "{a:?} vs {b:?}"
        );
    }

    #[test]
    fn common_fits_match_iso_286_tables() {
        // ⌀20 H7/g6: hole +0.021/0, shaft −0.007/−0.020.
        close(deviations("H7", 20.0).unwrap(), (0.021, 0.0));
        close(deviations("g6", 20.0).unwrap(), (-0.007, -0.020));
        // ⌀50 H8/f7: hole +0.039/0, shaft −0.025/−0.050.
        close(deviations("H8", 50.0).unwrap(), (0.039, 0.0));
        close(deviations("f7", 50.0).unwrap(), (-0.025, -0.050));
        // ⌀30 k6: +0.015/+0.002; p6: +0.035/+0.022.
        close(deviations("k6", 30.0).unwrap(), (0.015, 0.002));
        close(deviations("p6", 30.0).unwrap(), (0.035, 0.022));
        // ⌀25 js7 ±0.0105.
        close(deviations("js7", 25.0).unwrap(), (0.0105, -0.0105));
        // Holes with Δ: ⌀40 K7 +0.007/−0.018, N7 −0.008/−0.033, P7 −0.017/−0.042.
        close(deviations("K7", 40.0).unwrap(), (0.007, -0.018));
        close(deviations("N7", 40.0).unwrap(), (-0.008, -0.033));
        close(deviations("P7", 40.0).unwrap(), (-0.017, -0.042));
        // ⌀12 F8 +0.043/+0.016.
        close(deviations("F8", 12.0).unwrap(), (0.043, 0.016));
    }

    #[test]
    fn unsupported_classes_are_errors() {
        assert!(deviations("z6", 20.0).is_err());
        assert!(deviations("H7", 600.0).is_err());
        assert!(deviations("H", 20.0).is_err());
        assert!(deviations("H3", 20.0).is_err());
    }
}

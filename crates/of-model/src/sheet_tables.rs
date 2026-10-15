//! Gauge tables and bend tables, as CSV.
//!
//! A gauge table has the columns `gauge, thickness, radii, k_factor`
//! (millimetres; radii separated by `;`). Built-in tables are `steel`,
//! `stainless` and `aluminum`; any other name is read as a file path.
//!
//! A bend table has the columns `thickness, radius, angle` (degrees) and one of
//! `allowance` or `deduction`, one row per entry. Values between entries are
//! interpolated linearly in thickness, radius and angle.

use crate::sheet_metal::{Allowance, SheetMetalParams};

const BUILT_IN: &[(&str, &str)] = &[
    ("steel", include_str!("../assets/sheet-metal/steel.csv")),
    (
        "stainless",
        include_str!("../assets/sheet-metal/stainless.csv"),
    ),
    (
        "aluminum",
        include_str!("../assets/sheet-metal/aluminum.csv"),
    ),
];

/// Names of the built-in gauge tables.
pub fn built_in_gauge_tables() -> Vec<&'static str> {
    BUILT_IN.iter().map(|(n, _)| *n).collect()
}

/// One gauge of a gauge table.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Gauge {
    pub gauge: String,
    pub thickness: f64,
    pub radii: Vec<f64>,
    pub k_factor: f64,
}

fn source(table: &str) -> Result<String, String> {
    if let Some((_, text)) = BUILT_IN.iter().find(|(n, _)| n.eq_ignore_ascii_case(table)) {
        return Ok(text.to_string());
    }
    std::fs::read_to_string(table).map_err(|e| format!("can't read table {table}: {e}"))
}

/// Rows of a CSV file as named columns, skipping blank lines and `#` comments.
fn rows(text: &str) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'));
    let header: Vec<String> = lines
        .next()
        .ok_or("the table is empty")?
        .split(',')
        .map(|c| c.trim().to_ascii_lowercase())
        .collect();
    let body = lines
        .map(|l| l.split(',').map(|c| c.trim().to_string()).collect())
        .collect();
    Ok((header, body))
}

fn column(header: &[String], name: &str) -> Result<usize, String> {
    header
        .iter()
        .position(|h| h == name)
        .ok_or_else(|| format!("the table has no `{name}` column"))
}

fn number(row: &[String], i: usize) -> Result<f64, String> {
    let cell = row.get(i).ok_or("a table row is short")?;
    cell.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("`{cell}` is not a number"))
}

/// Every gauge in a gauge table.
pub fn gauge_table(table: &str) -> Result<Vec<Gauge>, String> {
    let (header, body) = rows(&source(table)?)?;
    let (g, t, r, k) = (
        column(&header, "gauge")?,
        column(&header, "thickness")?,
        column(&header, "radii")?,
        column(&header, "k_factor")?,
    );
    body.iter()
        .map(|row| {
            let radii = row
                .get(r)
                .ok_or("a table row is short")?
                .split(';')
                .map(|v| {
                    v.trim()
                        .parse::<f64>()
                        .map_err(|_| format!("`{v}` is not a radius"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Gauge {
                gauge: row.get(g).cloned().unwrap_or_default(),
                thickness: number(row, t)?,
                radii,
                k_factor: number(row, k)?,
            })
        })
        .collect()
}

/// The parameters with a gauge choice applied: thickness, radius and K-factor
/// from the table.
pub fn resolve(params: &SheetMetalParams) -> Result<SheetMetalParams, String> {
    let Some(choice) = &params.gauge else {
        return Ok(params.clone());
    };
    let gauges = gauge_table(&choice.table)?;
    let g = gauges
        .iter()
        .find(|g| g.gauge == choice.gauge)
        .ok_or_else(|| format!("gauge {} is not in table {}", choice.gauge, choice.table))?;
    let radius = match choice.radius {
        Some(r) => *g
            .radii
            .iter()
            .find(|v| (**v - r).abs() < 1e-6)
            .ok_or_else(|| format!("gauge {} does not allow radius {r} mm", choice.gauge))?,
        None => g.radii.iter().copied().fold(f64::INFINITY, f64::min),
    };
    if !radius.is_finite() {
        return Err(format!("gauge {} lists no radii", choice.gauge));
    }
    let mut out = params.clone();
    out.thickness = g.thickness;
    out.radius = radius;
    if matches!(out.allowance, Allowance::KFactor { .. }) {
        out.allowance = Allowance::KFactor { k: g.k_factor };
    }
    Ok(out)
}

struct BendRow {
    thickness: f64,
    radius: f64,
    angle: f64,
    value: f64,
}

/// Interpolate `f` over sorted distinct keys around `x`.
fn bracket(keys: &[f64], x: f64, what: &str) -> Result<(f64, f64, f64), String> {
    if let Some(k) = keys.iter().find(|k| (**k - x).abs() < 1e-6) {
        return Ok((*k, *k, 0.0));
    }
    let hi = keys.iter().position(|k| *k > x);
    match hi {
        Some(i) if i > 0 => {
            let (a, b) = (keys[i - 1], keys[i]);
            Ok((a, b, (x - a) / (b - a)))
        }
        _ => Err(format!(
            "the bend table has no entries around {what} {x:.3}"
        )),
    }
}

fn sorted(mut v: Vec<f64>) -> Vec<f64> {
    v.sort_by(f64::total_cmp);
    v.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    v
}

/// Bend allowance from a bend table for one bend.
pub fn bend_table_allowance(
    table: &str,
    angle: f64,
    radius: f64,
    thickness: f64,
) -> Result<f64, String> {
    let (header, body) = rows(&source(table)?)?;
    let (t, r, a) = (
        column(&header, "thickness")?,
        column(&header, "radius")?,
        column(&header, "angle")?,
    );
    let (v, deduction) = match column(&header, "allowance") {
        Ok(i) => (i, false),
        Err(_) => (column(&header, "deduction")?, true),
    };
    let rows: Vec<BendRow> = body
        .iter()
        .map(|row| {
            Ok(BendRow {
                thickness: number(row, t)?,
                radius: number(row, r)?,
                angle: number(row, a)?,
                value: number(row, v)?,
            })
        })
        .collect::<Result<_, String>>()?;
    let degrees = angle.to_degrees();
    let at = |th: f64, ra: f64| -> Result<f64, String> {
        let entries: Vec<&BendRow> = rows
            .iter()
            .filter(|e| (e.thickness - th).abs() < 1e-9 && (e.radius - ra).abs() < 1e-9)
            .collect();
        let angles = sorted(entries.iter().map(|e| e.angle).collect());
        let (a0, a1, f) = bracket(&angles, degrees, "angle")?;
        let value = |an: f64| {
            entries
                .iter()
                .find(|e| (e.angle - an).abs() < 1e-9)
                .map(|e| e.value)
                .unwrap()
        };
        Ok(value(a0) * (1.0 - f) + value(a1) * f)
    };
    let thicknesses = sorted(rows.iter().map(|e| e.thickness).collect());
    let (t0, t1, ft) = bracket(&thicknesses, thickness, "thickness")?;
    let mut by_thickness = Vec::new();
    for th in [t0, t1] {
        let radii = sorted(
            rows.iter()
                .filter(|e| (e.thickness - th).abs() < 1e-9)
                .map(|e| e.radius)
                .collect(),
        );
        let (r0, r1, fr) = bracket(&radii, radius, "radius")?;
        by_thickness.push(at(th, r0)? * (1.0 - fr) + at(th, r1)? * fr);
    }
    let value = by_thickness[0] * (1.0 - ft) + by_thickness[1] * ft;
    Ok(if deduction {
        2.0 * (radius + thickness) * (angle / 2.0).tan() - value
    } else {
        value
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_gauges_resolve() {
        let params = SheetMetalParams {
            gauge: Some(crate::sheet_metal::GaugeChoice {
                table: "steel".into(),
                gauge: "16".into(),
                radius: None,
            }),
            ..Default::default()
        };
        let p = resolve(&params).unwrap();
        assert!((p.thickness - 1.519).abs() < 1e-3);
        assert!((p.radius - 1.5).abs() < 1e-9);
        assert_eq!(p.allowance, Allowance::KFactor { k: 0.44 });
    }

    #[test]
    fn bend_table_interpolates() {
        let dir = std::env::temp_dir().join(format!("of-bend-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("table.csv");
        std::fs::write(
            &path,
            "thickness,radius,angle,allowance\n1,1,90,2\n1,1,180,4\n2,1,90,3\n2,1,180,6\n",
        )
        .unwrap();
        let p = path.to_str().unwrap();
        let v = bend_table_allowance(p, 135f64.to_radians(), 1.0, 1.5).unwrap();
        // Thickness 1: 3; thickness 2: 4.5; halfway: 3.75.
        assert!((v - 3.75).abs() < 1e-9, "{v}");
        assert!(bend_table_allowance(p, 45f64.to_radians(), 1.0, 1.0).is_err());
    }
}

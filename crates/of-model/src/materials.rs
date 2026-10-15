//! Materials for mass properties.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Material {
    pub name: String,
    /// kg/m³.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub density: f64,
}

/// Built-in materials: (name, density in kg/m³).
pub const LIBRARY: &[(&str, f64)] = &[
    ("Plain Carbon Steel", 7800.0),
    ("Stainless Steel 304", 8000.0),
    ("Aluminum 6061", 2700.0),
    ("Aluminum 7075", 2810.0),
    ("Brass", 8500.0),
    ("Copper", 8960.0),
    ("Titanium Ti-6Al-4V", 4430.0),
    ("Cast Iron", 7200.0),
    ("ABS", 1040.0),
    ("PLA", 1240.0),
    ("PETG", 1270.0),
    ("Nylon 6/6", 1140.0),
    ("Acetal (POM)", 1410.0),
    ("Polycarbonate", 1200.0),
    ("Acrylic (PMMA)", 1180.0),
    ("HDPE", 950.0),
    ("Oak", 750.0),
    ("Pine", 500.0),
];

pub fn by_name(name: &str) -> Option<Material> {
    LIBRARY
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(n, d)| Material {
            name: n.to_string(),
            density: *d,
        })
}

/// Density used when no material is set (water), in kg/m³.
pub const DEFAULT_DENSITY: f64 = 1000.0;

/// Sheet metal defaults of a library material: K-factor and inside bend
/// radius as a multiple of thickness. Typical press brake values for air
/// bending; None for materials that are not formed as sheet.
pub fn sheet_metal_defaults(name: &str) -> Option<(f64, f64)> {
    let n = name.to_ascii_lowercase();
    Some(match n.as_str() {
        "plain carbon steel" => (0.44, 1.0),
        "stainless steel 304" => (0.45, 1.5),
        "aluminum 6061" => (0.43, 1.5),
        "aluminum 7075" => (0.45, 3.0),
        "brass" => (0.42, 1.0),
        "copper" => (0.42, 1.0),
        "titanium ti-6al-4v" => (0.45, 2.5),
        _ => return None,
    })
}

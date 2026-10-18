//! Drafting standards: text, arrow, line and tolerance conventions. A
//! document picks a base standard (ISO or ANSI) and may override values.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StandardBase {
    /// ISO 128 / ISO 129 / ISO 3098 conventions.
    #[default]
    Iso,
    /// ASME Y14.2 / Y14.5 conventions.
    Ansi,
}

/// Values a document may change from its base standard. Lengths are mm of
/// paper.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Overrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_height: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_height: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrow_length: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thick_line: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thin_line: Option<f64>,
    /// Decimal places of linear dimensions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision: Option<u8>,
    /// Decimal places of angles, degrees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle_precision: Option<u8>,
    /// Gap between the part and an extension line, mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension_gap: Option<f64>,
    /// Extension line past the dimension line, mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension_overshoot: Option<f64>,
    /// Tolerance text height as a fraction of the value's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance_scale: Option<f64>,
    /// Show trailing zeros up to the precision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trailing_zeros: Option<bool>,
    /// Dimension text always horizontal (ANSI) instead of along the line (ISO).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unidirectional: Option<bool>,
    /// Show a second value in inches after every linear dimension.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dual_units: Option<bool>,
    /// Use a decimal comma.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimal_comma: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Standard {
    #[serde(default)]
    pub base: StandardBase,
    #[serde(default)]
    pub overrides: Overrides,
}

impl Standard {
    pub fn iso() -> Standard {
        Standard::default()
    }

    pub fn ansi() -> Standard {
        Standard {
            base: StandardBase::Ansi,
            overrides: Overrides::default(),
        }
    }

    fn pick(&self, o: Option<f64>, iso: f64, ansi: f64) -> f64 {
        o.unwrap_or(match self.base {
            StandardBase::Iso => iso,
            StandardBase::Ansi => ansi,
        })
    }

    /// Height of dimension and note text.
    pub fn text_height(&self) -> f64 {
        self.pick(self.overrides.text_height, 3.5, 3.175)
    }

    /// Height of view labels (`SECTION A-A`).
    pub fn label_height(&self) -> f64 {
        self.pick(self.overrides.label_height, 5.0, 4.8)
    }

    pub fn arrow_length(&self) -> f64 {
        self.pick(self.overrides.arrow_length, 3.0, 3.175)
    }

    /// Visible outlines.
    pub fn thick_line(&self) -> f64 {
        self.pick(self.overrides.thick_line, 0.5, 0.6)
    }

    /// Hidden, center, dimension and hatch lines.
    pub fn thin_line(&self) -> f64 {
        self.pick(self.overrides.thin_line, 0.25, 0.3)
    }

    pub fn precision(&self) -> u8 {
        self.overrides.precision.unwrap_or(match self.base {
            StandardBase::Iso => 1,
            StandardBase::Ansi => 2,
        })
    }

    pub fn angle_precision(&self) -> u8 {
        self.overrides.angle_precision.unwrap_or(0)
    }

    pub fn extension_gap(&self) -> f64 {
        self.pick(self.overrides.extension_gap, 1.0, 1.5)
    }

    pub fn extension_overshoot(&self) -> f64 {
        self.pick(self.overrides.extension_overshoot, 2.0, 3.0)
    }

    pub fn tolerance_scale(&self) -> f64 {
        self.overrides.tolerance_scale.unwrap_or(0.7)
    }

    pub fn trailing_zeros(&self) -> bool {
        self.overrides
            .trailing_zeros
            .unwrap_or(self.base == StandardBase::Ansi)
    }

    pub fn unidirectional(&self) -> bool {
        self.overrides
            .unidirectional
            .unwrap_or(self.base == StandardBase::Ansi)
    }

    pub fn dual_units(&self) -> bool {
        self.overrides.dual_units.unwrap_or(false)
    }

    /// A number at a precision, with the standard's zeros and separator.
    pub fn number(&self, v: f64, precision: u8) -> String {
        // Round half away from zero, as drafting does (`format!` rounds a
        // tie like 20.25 to even).
        let m = 10f64.powi(precision as i32);
        let r = (v * m).round() / m;
        let mut s = format!("{:.*}", precision as usize, r);
        if !self.trailing_zeros() && s.contains('.') {
            while s.ends_with('0') {
                s.pop();
            }
            if s.ends_with('.') {
                s.pop();
            }
        }
        if s == "-0" {
            s = "0".into();
        }
        if self.overrides.decimal_comma.unwrap_or(false) {
            s = s.replace('.', ",");
        }
        s
    }

    /// Dash and gap lengths for a line style, mm of paper, scaled with the
    /// line weight per ISO 128-20.
    pub fn dashes(&self, style: crate::graphics::LineStyle) -> &'static [f64] {
        use crate::graphics::LineStyle::*;
        match style {
            Continuous => &[],
            Hidden => &[3.0, 1.5],
            Center => &[8.0, 1.5, 0.5, 1.5],
            Phantom => &[8.0, 1.5, 0.5, 1.5, 0.5, 1.5],
        }
    }
}

/// `format` of a drafting standard file.
pub const STANDARD_FILE: &str = "openforge-drafting-standard";
/// Extension of drafting standard files.
pub const STANDARD_EXTENSION: &str = "ofstd";

#[derive(Serialize, Deserialize)]
struct StandardFile {
    format: String,
    version: u32,
    standard: Standard,
}

impl Standard {
    /// Save as a reusable drafting standard file.
    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(&StandardFile {
            format: STANDARD_FILE.into(),
            version: 1,
            standard: *self,
        })
        .map_err(|e| e.to_string())?;
        std::fs::write(path, text + "\n")
            .map_err(|e| format!("could not write {}: {e}", path.display()))
    }

    /// Read a drafting standard file.
    pub fn load(path: &std::path::Path) -> Result<Standard, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("could not open {}: {e}", path.display()))?;
        let f: StandardFile = serde_json::from_str(&text)
            .map_err(|e| format!("not a valid drafting standard file: {e}"))?;
        if f.format != STANDARD_FILE {
            return Err("not an OpenForge drafting standard file".into());
        }
        if f.version != 1 {
            return Err(format!(
                "drafting standard file version {} is not supported",
                f.version
            ));
        }
        Ok(f.standard)
    }
}

#[cfg(test)]
mod file_tests {
    use super::*;

    #[test]
    fn standards_round_trip_through_files() {
        let mut s = Standard::ansi();
        s.overrides.text_height = Some(4.0);
        s.overrides.decimal_comma = Some(true);
        let path = std::env::temp_dir().join(format!("of-std-{}.ofstd", std::process::id()));
        s.save(&path).unwrap();
        assert_eq!(Standard::load(&path).unwrap(), s);
        std::fs::write(&path, r#"{"format":"x","version":1,"standard":{}}"#).unwrap();
        assert!(Standard::load(&path).is_err());
        std::fs::remove_file(&path).ok();
        // ANSI keeps trailing zeros; ties round away from zero.
        assert_eq!(s.number(1.25, 1), "1,3");
        assert_eq!(s.number(2.0, 2), "2,00");
    }
}

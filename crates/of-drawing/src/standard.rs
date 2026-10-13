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

//! Custom properties: named values stored with a part (and overridden per
//! configuration) for bills of materials and title blocks. Values may link
//! to model values with `{mass}`, `{volume}`, `{material}`, `{density}`,
//! `{configuration}` or `{param:f2.end1.depth}`.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Document, State};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PropertyKind {
    #[default]
    Text,
    Number,
    Date,
    YesNo,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Property {
    pub value: String,
    #[serde(default)]
    pub kind: PropertyKind,
}

impl Property {
    pub fn text(value: impl Into<String>) -> Property {
        Property {
            value: value.into(),
            kind: PropertyKind::Text,
        }
    }

    /// Check the value suits its kind (links are checked when evaluated).
    pub fn validate(&self, name: &str) -> Result<(), String> {
        if name.trim().is_empty() {
            return Err("a property needs a name".into());
        }
        if self.value.contains('{') {
            return Ok(());
        }
        let ok = match self.kind {
            PropertyKind::Text => true,
            PropertyKind::Number => self.value.trim().parse::<f64>().is_ok(),
            PropertyKind::YesNo => matches!(
                self.value.trim().to_ascii_lowercase().as_str(),
                "yes" | "no" | "true" | "false"
            ),
            PropertyKind::Date => {
                let p: Vec<&str> = self.value.trim().split('-').collect();
                p.len() == 3 && p.iter().all(|s| s.parse::<u32>().is_ok())
            }
        };
        if ok {
            Ok(())
        } else {
            Err(format!(
                "property {name}: {:?} is not a valid {:?}",
                self.value, self.kind
            ))
        }
    }
}

/// Base properties with the active configuration's overrides applied.
pub fn effective(doc: &Document) -> BTreeMap<String, Property> {
    let mut out = doc.properties.clone();
    out.extend(doc.configurations.active().properties.clone());
    out
}

/// Replace links in a value with model values for this document and state.
pub fn evaluate(doc: &Document, state: &State, value: &str) -> String {
    if !value.contains('{') {
        return value.to_string();
    }
    let mass = crate::measure::mass_properties(doc, state).ok();
    let effective = crate::configurations::resolve(doc).ok();
    let mut out = String::new();
    let mut rest = value;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let token = &rest[start + 1..start + end];
        let replaced = match token {
            "mass" => mass.as_ref().map(|m| format!("{:.4}", m.mass)),
            "volume" => mass.as_ref().map(|m| format!("{:.3}", m.volume)),
            "density" => mass.as_ref().map(|m| format!("{}", m.density)),
            "material" => Some(
                effective
                    .as_ref()
                    .and_then(|d| d.material.as_ref())
                    .map(|m| m.name.clone())
                    .unwrap_or_default(),
            ),
            "configuration" => Some(doc.configurations.active().name.clone()),
            t if t.starts_with("param:") => {
                let key = &t[6..];
                effective.as_ref().and_then(|d| {
                    d.features
                        .iter()
                        .flat_map(crate::equations::parameters)
                        .find(|p| p.key == key)
                        .map(|p| format!("{}", (p.value.value * 1e6).round() / 1e6))
                })
            }
            _ => None,
        };
        match replaced {
            Some(v) => out.push_str(&v),
            None => out.push_str(&rest[start..start + end + 1]),
        }
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    out
}

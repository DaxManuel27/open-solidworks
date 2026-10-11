//! Manual variants share a feature tree and resolve overrides into a temporary
//! document. Switching configurations never writes solved geometry into the base.

use crate::{Document, FeatureId, equations, materials::Material};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mode", content = "value", rename_all = "snake_case")]
pub enum MaterialOverride {
    #[default]
    Inherit,
    Clear,
    Set(Material),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Configuration {
    /// Existing configuration ID from `configurations.items`.
    #[schemars(range(min = 1))]
    pub id: u32,
    pub name: String,
    /// Parameter key (`f2.end1.depth`) to value in core units (mm, radians).
    #[serde(default)]
    pub parameters: BTreeMap<String, f64>,
    /// Parameter key to formula; null stops inheriting the base equation.
    #[serde(default)]
    pub equations: BTreeMap<String, Option<String>>,
    /// Variable name to formula, overriding base variables.
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    /// Feature ID to suppression state in this configuration.
    #[serde(default)]
    pub suppressed: BTreeMap<FeatureId, bool>,
    #[serde(default)]
    pub material: MaterialOverride,
}

impl Configuration {
    pub fn new(id: u32, name: String) -> Self {
        Self {
            id,
            name,
            parameters: BTreeMap::new(),
            equations: BTreeMap::new(),
            variables: BTreeMap::new(),
            suppressed: BTreeMap::new(),
            material: MaterialOverride::Inherit,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Configurations {
    pub active: u32,
    pub next_id: u32,
    pub items: Vec<Configuration>,
}

impl Default for Configurations {
    fn default() -> Self {
        Self {
            active: 1,
            next_id: 2,
            items: vec![Configuration::new(1, "Default".into())],
        }
    }
}

impl Configurations {
    pub fn active(&self) -> &Configuration {
        self.items
            .iter()
            .find(|c| c.id == self.active)
            .expect("validated configuration identity")
    }
    pub fn validate(&self) -> Result<(), String> {
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        for c in &self.items {
            if c.id == 0 || c.id >= self.next_id || !ids.insert(c.id) {
                return Err("invalid configuration ID".into());
            }
            if c.name.trim().is_empty() || !names.insert(c.name.trim()) {
                return Err("configuration names must be nonempty and unique".into());
            }
            if c.variables.keys().any(|v| !equations::valid_variable(v)) {
                return Err("invalid configuration variable name".into());
            }
            if c.parameters.values().any(|v| !v.is_finite()) {
                return Err("configuration values must be finite".into());
            }
            if let MaterialOverride::Set(m) = &c.material
                && (!m.density.is_finite() || m.density <= 0.0)
            {
                return Err("material density must be positive and finite".into());
            }
        }
        if !ids.contains(&self.active) {
            return Err("active configuration does not exist".into());
        }
        Ok(())
    }
}

pub fn resolve(doc: &Document) -> Result<Document, String> {
    doc.configurations.validate()?;
    let config = doc.configurations.active();
    let mut effective = doc.clone();
    let parameters: BTreeMap<_, _> = doc
        .features
        .iter()
        .flat_map(equations::parameters)
        .map(|p| (p.key.clone(), p))
        .collect();
    for (key, value) in &config.parameters {
        let p = parameters
            .get(key)
            .ok_or_else(|| format!("configured parameter {key} no longer exists"))?;
        equations::set_parameter(
            effective.feature_mut(p.feature).unwrap(),
            p,
            equations::Quantity {
                value: *value,
                ..p.value
            },
        )?;
        effective.equations.remove(key);
    }
    for (key, expression) in &config.equations {
        if !parameters.contains_key(key) {
            return Err(format!("configured parameter {key} no longer exists"));
        }
        if let Some(expression) = expression {
            effective.equations.insert(key.clone(), expression.clone());
        } else {
            effective.equations.remove(key);
        }
    }
    effective.variables.extend(config.variables.clone());
    for (id, suppressed) in &config.suppressed {
        effective
            .feature_mut(*id)
            .ok_or_else(|| format!("configured feature {id} no longer exists"))?
            .suppressed = *suppressed;
    }
    effective.material = match &config.material {
        MaterialOverride::Inherit => doc.material.clone(),
        MaterialOverride::Clear => None,
        MaterialOverride::Set(m) => Some(m.clone()),
    };
    Ok(effective)
}

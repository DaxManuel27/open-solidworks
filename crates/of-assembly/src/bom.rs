//! Bills of materials: rows grouped by file and configuration, with part
//! properties, in top-level, parts-only or indented form.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::assembly::{Assembly, Instance, Loaded};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BomType {
    /// Components directly in this assembly.
    #[default]
    TopLevel,
    /// Every part, however deep, with total quantities.
    PartsOnly,
    /// Top level with each subassembly's contents beneath it.
    Indented,
}

#[derive(Debug, Clone, Serialize)]
pub struct BomRow {
    /// Item number: `3`, or `3.2` under item 3.
    pub item: String,
    pub level: usize,
    pub quantity: u32,
    /// Part number: the `PartNo` property, else the file name.
    pub part_number: String,
    pub description: String,
    pub configuration: String,
    pub file: String,
    pub assembly: bool,
    pub material: String,
    /// kg, each.
    pub mass: Option<f64>,
    /// Every evaluated custom property.
    pub properties: BTreeMap<String, String>,
}

fn row_for(i: &Instance) -> BomRow {
    let stem = i
        .file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("component")
        .to_string();
    let mut row = BomRow {
        item: String::new(),
        level: 0,
        quantity: 0,
        part_number: stem.clone(),
        description: String::new(),
        configuration: String::new(),
        file: i.file.to_string_lossy().into_owned(),
        assembly: matches!(i.loaded, Loaded::Assembly(_)),
        material: String::new(),
        mass: None,
        properties: BTreeMap::new(),
    };
    match &i.loaded {
        Loaded::Part(p) => {
            let doc = p.doc();
            row.configuration = doc.configurations.active().name.clone();
            for (k, v) in of_model::properties::effective(doc) {
                row.properties
                    .insert(k, of_model::properties::evaluate(doc, p.state(), &v.value));
            }
            if let Ok(m) = of_model::measure::mass_properties(doc, p.state()) {
                row.mass = Some(m.mass);
                row.material = m.material.unwrap_or_default();
            }
        }
        Loaded::Assembly(a) => {
            row.configuration = a.doc.active().name.clone();
            row.properties = a.doc.properties.clone();
        }
        Loaded::Missing(_) => {}
    }
    if let Some(p) = row.properties.get("PartNo").filter(|p| !p.is_empty()) {
        row.part_number = p.clone();
    }
    if let Some(d) = row.properties.get("Description") {
        row.description = d.clone();
    }
    row
}

/// A file and configuration: what makes BOM rows the same item.
type Item = (PathBuf, Option<u32>);

/// Group instances by file and configuration, keeping first-seen order.
fn group<'a>(instances: impl Iterator<Item = &'a Instance>) -> Vec<(BomRow, Vec<&'a Instance>)> {
    let mut groups: Vec<(Item, BomRow, Vec<&Instance>)> = Vec::new();
    for i in instances {
        let key = (crate::cache::canonical(&i.file), i.configuration);
        match groups.iter_mut().find(|g| g.0 == key) {
            Some(g) => {
                g.1.quantity += 1;
                g.2.push(i);
            }
            None => {
                let mut row = row_for(i);
                row.quantity = 1;
                groups.push((key, row, vec![i]));
            }
        }
    }
    groups.into_iter().map(|(_, r, v)| (r, v)).collect()
}

pub fn bom(asm: &mut Assembly, kind: BomType) -> Vec<BomRow> {
    let all = asm.instances();
    // Excluded components take their children with them.
    let excluded: Vec<Vec<String>> = all
        .iter()
        .filter(|i| i.exclude_from_bom)
        .map(|i| i.key.clone())
        .collect();
    let counted: Vec<&Instance> = all
        .iter()
        .filter(|i| !excluded.iter().any(|e| i.key.starts_with(e)))
        .collect();
    let mut rows = Vec::new();
    match kind {
        BomType::TopLevel => {
            for (n, (mut row, _)) in group(counted.iter().copied().filter(|i| i.key.len() == 1))
                .into_iter()
                .enumerate()
            {
                row.item = (n + 1).to_string();
                rows.push(row);
            }
        }
        BomType::PartsOnly => {
            for (n, (mut row, _)) in group(
                counted
                    .iter()
                    .copied()
                    .filter(|i| matches!(i.loaded, Loaded::Part(_))),
            )
            .into_iter()
            .enumerate()
            {
                row.item = (n + 1).to_string();
                rows.push(row);
            }
        }
        BomType::Indented => indented(&counted, &[], "", 0, &mut rows),
    }
    rows
}

fn indented(
    all: &[&Instance],
    parent: &[String],
    prefix: &str,
    level: usize,
    rows: &mut Vec<BomRow>,
) {
    let children = all
        .iter()
        .copied()
        .filter(|i| i.key.len() == parent.len() + 1 && i.key.starts_with(parent));
    for (n, (mut row, members)) in group(children).into_iter().enumerate() {
        let item = format!("{prefix}{}", n + 1);
        row.item = item.clone();
        row.level = level;
        let assembly = row.assembly;
        rows.push(row);
        if assembly {
            // List one instance's contents; quantities are per subassembly.
            indented(all, &members[0].key, &format!("{item}."), level + 1, rows);
        }
    }
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// CSV with the standard columns plus each custom property that appears.
pub fn csv(rows: &[BomRow]) -> String {
    let mut extra: Vec<String> = rows
        .iter()
        .flat_map(|r| r.properties.keys().cloned())
        .filter(|k| k != "PartNo" && k != "Description")
        .collect();
    extra.sort();
    extra.dedup();
    let mut out = String::from("Item,Qty,Part Number,Description,Configuration,Material,Mass (kg)");
    for e in &extra {
        out.push(',');
        out.push_str(&csv_cell(e));
    }
    out.push('\n');
    for r in rows {
        let cells = [
            r.item.clone(),
            r.quantity.to_string(),
            r.part_number.clone(),
            r.description.clone(),
            r.configuration.clone(),
            r.material.clone(),
            r.mass.map(|m| format!("{m:.4}")).unwrap_or_default(),
        ];
        let mut line: Vec<String> = cells.iter().map(|c| csv_cell(c)).collect();
        for e in &extra {
            line.push(csv_cell(r.properties.get(e).map_or("", String::as_str)));
        }
        out.push_str(&line.join(","));
        out.push('\n');
    }
    out
}

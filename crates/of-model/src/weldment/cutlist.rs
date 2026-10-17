//! Cut lists: the part's bodies grouped into items (identical members,
//! sheet metal blanks and other bodies), with manual groups, exclusions,
//! property overrides and sub-weldment folders; and the weld folder.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{BeadKind, WeldRecord};
use crate::build::State;
use crate::materials::DEFAULT_DENSITY;
use crate::{BodyId, Document};

/// The cut list's settings, saved with the part.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CutListSettings {
    /// Bodies listed together as one item whatever their shape.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<Vec<BodyId>>,
    /// Bodies left out of the cut list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excluded: Vec<BodyId>,
    /// Item properties set by hand, keyed by the item's first body.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<BodyId, BTreeMap<String, String>>,
    /// Folders of bodies fabricated apart, listed on their own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sub_weldments: Vec<SubWeldment>,
    /// List each body as its own item instead of collecting identical ones.
    #[serde(default, skip_serializing_if = "crate::sheet_metal::is_false")]
    pub separate: bool,
    #[serde(default)]
    pub sort: CutListSort,
}

impl CutListSettings {
    pub fn is_empty(&self) -> bool {
        *self == CutListSettings::default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SubWeldment {
    pub name: String,
    pub bodies: Vec<BodyId>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CutListSort {
    /// Members first in the order they were made, then the rest.
    #[default]
    Default,
    /// By profile, then length.
    Profile,
}

/// What kind of item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Member,
    SheetMetal,
    Body,
}

/// One cut list item: identical bodies and their properties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CutListItem {
    /// 1-based, in list order (excluded items have none).
    pub number: Option<u32>,
    pub kind: ItemKind,
    pub bodies: Vec<BodyId>,
    pub quantity: u32,
    /// Sub-weldment folder, if any.
    pub folder: Option<String>,
    pub excluded: bool,
    /// Properties with overrides applied: DESCRIPTION, MATERIAL, QUANTITY,
    /// MASS (kg, each), LENGTH, ANGLE1, ANGLE2 (degrees) for members,
    /// THICKNESS, BLANK LENGTH, BLANK WIDTH, BENDS for sheet metal, and the
    /// bounding box (BOUNDING LENGTH, WIDTH, THICKNESS).
    pub properties: BTreeMap<String, String>,
}

/// The weld folder: every bead and the totals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeldFolder {
    pub beads: Vec<WeldRecord>,
    /// mm.
    pub total_length: f64,
    /// kg: cosmetic beads by their mass per length, modeled ones by volume.
    pub total_mass: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CutList {
    pub items: Vec<CutListItem>,
    pub welds: WeldFolder,
}

fn num(v: f64, places: usize) -> String {
    let s = format!("{v:.places$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

/// A body's description for grouping and its properties.
struct Facts {
    kind: ItemKind,
    key: String,
    props: BTreeMap<String, String>,
    sort: (u8, String, i64),
}

fn facts(doc: &Document, state: &State, index: usize, density: f64) -> Result<Facts, String> {
    let body = &state.bodies[index];
    let shape = &body.geometry.shape;
    let mp = shape.mass_props().map_err(|e| e.0)?;
    let mass = mp.volume * density * 1e-9;
    let mut props = BTreeMap::new();
    let material = doc
        .material
        .as_ref()
        .map(|m| m.name.clone())
        .unwrap_or_default();
    props.insert("MATERIAL".into(), material);
    props.insert("MASS".into(), num(mass, 3));
    let bbox = shape.bbox().ok_or("a body has no extent")?;
    let mut sizes = [
        bbox.max.x - bbox.min.x,
        bbox.max.y - bbox.min.y,
        bbox.max.z - bbox.min.z,
    ];
    sizes.sort_by(|a, b| b.total_cmp(a));
    // Members: by profile, length and end angles.
    if let Some(m) = state.members.iter().find(|m| m.body == body.id)
        && state.members.iter().filter(|r| r.body == body.id).count() == 1
    {
        let desc = m.profile.description();
        let mut angles = m.angles.map(|a| a.to_degrees());
        angles.sort_by(f64::total_cmp);
        props.insert("DESCRIPTION".into(), desc.clone());
        props.insert("LENGTH".into(), num(m.length, 2));
        props.insert("ANGLE1".into(), num(angles[0], 1));
        props.insert("ANGLE2".into(), num(angles[1], 1));
        let w = m.outline.iter().map(|q| q[0]).fold(f64::MIN, f64::max)
            - m.outline.iter().map(|q| q[0]).fold(f64::MAX, f64::min);
        let h = m.outline.iter().map(|q| q[1]).fold(f64::MIN, f64::max)
            - m.outline.iter().map(|q| q[1]).fold(f64::MAX, f64::min);
        props.insert("BOUNDING LENGTH".into(), num(m.length, 2));
        props.insert("BOUNDING WIDTH".into(), num(w.max(h), 2));
        props.insert("BOUNDING THICKNESS".into(), num(w.min(h), 2));
        return Ok(Facts {
            kind: ItemKind::Member,
            key: format!("m|{desc}|{:.3}|{:.4}|{:.4}", m.length, angles[0], angles[1]),
            props,
            sort: (0, desc, (m.length * 1000.0).round() as i64),
        });
    }
    // Sheet metal: by blank.
    if let Some(i) = (0..state.sheet_metal.len())
        .find(|i| crate::sheet_metal::record_body(state, *i) == Some(index))
        && let Ok((_, p)) = crate::flat::sheet_properties(state, i)
    {
        let desc = match &p.gauge {
            Some(g) => format!("Sheet, gauge {g}"),
            None => format!("Sheet {} mm", num(p.thickness, 2)),
        };
        props.insert("DESCRIPTION".into(), desc.clone());
        props.insert("THICKNESS".into(), num(p.thickness, 3));
        props.insert("BLANK LENGTH".into(), num(p.flat_length, 2));
        props.insert("BLANK WIDTH".into(), num(p.flat_width, 2));
        props.insert("BENDS".into(), p.bends.to_string());
        props.insert("CUT OUTS".into(), p.cutouts.to_string());
        props.insert("CUTTING LENGTH".into(), num(p.outer_cut + p.inner_cut, 2));
        props.insert("BOUNDING LENGTH".into(), num(p.flat_length, 2));
        props.insert("BOUNDING WIDTH".into(), num(p.flat_width, 2));
        props.insert("BOUNDING THICKNESS".into(), num(p.thickness, 3));
        return Ok(Facts {
            kind: ItemKind::SheetMetal,
            key: format!(
                "s|{:.3}|{:.2}|{:.2}|{:.1}|{}|{}",
                p.thickness, p.flat_length, p.flat_width, p.flat_area, p.bends, p.cutouts
            ),
            props,
            sort: (1, desc, (p.flat_length * 1000.0).round() as i64),
        });
    }
    // Anything else: by volume, area, extents and principal moments.
    props.insert("DESCRIPTION".into(), "Body".into());
    props.insert("BOUNDING LENGTH".into(), num(sizes[0], 2));
    props.insert("BOUNDING WIDTH".into(), num(sizes[1], 2));
    props.insert("BOUNDING THICKNESS".into(), num(sizes[2], 2));
    let mut pm = mp.principal_moments.to_array();
    pm.sort_by(f64::total_cmp);
    let rel = |v: f64| format!("{:.5e}", v);
    Ok(Facts {
        kind: ItemKind::Body,
        key: format!(
            "b|{}|{}|{:.2}|{:.2}|{:.2}|{}|{}|{}",
            rel(mp.volume),
            rel(mp.area),
            sizes[0],
            sizes[1],
            sizes[2],
            rel(pm[0]),
            rel(pm[1]),
            rel(pm[2])
        ),
        props,
        sort: (2, "Body".into(), (mp.volume * 1000.0).round() as i64),
    })
}

/// The part's cut list.
pub fn cut_list(doc: &Document, state: &State) -> Result<CutList, String> {
    let effective = crate::configurations::resolve(doc)?;
    let density = effective
        .material
        .as_ref()
        .map(|m| m.density)
        .unwrap_or(DEFAULT_DENSITY);
    let settings = &doc.cut_list;
    // Beads modeled as bodies belong to the weld folder.
    let bead_bodies: Vec<&BodyId> = state.welds.iter().flat_map(|w| &w.bodies).collect();
    let mut facts_of: Vec<(usize, Facts)> = Vec::new();
    for i in 0..state.bodies.len() {
        if bead_bodies.contains(&&state.bodies[i].id) {
            continue;
        }
        facts_of.push((i, facts(&effective, state, i, density)?));
    }
    let folder_of = |id: &BodyId| {
        settings
            .sub_weldments
            .iter()
            .find(|f| f.bodies.contains(id))
            .map(|f| f.name.clone())
    };
    // Items: manual groups first, then automatic grouping by key.
    let mut items: Vec<(CutListItem, (u8, String, i64))> = Vec::new();
    let mut placed: Vec<usize> = Vec::new();
    for group in &settings.groups {
        let members: Vec<&(usize, Facts)> = facts_of
            .iter()
            .filter(|(i, _)| group.contains(&state.bodies[*i].id))
            .collect();
        let Some((_, first)) = members.first() else {
            continue;
        };
        placed.extend(members.iter().map(|(i, _)| *i));
        items.push((
            CutListItem {
                number: None,
                kind: first.kind,
                bodies: members
                    .iter()
                    .map(|(i, _)| state.bodies[*i].id.clone())
                    .collect(),
                quantity: members.len() as u32,
                folder: folder_of(&state.bodies[members[0].0].id),
                excluded: false,
                properties: first.props.clone(),
            },
            first.sort.clone(),
        ));
    }
    for (i, f) in &facts_of {
        if placed.contains(i) {
            continue;
        }
        let id = state.bodies[*i].id.clone();
        let excluded = settings.excluded.contains(&id);
        let folder = folder_of(&id);
        let existing = (!settings.separate && !excluded)
            .then(|| {
                items.iter_mut().find(|(item, sort)| {
                    !item.excluded
                        && item.folder == folder
                        && item.properties.get("__key") == Some(&f.key)
                        && sort.0 == f.sort.0
                })
            })
            .flatten();
        match existing {
            Some((item, _)) => {
                item.bodies.push(id);
                item.quantity += 1;
            }
            None => {
                let mut properties = f.props.clone();
                properties.insert("__key".into(), f.key.clone());
                items.push((
                    CutListItem {
                        number: None,
                        kind: f.kind,
                        bodies: vec![id],
                        quantity: 1,
                        folder,
                        excluded,
                        properties,
                    },
                    f.sort.clone(),
                ));
            }
        }
    }
    if settings.sort == CutListSort::Profile {
        items.sort_by(|a, b| a.1.cmp(&b.1));
    } else {
        items.sort_by_key(|a| a.1.0);
    }
    // Folders after the main list, in their order.
    items.sort_by_key(|(item, _)| {
        item.folder
            .as_ref()
            .and_then(|f| settings.sub_weldments.iter().position(|s| &s.name == f))
            .map_or(0, |p| p + 1)
    });
    let mut out = Vec::new();
    let mut n = 0;
    for (mut item, _) in items {
        item.properties.remove("__key");
        if !item.excluded {
            n += 1;
            item.number = Some(n);
        }
        item.properties
            .insert("QUANTITY".into(), item.quantity.to_string());
        if let Some(o) = item.bodies.first().and_then(|b| settings.overrides.get(b)) {
            for (k, v) in o {
                item.properties.insert(k.to_uppercase(), v.clone());
            }
        }
        out.push(item);
    }
    let welds = weld_folder(state, density);
    Ok(CutList { items: out, welds })
}

fn weld_folder(state: &State, density: f64) -> WeldFolder {
    let mut total_length = 0.0;
    let mut total_mass = 0.0;
    for w in &state.welds {
        total_length += w.length;
        total_mass += match w.kind {
            BeadKind::Cosmetic => w.mass_per_length.unwrap_or(0.0) * w.length / 1000.0,
            BeadKind::Fillet => w
                .bodies
                .iter()
                .filter_map(|id| state.solid(id).ok())
                .filter_map(|b| b.geometry.shape.mass_props().ok())
                .map(|m| m.volume * density * 1e-9)
                .sum(),
        };
    }
    WeldFolder {
        beads: state.welds.clone(),
        total_length,
        total_mass,
    }
}

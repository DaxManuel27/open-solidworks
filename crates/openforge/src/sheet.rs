//! Sheet metal in the core: the sheet metal state and properties of each
//! body, and flat pattern DXF export.

use std::path::PathBuf;

use of_drawing::{flat, standard::Standard};
use of_model::Model;
use of_model::flat::{FlatOutline, sheet_properties};
use serde_json::{Value, json};

use crate::tools::params::Export;
use crate::views::body_name;

/// Every sheet metal body: parameters, bends and cut list properties.
pub fn info(model: &Model, outline: bool) -> Value {
    let state = model.state();
    let bodies: Vec<Value> = state
        .sheet_metal
        .iter()
        .enumerate()
        .map(|(i, record)| {
            let bends: Vec<Value> = record
                .bends
                .iter()
                .map(|b| {
                    json!({ "feature": b.feature, "index": b.index, "faces": b.faces,
                        "angle": b.angle, "radius": b.radius, "allowance": b.allowance })
                })
                .collect();
            let (flat, props, error) = match sheet_properties(state, i) {
                Ok((flat, props)) => (Some(flat), Some(props), None),
                Err(e) => (None, None, Some(e)),
            };
            let body = props.as_ref().and_then(|p| p.body.clone());
            let name = body.as_ref().and_then(|id| {
                let index = state.bodies.iter().position(|b| b.id == *id)?;
                Some(body_name(&model.doc, state, index))
            });
            json!({
                "feature": record.feature,
                "body": body,
                "name": name,
                "parameters": record.params,
                "flattened": record.flat_plane.is_some(),
                "bends": bends,
                "properties": props,
                "outline": if outline { json!(flat) } else { Value::Null },
                "error": error,
            })
        })
        .collect();
    json!({ "bodies": bodies })
}

/// Write the flat patterns of the part's sheet metal bodies to DXF. Returns
/// the files written.
pub fn export_dxf(model: &Model, p: &Export) -> Result<Vec<PathBuf>, String> {
    let state = model.state();
    let mut all = Vec::new();
    for i in 0..state.sheet_metal.len() {
        let (flat, props) = sheet_properties(state, i)?;
        if let Some(id) = props.body {
            all.push((id, flat));
        }
    }
    if let Some(wanted) = &p.bodies
        && let Some(id) = wanted.iter().find(|w| !all.iter().any(|(id, _)| id == *w))
    {
        return Err(format!("body {} is not a sheet metal body", id.0));
    }
    let mut flats: Vec<(String, FlatOutline)> = all
        .into_iter()
        .filter(|(id, _)| p.bodies.as_ref().is_none_or(|b| b.contains(id)))
        .map(|(id, flat)| {
            let index = state.bodies.iter().position(|b| b.id == id).unwrap_or(0);
            (body_name(&model.doc, state, index), flat)
        })
        .collect();
    // Surface Flatten outlines too.
    for fid in state.flattenings.keys() {
        let prefix = format!("b{fid}:");
        for (i, b) in state.surfaces.iter().enumerate() {
            if !b.id.0.starts_with(&prefix) || p.bodies.as_ref().is_some_and(|w| !w.contains(&b.id))
            {
                continue;
            }
            let face = b.geometry.shape.face_info(0).map_err(|e| e.0)?;
            let plane = of_model::geom::Plane::new(face.point, face.x_dir, face.normal);
            let flat = of_model::sheet_metal::Flattened {
                shape: b.geometry.shape.clone(),
                names: b.geometry.names.clone(),
                plane,
                thickness: 0.0,
                bend_lines: Vec::new(),
                marks: Vec::new(),
            };
            let outline = of_model::flat::outline(&flat)?;
            flats.push((crate::views::surface_name(&model.doc, state, i), outline));
        }
    }
    if flats.is_empty() {
        return Err("the part has no sheet metal bodies or flattened surfaces to write".into());
    }
    let standard = Standard::iso();
    let version = p.dxf_version.unwrap_or_default();
    let write = |path: &PathBuf, flats: &[FlatOutline]| -> Result<(), String> {
        let text = flat::dxf(flats, p.bend_notes, &standard, version)?;
        std::fs::write(path, text).map_err(|e| format!("can't write {}: {e}", path.display()))
    };
    if !p.per_body || flats.len() == 1 {
        let all: Vec<FlatOutline> = flats.into_iter().map(|(_, f)| f).collect();
        write(&p.path, &all)?;
        return Ok(vec![p.path.clone()]);
    }
    let stem = p
        .path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "flat".into());
    let mut out = Vec::new();
    for (name, f) in flats {
        let safe: String = name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || " -_".contains(c) {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let file = p.path.with_file_name(format!("{stem}-{safe}.dxf"));
        write(&file, std::slice::from_ref(&f))?;
        out.push(file);
    }
    Ok(out)
}

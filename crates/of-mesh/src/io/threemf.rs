//! 3MF: a zip archive with the model as XML (`3D/3dmodel.model`).

use glam::DVec3;

use super::{attr, io, tags, zip};
use crate::{Result, TriMesh};

pub fn read(bytes: &[u8]) -> Result<TriMesh> {
    let models = zip::files(bytes, |n| n.to_ascii_lowercase().ends_with(".model"))?;
    if models.is_empty() {
        return Err(io("the 3MF file has no model"));
    }
    let mut out = TriMesh::default();
    let mut group = 0;
    for (_, xml) in models {
        let xml = String::from_utf8_lossy(&xml);
        let scale = match attr(tags(&xml, "model").first().copied().unwrap_or(""), "unit") {
            Some("micron") => 0.001,
            Some("centimeter") => 10.0,
            Some("meter") => 1000.0,
            Some("inch") => 25.4,
            Some("foot") => 304.8,
            _ => 1.0,
        };
        // Each <mesh> in its own group.
        for mesh in xml.split("<mesh").skip(1) {
            let body = mesh.split("</mesh>").next().unwrap_or("");
            let mut part = TriMesh::default();
            for v in tags(body, "vertex") {
                let c = |n| attr(v, n).and_then(|s| s.parse::<f64>().ok());
                let (Some(x), Some(y), Some(z)) = (c("x"), c("y"), c("z")) else {
                    return Err(io("a 3MF vertex is missing a coordinate"));
                };
                part.positions.push(DVec3::new(x, y, z) * scale);
            }
            for t in tags(body, "triangle") {
                let c = |n| attr(t, n).and_then(|s| s.parse::<u32>().ok());
                let (Some(a), Some(b), Some(c)) = (c("v1"), c("v2"), c("v3")) else {
                    return Err(io("a 3MF triangle is missing a vertex"));
                };
                part.triangles.push([a, b, c]);
            }
            part.groups = vec![0; part.triangles.len()];
            if group == 0 {
                out = part;
            } else {
                out.append(&part);
            }
            group += 1;
        }
    }
    if group <= 1 {
        out.groups.clear();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_triangle_in_centimetres() {
        let xml = br#"<?xml version="1.0"?><model unit="centimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02"><resources><object id="1" type="model"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources></model>"#;
        let z = super::zip::store(&[("3D/3dmodel.model", xml)], true);
        let m = super::read(&z).unwrap();
        assert!((m.area() - 50.0).abs() < 1e-9);
    }
}

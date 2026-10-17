//! AMF: XML (possibly zipped) with vertices and triangle volumes.

use glam::DVec3;

use super::{io, tags, texts, zip};
use crate::{Result, TriMesh};

pub fn read(bytes: &[u8]) -> Result<TriMesh> {
    let xml = if bytes.starts_with(b"PK") {
        zip::files(bytes, |_| true)?
            .into_iter()
            .next()
            .map(|(_, b)| b)
            .ok_or_else(|| io("the AMF archive is empty"))?
    } else {
        bytes.to_vec()
    };
    let xml = String::from_utf8_lossy(&xml);
    let scale = match tags(&xml, "amf")
        .first()
        .and_then(|t| super::attr(t, "unit"))
    {
        Some("micron") => 0.001,
        Some("inch") => 25.4,
        Some("feet") => 304.8,
        Some("meter") => 1000.0,
        _ => 1.0,
    };
    let mut out = TriMesh::default();
    let mut objects = 0;
    for object in xml.split("<object").skip(1) {
        let body = object.split("</object>").next().unwrap_or("");
        let mut part = TriMesh::default();
        for coords in texts(body, "coordinates") {
            let n = |name| -> Result<f64> {
                texts(coords, name)
                    .first()
                    .and_then(|s| s.trim().parse().ok())
                    .ok_or_else(|| io("an AMF vertex is missing a coordinate"))
            };
            part.positions
                .push(DVec3::new(n("x")?, n("y")?, n("z")?) * scale);
        }
        for tri in texts(body, "triangle") {
            let n = |name| -> Result<u32> {
                texts(tri, name)
                    .first()
                    .and_then(|s| s.trim().parse().ok())
                    .ok_or_else(|| io("an AMF triangle is missing a vertex"))
            };
            part.triangles.push([n("v1")?, n("v2")?, n("v3")?]);
        }
        part.groups = vec![0; part.triangles.len()];
        if objects == 0 {
            out = part;
        } else {
            out.append(&part);
        }
        objects += 1;
    }
    if objects <= 1 {
        out.groups.clear();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn plain_amf() {
        let xml = "<amf unit=\"millimeter\"><object id=\"0\"><mesh><vertices><vertex><coordinates><x>0</x><y>0</y><z>0</z></coordinates></vertex><vertex><coordinates><x>2</x><y>0</y><z>0</z></coordinates></vertex><vertex><coordinates><x>0</x><y>2</y><z>0</z></coordinates></vertex></vertices><volume><triangle><v1>0</v1><v2>1</v2><v3>2</v3></triangle></volume></mesh></object></amf>";
        let m = super::read(xml.as_bytes()).unwrap();
        assert!((m.area() - 2.0).abs() < 1e-12);
    }
}

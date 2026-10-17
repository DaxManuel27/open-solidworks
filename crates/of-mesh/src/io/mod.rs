//! Mesh file formats: STL, OBJ, OFF, PLY, 3MF and AMF, and point clouds
//! (XYZ, TXT, ASC).

mod amf;
mod obj;
mod off;
mod ply;
mod stl;
mod threemf;
mod zip;

use glam::DVec3;

use crate::{MeshError, Result, TriMesh};

pub use stl::write_stl;

/// A mesh file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Stl,
    Obj,
    Off,
    Ply,
    ThreeMf,
    Amf,
}

impl Format {
    /// Guess from a file extension.
    pub fn from_extension(ext: &str) -> Option<Format> {
        Some(match ext.to_ascii_lowercase().as_str() {
            "stl" => Format::Stl,
            "obj" => Format::Obj,
            "off" => Format::Off,
            "ply" => Format::Ply,
            "3mf" => Format::ThreeMf,
            "amf" => Format::Amf,
            _ => return None,
        })
    }
}

/// Read a mesh from a file's bytes.
pub fn read(format: Format, bytes: &[u8]) -> Result<TriMesh> {
    let mesh = match format {
        Format::Stl => stl::read(bytes)?,
        Format::Obj => obj::read(bytes)?,
        Format::Off => off::read(bytes)?,
        Format::Ply => ply::read(bytes)?,
        Format::ThreeMf => threemf::read(bytes)?,
        Format::Amf => amf::read(bytes)?,
    };
    mesh.validate()?;
    if mesh.triangles.is_empty() {
        return Err(MeshError::Invalid("the file has no triangles".into()));
    }
    Ok(mesh)
}

/// Read a point cloud: one point per line, `x y z` (commas or spaces),
/// ignoring anything after the third number and lines that don't start with
/// three numbers.
pub fn read_points(bytes: &[u8]) -> Result<Vec<DVec3>> {
    let text = std::str::from_utf8(bytes).map_err(|_| io("the point file isn't text"))?;
    let mut out = Vec::new();
    for line in text.lines() {
        let nums: Vec<f64> = line
            .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .filter(|s| !s.is_empty())
            .take(3)
            .map_while(|s| s.parse().ok())
            .collect();
        if nums.len() == 3 && nums.iter().all(|v| v.is_finite()) {
            out.push(DVec3::new(nums[0], nums[1], nums[2]));
        }
    }
    if out.is_empty() {
        return Err(io("the file has no points"));
    }
    Ok(out)
}

pub(crate) fn io(msg: impl Into<String>) -> MeshError {
    MeshError::Io(msg.into())
}

/// Triangulate a polygon (vertex indices) as a fan from its first vertex.
pub(crate) fn fan(poly: &[u32], out: &mut Vec<[u32; 3]>) {
    for i in 1..poly.len().saturating_sub(1) {
        out.push([poly[0], poly[i], poly[i + 1]]);
    }
}

/// The value of attribute `name` in an XML start tag's text.
pub(crate) fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag;
    while let Some(i) = rest.find(name) {
        let before = rest[..i].chars().last();
        let after = &rest[i + name.len()..];
        let after_trim = after.trim_start();
        if before.is_none_or(|c| c.is_whitespace()) && after_trim.starts_with('=') {
            let v = after_trim[1..].trim_start();
            let quote = v.chars().next()?;
            if quote == '"' || quote == '\'' {
                let end = v[1..].find(quote)?;
                return Some(&v[1..1 + end]);
            }
        }
        rest = &rest[i + name.len()..];
    }
    None
}

/// Every start tag named `name` in an XML document, as its attribute text.
pub(crate) fn tags<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let open = format!("<{name}");
    let mut rest = xml;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i + open.len()..];
        let next = after.chars().next();
        if next.is_some_and(|c| c.is_whitespace() || c == '>' || c == '/') {
            if let Some(end) = after.find('>') {
                out.push(&after[..end]);
                rest = &after[end..];
                continue;
            }
        }
        rest = after;
    }
    out
}

/// The text inside each `<name>…</name>` element.
pub(crate) fn texts<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let open = format!("<{name}");
    let close = format!("</{name}>");
    let mut rest = xml;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i + open.len()..];
        let Some(gt) = after.find('>') else { break };
        if after[..gt].ends_with('/') {
            rest = &after[gt..];
            continue;
        }
        let body = &after[gt + 1..];
        let Some(end) = body.find(&close) else { break };
        out.push(&body[..end]);
        rest = &body[end..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_helpers() {
        let xml = r#"<a><vertex x="1" y='2.5' z="-3"/><vertex x="4" y="5" z="6"></vertex></a>"#;
        let v = tags(xml, "vertex");
        assert_eq!(v.len(), 2);
        assert_eq!(attr(v[0], "y"), Some("2.5"));
        assert_eq!(attr(v[1], "x"), Some("4"));
        let xml = "<x><v1>0</v1><v1>7</v1></x>";
        assert_eq!(texts(xml, "v1"), vec!["0", "7"]);
    }

    #[test]
    fn point_clouds() {
        let p = read_points(b"# header\n1 2 3\n4,5,6 0.1\nbad\n").unwrap();
        assert_eq!(
            p,
            vec![DVec3::new(1.0, 2.0, 3.0), DVec3::new(4.0, 5.0, 6.0)]
        );
    }
}

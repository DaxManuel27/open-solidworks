//! PLY, ASCII and binary (either byte order), with vertex colours.

use glam::DVec3;

use super::{fan, io};
use crate::{Result, TriMesh};

#[derive(Debug, Clone, Copy)]
enum Ty {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
}

impl Ty {
    fn parse(s: &str) -> Result<Ty> {
        Ok(match s {
            "char" | "int8" => Ty::I8,
            "uchar" | "uint8" => Ty::U8,
            "short" | "int16" => Ty::I16,
            "ushort" | "uint16" => Ty::U16,
            "int" | "int32" => Ty::I32,
            "uint" | "uint32" => Ty::U32,
            "float" | "float32" => Ty::F32,
            "double" | "float64" => Ty::F64,
            _ => return Err(io(format!("unknown PLY type {s}"))),
        })
    }

    fn size(self) -> usize {
        match self {
            Ty::I8 | Ty::U8 => 1,
            Ty::I16 | Ty::U16 => 2,
            Ty::I32 | Ty::U32 | Ty::F32 => 4,
            Ty::F64 => 8,
        }
    }
}

#[derive(Debug, Clone)]
struct Property {
    name: String,
    ty: Ty,
    /// For list properties, the type of the count.
    list: Option<Ty>,
}

#[derive(Debug, Clone)]
struct Element {
    name: String,
    count: usize,
    props: Vec<Property>,
}

#[derive(Clone, Copy, PartialEq)]
enum Encoding {
    Ascii,
    Little,
    Big,
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
    enc: Encoding,
    words: Option<std::vec::IntoIter<&'a str>>,
}

impl Reader<'_> {
    fn value(&mut self, ty: Ty) -> Result<f64> {
        if self.enc == Encoding::Ascii {
            return self
                .words
                .as_mut()
                .and_then(|w| w.next())
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| io("the PLY data ends early"));
        }
        let n = ty.size();
        let bytes = self
            .data
            .get(self.at..self.at + n)
            .ok_or_else(|| io("the PLY data ends early"))?;
        self.at += n;
        let mut b = [0u8; 8];
        b[..n].copy_from_slice(bytes);
        if self.enc == Encoding::Big {
            b[..n].reverse();
        }
        Ok(match ty {
            Ty::I8 => b[0] as i8 as f64,
            Ty::U8 => b[0] as f64,
            Ty::I16 => i16::from_le_bytes([b[0], b[1]]) as f64,
            Ty::U16 => u16::from_le_bytes([b[0], b[1]]) as f64,
            Ty::I32 => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
            Ty::U32 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
            Ty::F32 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
            Ty::F64 => f64::from_le_bytes(b),
        })
    }
}

pub fn read(bytes: &[u8]) -> Result<TriMesh> {
    let end = bytes
        .windows(10)
        .position(|w| w == b"end_header")
        .ok_or_else(|| io("not a PLY file"))?;
    let header = std::str::from_utf8(&bytes[..end]).map_err(|_| io("the PLY header isn't text"))?;
    let mut body = end + 10;
    while body < bytes.len() && (bytes[body] == b'\r' || bytes[body] == b'\n') {
        body += 1;
        if bytes[body - 1] == b'\n' {
            break;
        }
    }
    let mut enc = None;
    let mut elements: Vec<Element> = Vec::new();
    for line in header.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        match w.as_slice() {
            ["format", f, ..] => {
                enc = Some(match *f {
                    "ascii" => Encoding::Ascii,
                    "binary_little_endian" => Encoding::Little,
                    "binary_big_endian" => Encoding::Big,
                    _ => return Err(io("unknown PLY format")),
                })
            }
            ["element", name, count] => elements.push(Element {
                name: name.to_string(),
                count: count.parse().map_err(|_| io("bad PLY element count"))?,
                props: Vec::new(),
            }),
            ["property", "list", count, ty, name] => elements
                .last_mut()
                .ok_or_else(|| io("a PLY property before any element"))?
                .props
                .push(Property {
                    name: name.to_string(),
                    ty: Ty::parse(ty)?,
                    list: Some(Ty::parse(count)?),
                }),
            ["property", ty, name] => elements
                .last_mut()
                .ok_or_else(|| io("a PLY property before any element"))?
                .props
                .push(Property {
                    name: name.to_string(),
                    ty: Ty::parse(ty)?,
                    list: None,
                }),
            _ => {}
        }
    }
    let enc = enc.ok_or_else(|| io("the PLY header has no format"))?;
    let words = (enc == Encoding::Ascii).then(|| {
        std::str::from_utf8(&bytes[body..])
            .unwrap_or("")
            .split_whitespace()
            .collect::<Vec<_>>()
            .into_iter()
    });
    let mut r = Reader {
        data: bytes,
        at: body,
        enc,
        words,
    };
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut triangles = Vec::new();
    for el in &elements {
        for _ in 0..el.count {
            let mut p = DVec3::ZERO;
            let mut c = [0u8; 3];
            let mut has_color = false;
            for prop in &el.props {
                if let Some(count_ty) = prop.list {
                    let k = r.value(count_ty)? as usize;
                    let mut poly = Vec::with_capacity(k);
                    for _ in 0..k {
                        poly.push(r.value(prop.ty)? as u32);
                    }
                    if el.name == "face" && prop.name.starts_with("vertex_ind") {
                        fan(&poly, &mut triangles);
                    }
                    continue;
                }
                let v = r.value(prop.ty)?;
                if el.name == "vertex" {
                    match prop.name.as_str() {
                        "x" => p.x = v,
                        "y" => p.y = v,
                        "z" => p.z = v,
                        "red" | "green" | "blue" => {
                            has_color = true;
                            let i = ["red", "green", "blue"]
                                .iter()
                                .position(|n| *n == prop.name)
                                .unwrap();
                            c[i] = match prop.ty {
                                Ty::F32 | Ty::F64 => (v.clamp(0.0, 1.0) * 255.0).round() as u8,
                                _ => v.clamp(0.0, 255.0) as u8,
                            };
                        }
                        _ => {}
                    }
                }
            }
            if el.name == "vertex" {
                positions.push(p);
                if has_color {
                    colors.push(c);
                }
            }
        }
    }
    let mut m = TriMesh::new(positions, triangles);
    if !colors.is_empty() && colors.len() == m.positions.len() {
        m.colors = Some(colors);
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    #[test]
    fn ascii_and_binary() {
        let ascii = "ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nproperty uchar red\nproperty uchar green\nproperty uchar blue\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n0 0 0 255 0 0\n1 0 0 0 255 0\n0 1 0 0 0 255\n3 0 1 2\n";
        let m = super::read(ascii.as_bytes()).unwrap();
        assert_eq!(m.triangles.len(), 1);
        assert_eq!(m.colors.as_ref().unwrap()[1], [0, 255, 0]);
        let mut bin = b"ply\nformat binary_little_endian 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n".to_vec();
        for p in [[0.0f32, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]] {
            for c in p {
                bin.extend(c.to_le_bytes());
            }
        }
        bin.push(3);
        for i in [0i32, 1, 2] {
            bin.extend(i.to_le_bytes());
        }
        let m = super::read(&bin).unwrap();
        assert!((m.area() - 2.0).abs() < 1e-9);
    }
}

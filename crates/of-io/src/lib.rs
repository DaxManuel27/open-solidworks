//! Exchange formats: STEP, IGES and STL.

use std::path::Path;

use base64::Engine;
use of_geom::Shape;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "format", rename_all = "lowercase")]
pub enum ExportFormat {
    Step,
    Iges,
    Stl {
        /// Write text instead of binary STL.
        #[serde(default)]
        ascii: bool,
        /// Maximum distance between the facets and the true surface, mm.
        #[serde(default = "default_deviation")]
        deviation: f64,
        /// Maximum angle between adjacent facets, radians.
        #[serde(default = "default_angle")]
        angle: f64,
    },
}

fn default_deviation() -> f64 {
    0.01
}

fn default_angle() -> f64 {
    0.2
}

impl ExportFormat {
    /// Guess from a file extension.
    pub fn from_path(path: &Path) -> Option<ExportFormat> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "step" | "stp" => ExportFormat::Step,
            "iges" | "igs" => ExportFormat::Iges,
            "stl" => ExportFormat::Stl {
                ascii: false,
                deviation: default_deviation(),
                angle: default_angle(),
            },
            _ => return None,
        })
    }
}

pub fn export(shape: &Shape, path: &Path, format: ExportFormat) -> Result<(), String> {
    if shape.is_null() {
        return Err("there's nothing to export".into());
    }
    let p = path.to_str().ok_or("the file path isn't valid UTF-8")?;
    match format {
        ExportFormat::Step => shape.write_step(p),
        ExportFormat::Iges => shape.write_iges(p),
        ExportFormat::Stl {
            ascii,
            deviation,
            angle,
        } => shape.write_stl(p, deviation, angle, ascii),
    }
    .map_err(|e| e.0)
}

/// Formats that can be imported as a body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportKind {
    Step,
    Iges,
}

/// A file read for import: its kind, base64 contents and file name.
pub struct ImportData {
    pub kind: ImportKind,
    pub data: String,
    pub source: String,
}

pub fn read_import(path: &Path) -> Result<ImportData, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let kind = match ext.as_str() {
        "step" | "stp" => ImportKind::Step,
        "iges" | "igs" => ImportKind::Iges,
        _ => {
            return Err(
                "OpenForge can import STEP (.step, .stp) and IGES (.iges, .igs) files".into(),
            );
        }
    };
    let bytes =
        std::fs::read(path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
    // Check it parses before storing it in the part.
    let check = match kind {
        ImportKind::Step => Shape::read_step(path.to_str().unwrap_or_default()),
        ImportKind::Iges => Shape::read_iges(path.to_str().unwrap_or_default()),
    };
    check.map_err(|e| e.0)?;
    Ok(ImportData {
        kind,
        data: base64::engine::general_purpose::STANDARD.encode(bytes),
        source: path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("import")
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_each_format() {
        let b = Shape::make_box(10.0, 10.0, 10.0).unwrap();
        let dir = std::env::temp_dir();
        for name in ["of-io-test.step", "of-io-test.igs", "of-io-test.stl"] {
            let p = dir.join(name);
            export(&b, &p, ExportFormat::from_path(&p).unwrap()).unwrap();
            assert!(std::fs::metadata(&p).unwrap().len() > 100, "{name}");
        }
        let imported = read_import(&dir.join("of-io-test.step")).unwrap();
        assert_eq!(imported.kind, ImportKind::Step);
        assert!(read_import(&dir.join("of-io-test.stl")).is_err());
    }
}

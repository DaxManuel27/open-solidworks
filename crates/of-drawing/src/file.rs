//! The `.ofd` drawing file: the document as JSON, with a format header.

use serde::{Deserialize, Serialize};

use crate::DrawingDocument;

pub const FORMAT: &str = "openforge-drawing";
pub const VERSION: u32 = 1;
pub const EXTENSION: &str = "ofd";

#[derive(Serialize)]
struct DrawingFile<'a> {
    format: &'a str,
    version: u32,
    document: &'a DrawingDocument,
}

pub fn to_string(doc: &DrawingDocument) -> String {
    serde_json::to_string_pretty(&DrawingFile {
        format: FORMAT,
        version: VERSION,
        document: doc,
    })
    .expect("documents always serialize")
}

pub fn from_str(text: &str) -> Result<DrawingDocument, String> {
    // Decode the envelope first so a newer file reports its version.
    #[derive(Deserialize)]
    struct Envelope {
        format: String,
        version: u32,
        document: serde_json::Value,
    }
    let f: Envelope =
        serde_json::from_str(text).map_err(|e| format!("not a valid drawing file: {e}"))?;
    if f.format != FORMAT {
        return Err("not an OpenForge drawing file".into());
    }
    if f.version > VERSION {
        return Err(format!(
            "this drawing was saved by a newer OpenForge (file version {})",
            f.version
        ));
    }
    if f.version == 0 {
        return Err("unsupported drawing file version 0".into());
    }
    let doc: DrawingDocument = serde_json::from_value(f.document)
        .map_err(|e| format!("not a valid version {} drawing: {e}", f.version))?;
    doc.validate()
        .map_err(|e| format!("invalid drawing: {e}"))?;
    Ok(doc)
}

pub fn save(doc: &DrawingDocument, path: &std::path::Path) -> Result<(), String> {
    std::fs::write(path, to_string(doc))
        .map_err(|e| format!("could not save {}: {e}", path.display()))
}

pub fn load(path: &std::path::Path) -> Result<DrawingDocument, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("could not open {}: {e}", path.display()))?;
    from_str(&text)
}

/// Whether a path names a drawing file.
pub fn is_drawing(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION))
}

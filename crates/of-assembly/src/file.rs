//! The `.ofa` assembly file: the document as JSON, with a format header.

use serde::{Deserialize, Serialize};

use crate::AssemblyDocument;

pub const FORMAT: &str = "openforge-assembly";
pub const VERSION: u32 = 1;
pub const EXTENSION: &str = "ofa";

#[derive(Serialize)]
struct AssemblyFile<'a> {
    format: &'a str,
    version: u32,
    document: &'a AssemblyDocument,
}

pub fn to_string(doc: &AssemblyDocument) -> String {
    serde_json::to_string_pretty(&AssemblyFile {
        format: FORMAT,
        version: VERSION,
        document: doc,
    })
    .expect("documents always serialize")
}

pub fn from_str(text: &str) -> Result<AssemblyDocument, String> {
    // Decode the envelope first so a newer file reports its version.
    #[derive(Deserialize)]
    struct Envelope {
        format: String,
        version: u32,
        document: serde_json::Value,
    }
    let f: Envelope =
        serde_json::from_str(text).map_err(|e| format!("not a valid assembly file: {e}"))?;
    if f.format != FORMAT {
        return Err("not an OpenForge assembly file".into());
    }
    if f.version > VERSION {
        return Err(format!(
            "this assembly was saved by a newer OpenForge (file version {})",
            f.version
        ));
    }
    if f.version == 0 {
        return Err("unsupported assembly file version 0".into());
    }
    let doc: AssemblyDocument = serde_json::from_value(f.document)
        .map_err(|e| format!("not a valid version {} assembly: {e}", f.version))?;
    doc.validate()
        .map_err(|e| format!("invalid assembly: {e}"))?;
    Ok(doc)
}

pub fn save(doc: &AssemblyDocument, path: &std::path::Path) -> Result<(), String> {
    std::fs::write(path, to_string(doc))
        .map_err(|e| format!("could not save {}: {e}", path.display()))
}

pub fn load(path: &std::path::Path) -> Result<AssemblyDocument, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("could not open {}: {e}", path.display()))?;
    from_str(&text)
}

/// Whether a path names an assembly file.
pub fn is_assembly(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION))
}

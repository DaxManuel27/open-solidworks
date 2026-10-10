//! The `.ofp` part file: the document as JSON, with a format header.

use serde::{Deserialize, Serialize};

use crate::Document;

pub const FORMAT: &str = "openforge-part";
pub const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct PartFile {
    format: String,
    version: u32,
    document: Document,
}

pub fn to_string(doc: &Document) -> String {
    let f = PartFile {
        format: FORMAT.into(),
        version: VERSION,
        document: doc.clone(),
    };
    serde_json::to_string_pretty(&f).expect("documents always serialize")
}

pub fn from_str(text: &str) -> Result<Document, String> {
    let f: PartFile =
        serde_json::from_str(text).map_err(|e| format!("not a valid part file: {e}"))?;
    if f.format != FORMAT {
        return Err("not an OpenForge part file".into());
    }
    if f.version > VERSION {
        return Err(format!(
            "this part was saved by a newer OpenForge (file version {})",
            f.version
        ));
    }
    Ok(f.document)
}

pub fn save(doc: &Document, path: &std::path::Path) -> Result<(), String> {
    std::fs::write(path, to_string(doc))
        .map_err(|e| format!("could not save {}: {e}", path.display()))
}

pub fn load(path: &std::path::Path) -> Result<Document, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("could not open {}: {e}", path.display()))?;
    from_str(&text)
}

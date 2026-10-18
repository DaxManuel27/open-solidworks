//! The `.ofp` part file: the document as JSON, with a format header.

use serde::{Deserialize, Serialize};

use crate::Document;

pub const FORMAT: &str = "openforge-part";
pub const VERSION: u32 = 5;

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
    // Decode the envelope first: a future feature must not hide the version error.
    #[derive(Deserialize)]
    struct Envelope {
        format: String,
        version: u32,
        document: serde_json::Value,
    }
    let f: Envelope =
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
    if f.version == 0 {
        return Err("unsupported part file version 0".into());
    }
    // V1 has no Phase 2 fields, V3 no custom properties and V4 no drawing
    // display of dimensions (every dimension is shown). Their serde defaults
    // are the migration contract; retain every existing feature ID, authored
    // parameter and topology name.
    let doc: Document = match f.version {
        1..=5 => serde_json::from_value(f.document)
            .map_err(|e| format!("not a valid version {} document: {e}", f.version))?,
        _ => unreachable!("newer versions were rejected above"),
    };
    doc.validate()
        .map_err(|e| format!("invalid document: {e}"))?;
    for feature in &doc.features {
        if let crate::FeatureKind::Sketch3d(s) = &feature.kind {
            s.validate()
                .map_err(|e| format!("invalid 3D sketch {}: {e}", feature.id))?;
        }
        if let crate::FeatureKind::Sketch(s) = &feature.kind {
            s.sketch
                .validate()
                .map_err(|e| format!("invalid sketch {}: {e}", feature.id))?;
        }
    }
    Ok(doc)
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

//! Loaded part and subassembly files, built once per configuration and
//! reloaded when their contents change on disk.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;

use of_model::geom::Shape;
use of_model::{Document, Model, State};

use crate::AssemblyDocument;

/// FNV-1a of a file's bytes, as hex.
pub fn content_hash(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// A part file built in one configuration.
pub struct Part {
    pub path: PathBuf,
    pub hash: String,
    pub configuration: Option<u32>,
    pub model: Model,
    /// Feature errors, joined, if any feature failed to build.
    pub error: Option<String>,
}

impl Part {
    pub fn doc(&self) -> &Document {
        &self.model.doc
    }

    pub fn state(&self) -> &State {
        self.model.state()
    }

    /// The part's solid geometry, if it has any.
    pub fn shape(&self) -> Option<&Shape> {
        self.state().body.as_ref().map(|b| &b.shape)
    }

    /// File name without the extension.
    pub fn stem(&self) -> String {
        self.path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("part")
            .to_string()
    }
}

/// A subassembly file.
pub struct SubAssembly {
    pub path: PathBuf,
    pub hash: String,
    pub doc: AssemblyDocument,
}

struct Stamp {
    modified: Option<SystemTime>,
    len: u64,
    hash: String,
}

#[derive(Default)]
pub struct Cache {
    parts: HashMap<(PathBuf, Option<u32>), Rc<Part>>,
    assemblies: HashMap<PathBuf, Rc<SubAssembly>>,
    stamps: HashMap<PathBuf, Stamp>,
}

/// Canonical form of a path for cache keys; falls back to the path itself.
pub fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

impl Cache {
    /// The file's content hash, rehashing only when its size or time changed.
    pub fn hash(&mut self, path: &Path) -> Result<String, String> {
        let meta = std::fs::metadata(path).map_err(|_| format!("{} is missing", path.display()))?;
        let modified = meta.modified().ok();
        if let Some(s) = self.stamps.get(path)
            && s.modified == modified
            && s.len == meta.len()
        {
            return Ok(s.hash.clone());
        }
        let bytes =
            std::fs::read(path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
        let hash = content_hash(&bytes);
        self.stamps.insert(
            path.to_path_buf(),
            Stamp {
                modified,
                len: meta.len(),
                hash: hash.clone(),
            },
        );
        Ok(hash)
    }

    /// A part built in a configuration (`None`: the file's active one).
    pub fn part(&mut self, path: &Path, configuration: Option<u32>) -> Result<Rc<Part>, String> {
        let path = canonical(path);
        let hash = self.hash(&path)?;
        let key = (path.clone(), configuration);
        if let Some(p) = self.parts.get(&key)
            && p.hash == hash
        {
            return Ok(p.clone());
        }
        let mut doc = of_model::file::load(&path)?;
        if let Some(c) = configuration {
            if !doc.configurations.items.iter().any(|i| i.id == c) {
                return Err(format!(
                    "{} has no configuration {c}",
                    path.file_name().and_then(|n| n.to_str()).unwrap_or("part")
                ));
            }
            doc.configurations.active = c;
        }
        let mut model = Model::new(doc);
        model.rebuild();
        let errors: Vec<String> = model
            .statuses()
            .iter()
            .filter(|s| !s.rolled_back && !s.suppressed)
            .filter_map(|s| s.error.clone())
            .collect();
        let part = Rc::new(Part {
            path,
            hash,
            configuration,
            model,
            error: (!errors.is_empty()).then(|| errors.join("; ")),
        });
        self.parts.insert(key, part.clone());
        Ok(part)
    }

    pub fn assembly(&mut self, path: &Path) -> Result<Rc<SubAssembly>, String> {
        let path = canonical(path);
        let hash = self.hash(&path)?;
        if let Some(a) = self.assemblies.get(&path)
            && a.hash == hash
        {
            return Ok(a.clone());
        }
        let doc = crate::file::load(&path)?;
        let sub = Rc::new(SubAssembly {
            path: path.clone(),
            hash,
            doc,
        });
        self.assemblies.insert(path, sub.clone());
        Ok(sub)
    }

    /// Forget everything about a file so it reloads on next use.
    pub fn forget(&mut self, path: &Path) {
        let path = canonical(path);
        self.stamps.remove(&path);
        self.parts.retain(|(p, _), _| *p != path);
        self.assemblies.remove(&path);
    }
}

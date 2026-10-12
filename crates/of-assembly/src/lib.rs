//! Assemblies: components placed by mates, patterns and mirrors, with
//! explode steps, interference checks, mass properties and bills of
//! materials. Parts are loaded from their `.ofp` files and built once per
//! configuration; mates are solved by `of-mate`.

pub mod analysis;
mod assembly;
pub mod bom;
pub mod cache;
pub mod document;
pub mod exchange;
pub mod explode;
pub mod file;
pub mod geometry;
pub mod mates;
pub mod patterns;

pub use assembly::{
    Assembly, Diagnostics, DragTarget, Instance, Loaded, mate_base_name, mate_components,
    mate_refs, relative_to, resolve_file,
};
pub use document::*;
pub use of_mate::{self as solver, DQuat, DVec3, Pose};

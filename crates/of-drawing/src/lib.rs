//! Drawings: sheets with views of parts and assemblies, annotations and
//! tables, rendered into a 2D display list and written as PDF or DXF.

pub mod annot;
pub mod annotations;
pub mod clip;
pub mod dims;
pub mod document;
pub mod drawing;
pub mod dxf;
pub mod file;
pub mod fits;
pub mod formats;
pub mod graphics;
pub mod hatch;
pub mod layout;
pub mod locate;
pub mod model;
pub mod pdf;
pub mod section;
pub mod standard;
pub mod symbols;
pub mod tables;
pub mod text;
pub mod viewdraw;
pub mod views;

pub use document::*;

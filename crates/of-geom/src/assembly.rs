//! Placing shapes for assemblies, interference volumes, and STEP files with
//! product structure.

use cxx::UniquePtr;
use glam::{DAffine3, DMat3, DVec3};
use of_kernel_sys::ffi;

use crate::{Result, Shape};

/// Row-major 3x4 matrix of a rigid transform.
fn rows(t: &DAffine3) -> [f64; 12] {
    let m = t.matrix3;
    let p = t.translation;
    [
        m.x_axis.x, m.y_axis.x, m.z_axis.x, p.x, //
        m.x_axis.y, m.y_axis.y, m.z_axis.y, p.y, //
        m.x_axis.z, m.y_axis.z, m.z_axis.z, p.z,
    ]
}

fn affine(m: &[f64]) -> DAffine3 {
    if m.len() != 12 {
        return DAffine3::IDENTITY;
    }
    DAffine3 {
        matrix3: DMat3::from_cols(
            DVec3::new(m[0], m[4], m[8]),
            DVec3::new(m[1], m[5], m[9]),
            DVec3::new(m[2], m[6], m[10]),
        ),
        translation: DVec3::new(m[3], m[7], m[11]),
    }
}

impl Shape {
    /// This shape placed by a rigid transform, sharing its geometry. Unlike
    /// [`Shape::transform`] nothing is copied and no history is kept.
    pub fn located(&self, t: &DAffine3) -> Result<Shape> {
        Ok(Shape(ffi::located(&self.0, &rows(t))?))
    }

    /// Volume this solid shares with another, 0 when their boxes are apart.
    pub fn common_volume(&self, other: &Shape) -> Result<f64> {
        Ok(ffi::common_volume(&self.0, &other.0)?)
    }

    /// The material two solids share.
    pub fn common(&self, other: &Shape) -> Result<Shape> {
        Ok(Shape(ffi::common_shape(&self.0, &other.0)?))
    }
}

/// Builds a STEP file in which parts are defined once and placed as named
/// instances in (sub)assemblies.
pub struct StepAssemblyWriter(UniquePtr<ffi::StepAssembly>);

/// Label of the root assembly in a [`StepAssemblyWriter`].
pub const STEP_ROOT: u32 = 0;

impl StepAssemblyWriter {
    pub fn new(name: &str) -> Result<StepAssemblyWriter> {
        Ok(StepAssemblyWriter(ffi::step_assembly_new(name)?))
    }

    /// Define a part; returns its label.
    pub fn part(&mut self, name: &str, shape: &Shape) -> Result<u32> {
        Ok(ffi::step_assembly_part(self.0.pin_mut(), name, &shape.0)?)
    }

    /// Define an empty subassembly; returns its label.
    pub fn assembly(&mut self, name: &str) -> Result<u32> {
        Ok(ffi::step_assembly_node(self.0.pin_mut(), name)?)
    }

    /// Place `child` in `parent` as a named instance.
    pub fn place(&mut self, parent: u32, child: u32, name: &str, t: &DAffine3) -> Result<()> {
        Ok(ffi::step_assembly_place(
            self.0.pin_mut(),
            parent,
            child,
            name,
            &rows(t),
        )?)
    }

    pub fn write(mut self, path: &str) -> Result<()> {
        Ok(ffi::step_assembly_write(self.0.pin_mut(), path)?)
    }
}

/// One instance read from a STEP product structure.
pub struct StepInstance {
    /// Index of the parent instance; `None` for a root.
    pub parent: Option<usize>,
    /// Instance name, or the definition name when the instance has none.
    pub name: String,
    /// Name of the part or assembly definition.
    pub definition: String,
    /// Placement in the parent.
    pub placement: DAffine3,
    /// The part's shape in its own frame; `None` for assemblies.
    pub shape: Option<Shape>,
}

/// Read a STEP file with its product structure, depth first.
pub fn read_step_assembly(path: &str) -> Result<Vec<StepInstance>> {
    let r = ffi::step_read(path)?;
    let mut out = Vec::new();
    for i in 0..ffi::step_read_count(&r) {
        let node = ffi::step_read_node(&r, i)?;
        out.push(StepInstance {
            parent: usize::try_from(node.parent).ok(),
            name: node.name,
            definition: node.part,
            placement: affine(&node.matrix),
            shape: if node.assembly {
                None
            } else {
                Some(Shape(ffi::step_read_shape(&r, i)?))
            },
        });
    }
    Ok(out)
}

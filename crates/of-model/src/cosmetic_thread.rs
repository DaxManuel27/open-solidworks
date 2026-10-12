//! Lightweight thread annotations on complete cylindrical faces. These do not
//! generate a solid or remove material; dimensions are explicit, without a
//! standards catalogue or implied manufacturing profile.

use crate::{BodyId, Ref, Resolved, State, resolve};
use of_geom::{CurveKind, DVec3, Kind, SurfaceKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CosmeticThread {
    /// Persistent cylindrical face reference. The face must be bounded by two
    /// complete coaxial circles; trimmed or interrupted cylinders are rejected.
    pub face: Ref,
    /// Cylinder diameter, mm; must match the selected cylindrical surface.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub diameter: f64,
    /// Axial distance per turn, mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub pitch: f64,
    /// Axial annotated length, mm; contained within the selected face.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub length: f64,
    /// Distance from the selected end rim along the cylinder axis, mm.
    #[serde(default)]
    #[schemars(range(min = 0.0))]
    pub start_offset: f64,
    /// Start phase in the cylindrical face's radial frame, radians.
    #[serde(default)]
    pub start_angle: f64,
    /// Start at the opposite rim and advance toward the first rim.
    #[serde(default)]
    pub reverse: bool,
    #[serde(default)]
    pub left_handed: bool,
    /// True selects an inward-facing hole wall; false selects an external boss.
    #[serde(default)]
    pub internal: bool,
}

/// A modeled thread: the same parameters as a cosmetic thread, cut into the
/// cylinder with the ISO 68-1 basic 60° profile. External threads take the face
/// diameter as the major diameter; internal threads take it as the minor diameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Thread {
    /// Persistent cylindrical face reference, bounded by two complete coaxial circles.
    pub face: Ref,
    /// Cylinder diameter, mm; must match the selected cylindrical surface.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub diameter: f64,
    /// Axial distance per turn, mm.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub pitch: f64,
    /// Axial threaded length, mm; contained within the selected face.
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub length: f64,
    /// Distance from the selected end rim along the cylinder axis, mm.
    #[serde(default)]
    #[schemars(range(min = 0.0))]
    pub start_offset: f64,
    /// Start phase in the cylindrical face's radial frame, radians.
    #[serde(default)]
    pub start_angle: f64,
    /// Start at the opposite rim and advance toward the first rim.
    #[serde(default)]
    pub reverse: bool,
    #[serde(default)]
    pub left_handed: bool,
    /// True cuts into an inward-facing hole wall; false into an external boss.
    #[serde(default)]
    pub internal: bool,
}

impl Thread {
    /// The equivalent cosmetic description, which shares validation and framing.
    pub fn cosmetic(&self) -> CosmeticThread {
        CosmeticThread {
            face: self.face.clone(),
            diameter: self.diameter,
            pitch: self.pitch,
            length: self.length,
            start_offset: self.start_offset,
            start_angle: self.start_angle,
            reverse: self.reverse,
            left_handed: self.left_handed,
            internal: self.internal,
        }
    }
}

/// The thread of one tapped hole, recorded by the hole feature.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HoleThread {
    /// Where the thread starts on the hole axis.
    pub start: DVec3,
    /// Unit drilling direction.
    pub axis: DVec3,
    /// Nominal (major) diameter, mm.
    pub diameter: f64,
    pub pitch: f64,
    /// Threaded length through the material, mm.
    pub length: f64,
    /// Size from the hole table, like "M6".
    pub size: String,
}

impl HoleThread {
    /// A right-handed helix at the major diameter for display.
    pub fn points(&self) -> Vec<DVec3> {
        let axis = self.axis.normalize();
        let x = axis.any_orthonormal_vector();
        let y = axis.cross(x);
        let turns = (self.length / self.pitch).min(500.0);
        let samples = (turns * 48.0).ceil().max(16.0) as usize;
        let r = self.diameter / 2.0;
        (0..=samples)
            .map(|i| {
                let f = i as f64 / samples as f64;
                let a = TAU * turns * f;
                self.start + axis * (self.length * f) + (x * a.cos() + y * a.sin()) * r
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Annotation {
    pub body: BodyId,
    pub points: Vec<DVec3>,
    pub axis: DVec3,
    pub start: DVec3,
    pub available_length: f64,
}

/// Resolve against the displayed state so later supported body moves also move
/// the annotation; a deleted or trimmed reference becomes an explicit error.
pub fn annotation(p: &CosmeticThread, state: &State) -> Result<Annotation, String> {
    if ![p.diameter, p.pitch, p.length, p.start_offset, p.start_angle]
        .iter()
        .all(|v| v.is_finite())
        || p.diameter <= 0.0
        || p.pitch <= 0.0
        || p.length <= 0.0
        || p.start_offset < 0.0
    {
        return Err("thread diameter, pitch and length must be positive and finite; start offset must be nonnegative".into());
    }
    let turns = p.length / p.pitch;
    if turns > 500.0 {
        return Err("cosmetic thread display supports at most 500 turns".into());
    }
    let Resolved::Face { index, info } = resolve(state, &p.face)? else {
        return Err("select a cylindrical face for the cosmetic thread".into());
    };
    if info.kind != SurfaceKind::Cylinder {
        return Err("cosmetic threads require a cylindrical face".into());
    }
    let tolerance = (info.radius * 1e-7).max(1e-7);
    if (p.diameter - 2.0 * info.radius).abs() > 2.0 * tolerance {
        return Err(format!(
            "thread diameter must match the cylinder diameter ({} mm)",
            2.0 * info.radius
        ));
    }
    let shape = &state
        .body
        .as_ref()
        .ok_or("the thread body no longer exists")?
        .shape;
    // The surface frame transforms with the body, preserving the chosen end
    // and phase through rotations instead of reorienting to the world axes.
    let axis = info.axis_dir.normalize();
    let radial = info.point - info.axis_origin - axis * (info.point - info.axis_origin).dot(axis);
    if (info.normal.dot(radial) < 0.0) != p.internal {
        return Err(if p.internal {
            "select an internal cylindrical hole wall"
        } else {
            "select an external cylindrical boss face"
        }
        .into());
    }
    let mut rims = Vec::new();
    for edge in &shape.face_edges()[index as usize] {
        let edge = shape.edge_info(*edge).map_err(|e| e.0)?;
        match edge.kind {
            CurveKind::Circle if edge.closed
                && (edge.radius - info.radius).abs() <= tolerance
                && edge.axis.normalize().dot(axis).abs() > 1.0 - 1e-8
                && (edge.length - TAU * info.radius).abs() <= tolerance * 20.0 => {
                let offset = edge.center - info.axis_origin;
                if (offset - axis * offset.dot(axis)).length() > tolerance {
                    return Err("the cylinder boundary circles are not coaxial".into());
                }
                rims.push(offset.dot(axis));
            }
            CurveKind::Line if (edge.end - edge.start).normalize().dot(axis).abs() > 1.0 - 1e-8 => {}
            _ => return Err("cosmetic threads require an uninterrupted cylinder bounded by two complete circles".into()),
        }
    }
    rims.sort_by(f64::total_cmp);
    rims.dedup_by(|a, b| (*a - *b).abs() <= tolerance);
    if rims.len() != 2 {
        return Err("cosmetic threads require two complete circular cylinder boundaries".into());
    }
    let available_length = rims[1] - rims[0];
    if (info.area - TAU * info.radius * available_length).abs() > (info.area * 1e-7).max(1e-7) {
        return Err("the cylindrical face is trimmed or interrupted".into());
    }
    if p.start_offset + p.length > available_length + tolerance {
        return Err(format!(
            "thread start offset plus length exceeds the cylinder length ({available_length} mm)"
        ));
    }
    let rim = info.axis_origin + axis * if p.reverse { rims[1] } else { rims[0] };
    let axis = if p.reverse { -axis } else { axis };
    let start = rim + axis * p.start_offset;
    let seed = if axis.x.abs() < 0.8 {
        DVec3::X
    } else {
        DVec3::Y
    };
    let x = (seed - axis * seed.dot(axis)).normalize();
    let y = axis.cross(x);
    let samples = (turns * 48.0).ceil().max(16.0) as usize;
    let handed = if p.left_handed { -1.0 } else { 1.0 };
    let points = (0..=samples)
        .map(|i| {
            let fraction = i as f64 / samples as f64;
            let angle = p.start_angle + handed * TAU * turns * fraction;
            start + axis * (p.length * fraction) + info.radius * (x * angle.cos() + y * angle.sin())
        })
        .collect();
    let body = state
        .bodies
        .iter()
        .find(|body| {
            body.geometry
                .shape
                .indices_in(shape, Kind::Face)
                .is_ok_and(|faces| faces.contains(&index))
        })
        .ok_or("the cosmetic thread face has no owning body")?
        .id
        .clone();
    Ok(Annotation {
        body,
        points,
        axis,
        start,
        available_length,
    })
}

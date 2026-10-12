//! Measure and mass properties.

use of_geom::{CurveKind, DMat3, DVec3, Kind, SurfaceKind};
use serde::{Deserialize, Serialize};

use crate::build::State;
use crate::materials::DEFAULT_DENSITY;
use crate::resolve::{Resolved, resolve};
use crate::{Document, Ref};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quantity {
    Length,
    Angle,
    Area,
    Coordinate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Value {
    pub label: String,
    pub quantity: Quantity,
    /// Millimetres, radians or square millimetres.
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    pub values: Vec<Value>,
    /// Closest points, for drawing the distance in the viewport.
    pub points: Option<(DVec3, DVec3)>,
}

fn v(label: &str, quantity: Quantity, value: f64) -> Value {
    Value {
        label: label.into(),
        quantity,
        value,
    }
}

/// Measure one or more references.
pub fn measure(state: &State, refs: &[Ref]) -> Result<Measurement, String> {
    let resolved: Vec<Resolved> = refs
        .iter()
        .map(|r| resolve(state, r))
        .collect::<Result<_, _>>()?;
    let mut values = Vec::new();
    let mut points = None;
    match resolved.as_slice() {
        [] => return Err("select something to measure".into()),
        [one] => describe(one, &mut values),
        [a, b] => {
            if let (Some(sa), Some(sb)) = (sub_shape(state, a), sub_shape(state, b)) {
                let d = sa.distance(&sb).map_err(|e| e.0)?;
                values.push(v("Distance", Quantity::Length, d.value));
                for (label, axis) in [("dX", 0), ("dY", 1), ("dZ", 2)] {
                    values.push(v(label, Quantity::Length, (d.b - d.a)[axis].abs()));
                }
                points = Some((d.a, d.b));
            } else if let (Ok(p), Ok(q)) = (a.point(), b.point()) {
                values.push(v("Distance", Quantity::Length, (q - p).length()));
                points = Some((p, q));
            }
            if let (Some(ca), Some(cb)) = (center(a), center(b)) {
                values.push(v("Center distance", Quantity::Length, (cb - ca).length()));
            }
            if let (Ok(da), Ok(db)) = (a.direction(), b.direction()) {
                let ang = da.angle_between(db);
                values.push(v(
                    "Angle",
                    Quantity::Angle,
                    ang.min(std::f64::consts::PI - ang),
                ));
            }
        }
        many => {
            let total_len: f64 = many
                .iter()
                .filter_map(|r| {
                    if let Resolved::Edge { info, .. } = r {
                        Some(info.length)
                    } else {
                        None
                    }
                })
                .sum();
            let total_area: f64 = many
                .iter()
                .filter_map(|r| {
                    if let Resolved::Face { info, .. } = r {
                        Some(info.area)
                    } else {
                        None
                    }
                })
                .sum();
            if total_len > 0.0 {
                values.push(v("Total length", Quantity::Length, total_len));
            }
            if total_area > 0.0 {
                values.push(v("Total area", Quantity::Area, total_area));
            }
        }
    }
    Ok(Measurement { values, points })
}

fn describe(r: &Resolved, out: &mut Vec<Value>) {
    match r {
        Resolved::Edge { info, .. } => {
            out.push(v("Length", Quantity::Length, info.length));
            if info.kind == CurveKind::Circle {
                out.push(v("Radius", Quantity::Length, info.radius));
                out.push(v("Diameter", Quantity::Length, info.radius * 2.0));
                push_point(out, "Center", info.center);
            }
        }
        Resolved::Face { info, .. } => {
            out.push(v("Area", Quantity::Area, info.area));
            if matches!(info.kind, SurfaceKind::Cylinder | SurfaceKind::Sphere) {
                out.push(v("Radius", Quantity::Length, info.radius));
                out.push(v("Diameter", Quantity::Length, info.radius * 2.0));
            }
        }
        other => {
            if let Ok(p) = other.point() {
                push_point(out, "", p);
            }
        }
    }
}

fn push_point(out: &mut Vec<Value>, prefix: &str, p: DVec3) {
    for (axis, val) in ["X", "Y", "Z"].iter().zip([p.x, p.y, p.z]) {
        out.push(v(
            &format!("{prefix}{}{axis}", if prefix.is_empty() { "" } else { " " }),
            Quantity::Coordinate,
            val,
        ));
    }
}

fn sub_shape(state: &State, r: &Resolved) -> Option<of_geom::Shape> {
    if let Resolved::Body { id, .. } = r {
        return Some(state.solid(id).ok()?.geometry.shape.clone());
    }
    let body = state.body.as_ref()?;
    let (kind, i) = match r {
        Resolved::Face { index, .. } => (Kind::Face, *index),
        Resolved::Edge { index, .. } => (Kind::Edge, *index),
        Resolved::Vertex { index, .. } => (Kind::Vertex, *index),
        _ => return None,
    };
    body.shape.sub_shape(kind, i).ok()
}

fn center(r: &Resolved) -> Option<DVec3> {
    match r {
        Resolved::Edge { info, .. } if info.kind == CurveKind::Circle => Some(info.center),
        Resolved::Face { info, .. } if info.kind == SurfaceKind::Sphere => Some(info.axis_origin),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MassProperties {
    pub material: Option<String>,
    /// kg/m³
    pub density: f64,
    /// kg
    pub mass: f64,
    /// mm³
    pub volume: f64,
    /// mm²
    pub area: f64,
    /// mm
    pub center: DVec3,
    /// Inertia tensor about the center of mass, kg·mm².
    pub inertia: DMat3,
    /// kg·mm²
    pub principal_moments: DVec3,
    pub principal_axes: [DVec3; 3],
}

pub fn mass_properties(doc: &Document, state: &State) -> Result<MassProperties, String> {
    let effective = crate::configurations::resolve(doc)?;
    let doc = &effective;
    let body = state.body.as_ref().ok_or("there's no body")?;
    let m = body.shape.mass_props().map_err(|e| e.0)?;
    let density = doc
        .material
        .as_ref()
        .map(|m| m.density)
        .unwrap_or(DEFAULT_DENSITY);
    // Volume integrals are in mm; density per mm³ is density * 1e-9.
    let k = density * 1e-9;
    Ok(MassProperties {
        material: doc.material.as_ref().map(|m| m.name.clone()),
        density,
        mass: m.volume * k,
        volume: m.volume,
        area: m.area,
        center: m.center,
        inertia: m.inertia * k,
        principal_moments: m.principal_moments * k,
        principal_axes: m.principal_axes,
    })
}

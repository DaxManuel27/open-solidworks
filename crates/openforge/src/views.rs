//! JSON views of the model sent to clients.

use of_model::geom::{DVec2, DVec3, Kind, Plane, Shape};
use of_model::sketch::{Definition, Geometry, Sketch, SolveReport};
use of_model::{Datum, Document, FeatureKind, Model, State};
use of_protocol::{MeshPayload, b64_f32, b64_u32};
use serde_json::{Value, json};

/// Display quality: chord deviation relative to model size, and angle.
const ANGULAR: f64 = 0.35;

pub fn mesh(shape: &Shape) -> Result<MeshPayload, String> {
    let size = shape.bbox().map(|b| b.diagonal()).unwrap_or(100.0);
    let m = shape
        .mesh((size * 0.0008).clamp(0.002, 0.5), ANGULAR)
        .map_err(|e| e.0)?;
    let vertices: Vec<f32> = (0..shape.count(Kind::Vertex))
        .filter_map(|i| shape.vertex_point(i).ok())
        .flat_map(|p| [p.x as f32, p.y as f32, p.z as f32])
        .collect();
    Ok(MeshPayload {
        positions: b64_f32(&m.positions),
        normals: b64_f32(&m.normals),
        indices: b64_u32(&m.indices),
        face_ranges: b64_u32(&m.face_tri_start),
        edge_points: b64_f32(&m.edge_points),
        edge_ranges: b64_u32(&m.edge_start),
        vertices: b64_f32(&vertices),
        face_count: shape.count(Kind::Face),
        edge_count: shape.count(Kind::Edge),
    })
}

fn v3(p: DVec3) -> Value {
    json!([p.x, p.y, p.z])
}

fn plane_json(p: &Plane) -> Value {
    json!({ "origin": v3(p.origin), "x_dir": v3(p.x_dir), "normal": v3(p.normal) })
}

/// Everything the viewport draws for a model state.
pub fn scene(doc: &Document, state: &State) -> Result<Value, String> {
    let body = match &state.body {
        Some(b) => Some(mesh(&b.shape)?),
        None => None,
    };
    let bbox = state.body.as_ref().and_then(|b| b.shape.bbox());
    let name = |id: u32| doc.feature(id).map(|f| f.name.clone()).unwrap_or_default();
    let datums: Vec<Value> = state
        .datums
        .iter()
        .map(|(id, d)| {
            let geometry = match d {
                Datum::Plane(p) => json!({ "type": "plane", "plane": plane_json(p) }),
                Datum::Axis { origin, dir } => json!({ "type": "axis", "origin": v3(*origin), "dir": v3(*dir) }),
                Datum::Point(p) => json!({ "type": "point", "point": v3(*p) }),
                Datum::CoordSys { origin, x, y, z } => {
                    json!({ "type": "coord_sys", "origin": v3(*origin), "x": v3(*x), "y": v3(*y), "z": v3(*z) })
                }
            };
            json!({ "id": id, "name": name(*id), "geometry": geometry })
        })
        .collect();
    // Sketches no feature uses yet are shown in 3D.
    let used: Vec<u32> = doc
        .features
        .iter()
        .flat_map(|f| f.kind.depends_on())
        .collect();
    let sketches: Vec<Value> = state
        .sketches
        .iter()
        .filter(|(id, _)| !used.contains(id))
        .map(|(id, (plane, sk))| {
            let polylines: Vec<Vec<f64>> = sk
                .curve_ids()
                .filter_map(|c| sk.curve(c).ok())
                .map(|cv| {
                    cv.sample(0.05)
                        .iter()
                        .flat_map(|p| {
                            let w = plane.to_world(*p);
                            [w.x, w.y, w.z]
                        })
                        .collect()
                })
                .collect();
            json!({ "id": id, "name": name(*id), "plane": plane_json(plane), "polylines": polylines })
        })
        .collect();
    Ok(json!({
        "body": body,
        "bbox": bbox.map(|b| json!({ "min": v3(b.min), "max": v3(b.max) })),
        "datums": datums,
        "sketches": sketches,
    }))
}

fn definition(d: Option<&Definition>) -> &'static str {
    match d {
        Some(Definition::Fully) => "fully",
        Some(Definition::Over) => "over",
        _ => "under",
    }
}

/// A sketch for the sketch editor: positions, sampled curves and statuses.
pub fn sketch(id: u32, plane: &Plane, sk: &Sketch, report: &SolveReport) -> Value {
    let mut points = Vec::new();
    let mut curves = Vec::new();
    for (eid, e) in &sk.entities {
        let status = definition(report.entities.get(eid));
        match &e.geo {
            Geometry::Point { p } => points.push(json!({
                "id": eid, "x": p.x, "y": p.y, "status": status,
                "construction": e.construction, "external": e.external.is_some(),
                "standalone": !sk.entities.values().any(|o| o.point_refs().contains(eid)),
            })),
            geo => {
                let Ok(c) = sk.curve(*eid) else { continue };
                let samples: Vec<[f64; 2]> =
                    c.sample(0.02).iter().map(|p: &DVec2| [p.x, p.y]).collect();
                let (center, radius) = match sk.circle(*eid) {
                    Ok((c, r)) => (Some([c.x, c.y]), Some(r)),
                    Err(_) => (None, None),
                };
                let kind = match geo {
                    Geometry::Line { .. } => "line",
                    Geometry::Circle { .. } => "circle",
                    Geometry::Arc { .. } => "arc",
                    Geometry::Spline { .. } | Geometry::Bezier { .. } => "spline",
                    Geometry::Point { .. } => unreachable!(),
                };
                curves.push(json!({
                    "id": eid, "type": kind, "points": e.point_refs(), "status": status,
                    "construction": e.construction, "external": e.external.is_some(),
                    "samples": samples, "center": center, "radius": radius,
                }));
            }
        }
    }
    let constraints: Vec<Value> = sk
        .constraints
        .iter()
        .map(|(cid, c)| {
            let mut v = serde_json::to_value(c).unwrap();
            let status = if report.conflicting.contains(cid) {
                "conflicting"
            } else if report.redundant.contains(cid) {
                "redundant"
            } else {
                "ok"
            };
            v["id"] = json!(cid);
            v["status"] = json!(status);
            v["refs"] = json!(c.kind.refs());
            v["measured"] = json!(c.measured(sk));
            v
        })
        .collect();
    let profile_error = sk.contours().err().map(|e| e.to_string());
    json!({
        "id": id,
        "plane": plane_json(plane),
        "points": points,
        "curves": curves,
        "constraints": constraints,
        "dof": report.dof,
        "status": report.summary(),
        "solved": report.status == of_model::sketch::SolveStatus::Solved,
        "profile_error": profile_error,
    })
}

/// The feature tree and document settings.
pub fn snapshot(model: &Model) -> Value {
    let statuses = model.statuses();
    let features: Vec<Value> = model
        .doc
        .features
        .iter()
        .zip(&statuses)
        .map(|(f, s)| {
            let mut params = serde_json::to_value(&f.kind).unwrap();
            // Sketch geometry is fetched separately; keep the tree light.
            if let FeatureKind::Sketch(_) = f.kind {
                params["sketch"] = Value::Null;
            }
            json!({
                "id": f.id,
                "name": f.name,
                "type": params["type"].clone(),
                "suppressed": f.suppressed,
                "error": s.error,
                "rolled_back": s.rolled_back,
                "sketch_status": s.sketch.as_ref().map(|r| r.summary()),
                "params": params,
            })
        })
        .collect();
    json!({
        "features": features,
        "rollback": model.doc.rollback,
        "units": model.doc.units,
        "material": model.doc.material,
    })
}

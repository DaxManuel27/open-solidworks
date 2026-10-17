//! JSON views of the model sent to clients.

use of_model::geom::{DVec2, DVec3, Kind, Plane, Shape};
use of_model::sketch::{Definition, Geometry, Sketch, SolveReport};
use of_model::{Datum, Document, FeatureKind, Model, Ref, State};
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
    let body = match state.everything() {
        Some(b) => Some(mesh(&b.shape)?),
        None => None,
    };
    let display_geometry = Shape::compound(
        state
            .everything()
            .into_iter()
            .map(|b| &b.shape)
            .chain(state.curves.values().map(|c| &c.shape)),
    );
    let bbox = display_geometry.bbox();
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
    let curves = state
        .curves
        .iter()
        .map(|(id, curve)| {
            let polylines = (0..curve.shape.count(of_model::geom::Kind::Edge))
                .map(|edge| {
                    curve
                        .shape
                        .edge_samples(edge, 0.05, ANGULAR)
                        .map(|points| {
                            points
                                .iter()
                                .flat_map(|p| [p.x, p.y, p.z])
                                .collect::<Vec<_>>()
                        })
                        .map_err(|e| e.0)
                })
                .collect::<Result<Vec<_>, String>>()?;
            let segments = curve.segments.iter().map(|(segment, shape)| {
                let polylines = (0..shape.count(Kind::Edge)).map(|edge| {
                    shape.edge_samples(edge, 0.05, ANGULAR).map(|points| points.iter()
                        .flat_map(|p| [p.x, p.y, p.z]).collect::<Vec<_>>()).map_err(|e| e.0)
                }).collect::<Result<Vec<_>, _>>()?;
                Ok(json!({ "reference": Ref::CurveSegment { curve: *id, segment: segment.clone() }, "polylines": polylines }))
            }).collect::<Result<Vec<Value>, String>>()?;
            Ok(json!({ "id": id, "name": name(*id), "polylines": polylines, "segments": segments }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    // Cosmetic weld beads, piece by piece.
    let weld_beads: Vec<Value> = state
        .welds
        .iter()
        .filter(|w| w.kind == of_model::weldment::BeadKind::Cosmetic)
        .flat_map(|w| {
            w.pieces.iter().map(move |p| {
                json!({ "feature": w.feature, "polyline": p.iter().flatten().collect::<Vec<_>>(), "size": w.size })
            })
        })
        .collect();
    // Bend lines of flattened sheet metal bodies.
    let bend_lines: Vec<Value> = state
        .sheet_metal
        .iter()
        .filter(|r| r.flat_plane.is_some())
        .flat_map(|r| {
            r.bend_lines.iter().map(|l| {
                json!({ "polyline": [l.a[0], l.a[1], l.a[2], l.b[0], l.b[1], l.b[2]], "up": l.up })
            })
        })
        .collect();
    let sketches3d: Vec<_> = state
        .sketches3d
        .iter()
        .map(|(id, (sk, report))| {
            let polylines: Vec<Vec<f64>> = sk
                .entities
                .iter()
                .filter(|e| {
                    !e.construction
                        && !matches!(
                            e.geometry,
                            of_model::sketch::spatial::Geometry3::Point { .. }
                        )
                })
                .map(|e| {
                    (0..=48)
                        .filter_map(|i| sk.sample(e.id, i as f64 / 48.0).ok())
                        .flat_map(|p| p.to_array())
                        .collect()
                })
                .collect();
            json!({"id":id,"name":name(*id),"polylines":polylines,"sketch":sk,"report":report})
        })
        .collect();
    let mut bounds = bbox.map(|b| (b.min, b.max));
    for (sk, _) in state.sketches3d.values() {
        for e in &sk.entities {
            if let of_model::sketch::spatial::Geometry3::Point { at } = e.geometry {
                bounds = Some(bounds.map_or((at, at), |(lo, hi)| (lo.min(at), hi.max(at))));
            }
        }
    }
    let hole_threads = state.hole_threads.iter().map(|(id, threads)| {
        json!({
            "id": id, "name": name(*id),
            "parameters": {"holes": threads},
            "body": null,
            "hidden": false,
            "polylines": threads
                .iter()
                .map(|t| t.points().iter().flat_map(|p| p.to_array()).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            "error": null,
        })
    });
    let cosmetic_threads: Vec<Value> = state.cosmetic_threads.iter().map(|(id, thread)| {
        match of_model::cosmetic_thread::annotation(thread, state) {
            Ok(annotation) => json!({
                "id": id, "name": name(*id), "parameters": thread,
                "body": annotation.body,
                "hidden": doc.body_display.get(&annotation.body).is_some_and(|d| d.hidden),
                "polylines": [annotation.points.iter().flat_map(|p| p.to_array()).collect::<Vec<_>>()],
                "error": null,
            }),
            Err(error) => json!({"id":id,"name":name(*id),"parameters":thread,"body":null,"hidden":false,"polylines":[],"error":error}),
        }
    }).chain(hole_threads).collect();
    Ok(json!({
        "body": body,
        "bodies": body_views(doc, state)?,
        "bbox": bounds.map(|(lo,hi)| json!({ "min": v3(lo), "max": v3(hi) })),
        "datums": datums,
        "sketches": sketches,
        "curves": curves,
        "sketches3d": sketches3d,
        "cosmetic_threads": cosmetic_threads,
        "bend_lines": bend_lines,
        "weld_beads": weld_beads,
    }))
}

/// A body's display name, or `Body N` by its position.
pub fn body_name(doc: &Document, state: &State, index: usize) -> String {
    doc.body_display
        .get(&state.bodies[index].id)
        .and_then(|d| d.name.clone())
        .unwrap_or_else(|| format!("Body {}", index + 1))
}

/// A surface body's display name, or `Surface-Body N` by its position.
pub fn surface_name(doc: &Document, state: &State, index: usize) -> String {
    doc.body_display
        .get(&state.surfaces[index].id)
        .and_then(|d| d.name.clone())
        .unwrap_or_else(|| format!("Surface-Body {}", index + 1))
}

fn body_views(doc: &Document, state: &State) -> Result<Vec<Value>, String> {
    let Some(whole) = state.everything() else {
        return Ok(Vec::new());
    };
    let solids = state.bodies.iter().enumerate().map(|(i, b)| (b, false, i));
    let surfaces = state.surfaces.iter().enumerate().map(|(i, b)| (b, true, i));
    solids
        .chain(surfaces)
        .map(|(body, surface, i)| {
            let display = doc.body_display.get(&body.id);
            let indices = |kind| {
                body.geometry
                    .shape
                    .indices_in(&whole.shape, kind)
                    .map_err(|e| e.0)
            };
            Ok(json!({
                "id": body.id,
                "kind": if surface { "surface" } else { "solid" },
                "name": if surface { surface_name(doc, state, i) } else { body_name(doc, state, i) },
                "hidden": display.is_some_and(|d| d.hidden),
                "faces": indices(of_model::geom::Kind::Face)?,
                "edges": indices(of_model::geom::Kind::Edge)?,
                "vertices": indices(of_model::geom::Kind::Vertex)?,
            }))
        })
        .collect()
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
    let contour_result = sk.contours();
    let profile_error = contour_result.as_ref().err().map(|e| e.to_string());
    let contours: Vec<Value> = contour_result.map(|c| {
        c.loops.into_iter().map(|chain| (chain, true))
            .chain(c.open.into_iter().map(|chain| (chain, false)))
            .map(|(chain, closed)| {
                let ordered_entities = chain.entities;
                let mut entities = ordered_entities.clone();
                entities.sort_unstable();
                json!({ "reference": Ref::SketchContour { sketch: id, entities }, "closed": closed, "ordered_entities": ordered_entities })
            }).collect()
    }).unwrap_or_default();
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
        "contours": contours,
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
            params["scope"] = json!(f.scope);
            // Sketch geometry is fetched separately; keep the tree light.
            if let FeatureKind::Sketch(_) = f.kind {
                params["sketch"] = Value::Null;
            }
            json!({
                "id": f.id,
                "name": f.name,
                "type": params["type"].clone(),
                "suppressed": s.suppressed,
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
        "effective_material": of_model::configurations::resolve(&model.doc).ok().and_then(|doc| doc.material),
        "parameters": model.doc.features.iter().flat_map(of_model::equations::parameters).collect::<Vec<_>>(),
        "variables": model.doc.variables,
        "equations": model.doc.equations,
        "equation_values": equation_values(&model.doc),
        "configurations": model.doc.configurations,
        "properties": model.doc.properties,
        "dimension_display": model.doc.dimension_display,
        "bodies": body_views(&model.doc, model.state()).unwrap_or_default(),
        "sheet_metal": sheet_metal_views(&model.doc, model.state()),
    })
}

/// Each sheet metal body for the tree: its creating feature, body and
/// parameters, without flattening anything.
fn sheet_metal_views(doc: &Document, state: &State) -> Vec<Value> {
    state
        .sheet_metal
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let index = of_model::sheet_metal::record_body(state, i);
            json!({
                "feature": r.feature,
                "body": index.map(|i| state.bodies[i].id.clone()),
                "name": index.map(|i| body_name(doc, state, i)),
                "parameters": r.params,
                "bends": r.bends.len(),
                "flattened": r.flat_plane.is_some(),
            })
        })
        .collect()
}

pub fn equation_values(doc: &Document) -> Value {
    let evaluation = of_model::equations::effective(doc);
    let values: serde_json::Map<String, Value> = evaluation
        .values
        .into_iter()
        .map(|(name, value)| {
            let result = match value {
                Ok(value) => json!({ "value": value, "error": null }),
                Err(error) => json!({ "value": null, "error": error }),
            };
            (name, result)
        })
        .collect();
    Value::Object(values)
}

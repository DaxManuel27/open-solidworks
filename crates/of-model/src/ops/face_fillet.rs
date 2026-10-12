//! Face fillets: a rolling-ball blend between two sets of planar faces.
//!
//! Each set lies in one plane. A ball of the fillet radius touching both planes
//! traces a cylinder whose axis is parallel to the planes' intersection line. The
//! region between that cylinder and the two planes (in cross-section: the corner
//! vertex, both tangent points and the arc between them) is added for a concave
//! corner or removed for a convex one, so the result is tangent to both planes.
//! The faces need not be adjacent: anything between the sets inside that region is
//! replaced by the blend.

use of_geom::{BoolOp, Curve2 as GCurve, DVec2, DVec3, Kind, Plane, Profile, ProfileFace, Shape};

use super::{RResult, apply_feature_tool, body_of, geo, sweep_names};
use crate::build::State;
use crate::naming::{Inputs, Naming};
use crate::{FaceFillet, FeatureId, Ref};

/// A set of coplanar faces: the plane's point and outward normal, plus points
/// sampled along the faces' edges.
struct FaceSet {
    point: DVec3,
    normal: DVec3,
    samples: Vec<DVec3>,
    shapes: Vec<Shape>,
}

fn face_set(prev: &State, refs: &[Ref], which: &str) -> RResult<FaceSet> {
    if refs.is_empty() {
        return Err(format!("select at least one face for {which}"));
    }
    let body = body_of(prev)?;
    let face_edges = body.shape.face_edges();
    let mut set: Option<FaceSet> = None;
    for r in refs {
        let (kind, index) = prev.index_of(r)?;
        if kind != Kind::Face {
            return Err(format!("{which} must contain faces"));
        }
        let info = body.shape.face_info(index).map_err(geo)?;
        if info.kind != of_geom::SurfaceKind::Plane {
            return Err("face fillets support planar faces only".into());
        }
        let mut samples = Vec::new();
        for &e in &face_edges[index as usize] {
            samples.extend(body.shape.edge_samples(e, 0.01, 0.05).map_err(geo)?);
        }
        let shape = body.shape.sub_shape(Kind::Face, index).map_err(geo)?;
        match &mut set {
            None => {
                set = Some(FaceSet {
                    point: info.point,
                    normal: info.normal.normalize(),
                    samples,
                    shapes: vec![shape],
                })
            }
            Some(s) => {
                if s.normal.dot(info.normal) < 1.0 - 1e-9
                    || (info.point - s.point).dot(s.normal).abs() > 1e-6
                {
                    return Err(format!("the faces in {which} must lie in one plane"));
                }
                s.samples.extend(samples);
                s.shapes.push(shape);
            }
        }
    }
    Ok(set.expect("at least one face"))
}

pub(super) fn face_fillet(
    fid: FeatureId,
    p: &FaceFillet,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let r = p.radius;
    if !(r.is_finite() && r > 1e-9) {
        return Err("the fillet radius must be more than zero".into());
    }
    let a = face_set(prev, &p.faces1, "face set 1")?;
    let b = face_set(prev, &p.faces2, "face set 2")?;
    let (n1, n2) = (a.normal, b.normal);
    let d = n1.cross(n2);
    if d.length() < 1e-6 {
        return Err("the two face sets are parallel; a face fillet needs planes that meet".into());
    }
    let d = d.normalize();
    // A point on both planes: solve within the span of the two normals.
    let c = n1.dot(n2);
    let (h1, h2) = (n1.dot(a.point), n2.dot(b.point));
    let det = 1.0 - c * c;
    let vertex = n1 * ((h1 - c * h2) / det) + n2 * ((h2 - c * h1) / det);

    // Concave when each set lies in front of the other's plane, convex when behind.
    let side = |set: &FaceSet, point: DVec3, normal: DVec3| -> f64 {
        set.samples
            .iter()
            .map(|q| (*q - point).dot(normal))
            .fold(0.0, |acc: f64, x| if x.abs() > acc.abs() { x } else { acc })
    };
    let (s1, s2) = (side(&b, a.point, n1), side(&a, b.point, n2));
    let scale = (a.samples.iter().chain(&b.samples))
        .map(|q| (*q - vertex).length())
        .fold(1.0, f64::max);
    let tol = 1e-7 * scale;
    let concave = match (s1 > tol, s2 > tol, s1 < -tol, s2 < -tol) {
        (true, true, _, _) => true,
        (_, _, true, true) => false,
        _ => {
            return Err(
                "cannot tell which side to fillet; select faces that form one corner".into(),
            );
        }
    };
    let s = if concave { 1.0 } else { -1.0 };
    let center = vertex + (n1 + n2) * (s * r / (1.0 + c));
    let t1 = center - n1 * (s * r);
    let t2 = center - n2 * (s * r);

    // The blend spans the overlap of both sets along the corner line.
    let extent = |set: &FaceSet| {
        set.samples
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), q| {
                let x = (*q - vertex).dot(d);
                (lo.min(x), hi.max(x))
            })
    };
    let ((lo1, hi1), (lo2, hi2)) = (extent(&a), extent(&b));
    let (lo, hi) = (lo1.max(lo2), hi1.min(hi2));
    if hi - lo <= tol {
        return Err("the face sets do not overlap along their corner".into());
    }
    // Both tangent lines must run along the selected faces.
    let mid = (lo + hi) / 2.0;
    for (point, set, which) in [(t1, &a, "face set 1"), (t2, &b, "face set 2")] {
        // A tiny box stands in for the tangent point in the distance query.
        let at = point + d * (mid - (point - vertex).dot(d));
        let probe = Shape::make_box(tol, tol, tol)
            .and_then(|b| b.transform(&of_geom::DAffine3::from_translation(at)))
            .map_err(geo)?
            .shape;
        let near = set
            .shapes
            .iter()
            .filter_map(|f| f.distance(&probe).ok())
            .any(|dist| dist.value < 1e-5 * scale);
        if !near {
            return Err(format!(
                "the fillet radius does not fit on {which}; reduce the radius"
            ));
        }
    }

    // Cross-section in a plane normal to the corner line, at the overlap start.
    let origin = vertex + d * lo;
    let e1 = (t1 - vertex).normalize_or_zero();
    let e1 = if e1.length() > 0.5 {
        e1
    } else {
        n1.cross(d).normalize()
    };
    let plane = Plane::new(origin, e1, d);
    let local = |q: DVec3| plane.to_local(q - d * (q - origin).dot(d));
    let (v2, a2, b2, c2) = (local(vertex), local(t1), local(t2), local(center));
    // The arc runs counter-clockwise; take the direction whose middle faces the vertex.
    let ccw = |from: DVec2, to: DVec2| GCurve::Arc {
        center: c2,
        start: from,
        end: to,
    };
    let bisector = (v2 - c2).normalize();
    let mid_of = |from: DVec2, to: DVec2| {
        let (af, at) = ((from - c2).to_angle(), (to - c2).to_angle());
        let mut sweep = at - af;
        if sweep <= 0.0 {
            sweep += std::f64::consts::TAU;
        }
        DVec2::from_angle(af + sweep / 2.0)
    };
    let arc = if mid_of(a2, b2).dot(bisector) > 0.0 {
        ccw(a2, b2)
    } else {
        ccw(b2, a2)
    };
    let outer = vec![
        GCurve::Line { a: v2, b: a2 },
        arc,
        GCurve::Line { a: b2, b: v2 },
    ];
    let face = Profile {
        plane,
        faces: vec![ProfileFace {
            outer,
            holes: vec![],
        }],
    }
    .to_shape()
    .map_err(geo)?;
    let edge_names: Vec<String> = (0..face.count(Kind::Edge))
        .map(|i| match face.edge_info(i) {
            Ok(info) if info.kind == of_geom::CurveKind::Circle => "blend".to_string(),
            Ok(info) if (info.mid - a.point).dot(n1).abs() < tol => "set1".to_string(),
            Ok(_) => "set2".to_string(),
            Err(_) => "e".to_string(),
        })
        .collect();
    let swept = face.prism(d * (hi - lo)).map_err(geo)?;
    let mut generate = sweep_names(fid, "", &edge_names);
    let names = Naming::from_history(
        &swept.shape,
        &swept.history,
        Inputs {
            named: [None, None],
        },
        &mut generate,
        &fid.to_string(),
    );
    let op = if concave { BoolOp::Fuse } else { BoolOp::Cut };
    apply_feature_tool(fid, state, swept.shape, names, op, true)
}

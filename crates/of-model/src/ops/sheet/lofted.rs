//! Lofted bends: a faceted transition between two open profiles.
//!
//! Both profiles are measured by arc length from their start (the seam) and
//! sampled at matching fractions, including every corner of either profile.
//! Neighbouring samples make triangles, which are flat, so the surface is a
//! strip of facets that unfolds exactly by turning each facet about the edge
//! it shares with the last. The sheet is those facets offset by the
//! thickness.

use std::collections::BTreeMap;

use of_geom::{DVec3, Kind, Plane, Shape};

use super::{RResult, Seg, check_params, geo, oriented_chain, sketch_of};
use crate::FeatureId;
use crate::build::{Body, State};
use crate::naming::Naming;
use crate::sheet_metal::{LoftMethod, LoftedBend, SheetMetalBody};

struct Path {
    plane: Plane,
    segs: Vec<Seg>,
    lengths: Vec<f64>,
    total: f64,
}

impl Path {
    fn new(plane: Plane, segs: Vec<Seg>) -> Path {
        let lengths: Vec<f64> = segs.iter().map(Seg::length).collect();
        let total = lengths.iter().sum();
        Path {
            plane,
            segs,
            lengths,
            total,
        }
    }

    /// Fractions of the length at which segments end.
    fn corners(&self) -> Vec<f64> {
        let mut at = 0.0;
        let mut out = Vec::new();
        for l in &self.lengths[..self.lengths.len() - 1] {
            at += l;
            out.push(at / self.total);
        }
        out
    }

    /// The point at a fraction of the length, and the segment it is on.
    fn at(&self, f: f64) -> (DVec3, usize) {
        let mut s = f.clamp(0.0, 1.0) * self.total;
        for (i, seg) in self.segs.iter().enumerate() {
            if s <= self.lengths[i] + 1e-12 || i == self.segs.len() - 1 {
                return (self.plane.to_world(seg.point_at(s / self.lengths[i])), i);
            }
            s -= self.lengths[i];
        }
        unreachable!()
    }

    /// How many facets an interval of a segment needs to stay within
    /// `deviation` of it.
    fn pieces(&self, from: f64, to: f64, deviation: f64) -> usize {
        let (_, i) = self.at((from + to) / 2.0);
        match self.segs[i] {
            Seg::Line { .. } => 1,
            Seg::Arc { r, .. } => {
                let angle = (to - from) * self.total / r;
                let step = 2.0 * (1.0 - (deviation / r).min(1.0)).acos();
                ((angle / step.max(1e-6)).ceil() as usize).max(1)
            }
        }
    }

    fn curved(&self, from: f64, to: f64) -> bool {
        matches!(self.segs[self.at((from + to) / 2.0).1], Seg::Arc { .. })
    }
}

fn profile(prev: &State, sketch: FeatureId) -> RResult<Path> {
    let (plane, sk) = sketch_of(prev, sketch)?;
    let contours = sk.contours().map_err(geo)?;
    match (contours.loops.as_slice(), contours.open.as_slice()) {
        ([], [chain]) => Ok(Path::new(*plane, oriented_chain(&chain.curves)?)),
        _ => Err("each lofted bend profile must be one open chain with a small gap".into()),
    }
}

pub(in crate::ops) fn lofted_bend(
    fid: FeatureId,
    p: &LoftedBend,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let params = check_params(&p.sheet, prev.material.as_deref())?;
    let t = params.thickness;
    let (a, b) = (profile(prev, p.profile1)?, profile(prev, p.profile2)?);
    // Sample fractions: every corner of both profiles, then facets between.
    let mut cuts: Vec<f64> = [0.0, 1.0]
        .into_iter()
        .chain(a.corners())
        .chain(b.corners())
        .collect();
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
    let mut fractions = vec![0.0];
    for w in cuts.windows(2) {
        let (from, to) = (w[0], w[1]);
        let n = match p.method {
            LoftMethod::Bent => {
                if a.curved(from, to) || b.curved(from, to) {
                    p.facets.max(1) as usize
                } else {
                    1
                }
            }
            LoftMethod::Formed => {
                a.pieces(from, to, p.deviation)
                    .max(b.pieces(from, to, p.deviation))
            }
        };
        for k in 1..=n {
            fractions.push(from + (to - from) * k as f64 / n as f64);
        }
    }
    let n = fractions.len();
    let lower: Vec<DVec3> = fractions.iter().map(|f| a.at(*f).0).collect();
    let upper: Vec<DVec3> = fractions.iter().map(|f| b.at(*f).0).collect();
    // Facets of the inner surface, and each vertex's averaged normal.
    let mut tris: Vec<[usize; 3]> = Vec::new();
    // Vertex i is lower[i], vertex n + i is upper[i].
    for i in 0..n - 1 {
        tris.push([i, i + 1, n + i + 1]);
        tris.push([i, n + i + 1, n + i]);
    }
    let points: Vec<DVec3> = lower.iter().chain(&upper).copied().collect();
    let mut normals = vec![DVec3::ZERO; 2 * n];
    for tri in &tris {
        let [p0, p1, p2] = tri.map(|i| points[i]);
        let normal = (p1 - p0).cross(p2 - p0);
        if normal.length() < 1e-12 {
            return Err("two profile points meet; move the profiles apart".into());
        }
        for i in tri {
            normals[*i] += normal.normalize();
        }
    }
    let side = if p.reverse { -1.0 } else { 1.0 };
    let mut all = points.clone();
    all.extend(
        points
            .iter()
            .zip(&normals)
            .map(|(q, nrm)| *q + nrm.normalize() * t * side),
    );
    let outer = |i: usize| i + 2 * n;
    let mut faces: Vec<Vec<u32>> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut add = |f: Vec<usize>, name: String, faces: &mut Vec<Vec<u32>>| {
        let f: Vec<u32> = f.into_iter().map(|i| i as u32).collect();
        faces.push(if side > 0.0 {
            f.into_iter().rev().collect()
        } else {
            f
        });
        names.push(name);
    };
    for (k, tri) in tris.iter().enumerate() {
        add(tri.to_vec(), format!("{fid}.f{k}.a"), &mut faces);
        let o = [outer(tri[0]), outer(tri[2]), outer(tri[1])];
        add(o.to_vec(), format!("{fid}.f{k}.b"), &mut faces);
    }
    // Thickness walls along both profiles and at the seam.
    for i in 0..n - 1 {
        for (row, tag) in [(0, "p1"), (n, "p2")] {
            let (u, v) = (row + i, row + i + 1);
            let quad = if row == 0 {
                [u, outer(u), outer(v), v]
            } else {
                [v, outer(v), outer(u), u]
            };
            add(
                vec![quad[0], quad[1], quad[2]],
                format!("{fid}.{tag}.{i}"),
                &mut faces,
            );
            add(
                vec![quad[0], quad[2], quad[3]],
                format!("{fid}.{tag}.{i}"),
                &mut faces,
            );
        }
    }
    for (end, tag) in [(0, "seam0"), (n - 1, "seam1")] {
        let (u, v) = (end, n + end);
        let quad = if end == 0 {
            [v, outer(v), outer(u), u]
        } else {
            [u, outer(u), outer(v), v]
        };
        add(
            vec![quad[0], quad[1], quad[2]],
            format!("{fid}.{tag}"),
            &mut faces,
        );
        add(
            vec![quad[0], quad[2], quad[3]],
            format!("{fid}.{tag}"),
            &mut faces,
        );
    }
    let shape =
        Shape::polyhedron(&all, &faces).map_err(|e| format!("the lofted bend failed: {}", e.0))?;
    // Name the solid's faces after the polygons they came from.
    let centroids: Vec<DVec3> = faces
        .iter()
        .map(|f| f.iter().map(|i| all[*i as usize]).sum::<DVec3>() / f.len() as f64)
        .collect();
    let mut by_face: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for i in 0..shape.count(Kind::Face) {
        let c = shape.face_info(i).map_err(geo)?.centroid;
        let k = (0..centroids.len())
            .min_by(|x, y| {
                (centroids[*x] - c)
                    .length()
                    .total_cmp(&(centroids[*y] - c).length())
            })
            .unwrap();
        by_face.insert(i, vec![names[k].clone()]);
    }
    let body = Body {
        names: Naming::from_faces(&shape, by_face.into_values().collect()),
        shape,
    };
    let facets: Vec<String> = (0..tris.len()).map(|k| format!("{fid}.f{k}.a")).collect();
    let fixed = facets[(tris.len() / 2) & !1].clone();
    super::apply_tool(
        fid,
        state,
        body.shape,
        body.names,
        of_geom::BoolOp::Fuse,
        false,
    )?;
    state.sheet_metal.push(SheetMetalBody {
        feature: fid,
        fixed_face: fixed,
        params,
        bends: Vec::new(),
        bend_lines: Vec::new(),
        cross_breaks: Vec::new(),
        facets,
    });
    Ok(())
}

//! Sheet metal features: base flanges, tabs and edge flanges, and the flat
//! pattern. See [`crate::sheet_metal`] for the bend records they write.

mod bend;
mod convert;
mod corners;
mod fold;
mod forms;
mod hem;
mod lofted;
mod miter;
mod swept;
mod unfold;

use std::f64::consts::{PI, TAU};

use of_geom::{BoolOp, Curve2 as GCurve, DVec2, DVec3, Kind, Plane, Profile, ProfileFace, Shape};
use of_sketch::Curve2 as SCurve;

use super::{
    ProfileShape, RResult, apply_tool, body_of, extrude_side, geo, sketch_of, sketch_profile,
    sweep_names, translate,
};
use crate::build::{Body, State};
use crate::naming::{Inputs, Naming};
use crate::resolve::{Resolved, resolve};
use crate::sheet_metal::{
    BaseFlange, BendRecord, CrossBreak, CrossBreakRecord, EdgeFlange, FlangeLength, FlangePosition,
    FlatPattern, LengthMeasure, ReliefKind, SheetMetalBody, SheetMetalParams, ThicknessSide,
};
use crate::{EndCondition, FeatureId, Ref};

pub(crate) use bend::{jog, sketched_bend};
pub(crate) use convert::{convert, insert_bends, rip};
pub(crate) use corners::{bend_notch, break_corner, closed_corner, corner_relief, welded_corner};
pub(crate) use fold::{Unfolded, cut_flat_regions, fold, normal_cut, unfold};
pub(crate) use forms::{form_tool, gusset, tab_and_slot};
pub(super) use hem::hem;
pub(super) use lofted::lofted_bend;
pub(super) use miter::miter_flange;
pub(super) use swept::swept_flange;
pub(crate) use unfold::flatten;

/// Strip a split suffix (`~2`) from a face name.
pub(crate) fn base_name(name: &str) -> &str {
    match name.rsplit_once('~') {
        Some((base, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => base,
        _ => name,
    }
}

/// Whether any of a face's names is `name`, ignoring split suffixes.
fn has_name(names: &[String], name: &str) -> bool {
    let name = base_name(name);
    names.iter().any(|n| base_name(n) == name)
}

/// The index in `state.bodies` of the body a sheet metal record describes.
pub(crate) fn record_body(state: &State, record: &SheetMetalBody) -> Option<usize> {
    state.bodies.iter().position(|b| {
        b.geometry
            .names
            .faces
            .iter()
            .any(|f| has_name(f, &record.fixed_face))
    })
}

/// The sheet metal record of the body that owns face `face` of the aggregate.
fn record_for_face(state: &State, face: u32) -> RResult<usize> {
    let whole = &body_of(state)?.shape;
    let body = state
        .bodies
        .iter()
        .position(|b| {
            b.geometry
                .shape
                .indices_in(whole, Kind::Face)
                .is_ok_and(|ix| ix.contains(&face))
        })
        .ok_or("the selection is not on a body")?;
    state
        .sheet_metal
        .iter()
        .position(|r| record_body(state, r) == Some(body))
        .ok_or_else(|| "select geometry of a sheet metal body".to_string())
}

fn check_params(p: &SheetMetalParams, material: Option<&str>) -> RResult<SheetMetalParams> {
    let mut p = crate::sheet_tables::resolve(p)?;
    if p.use_material {
        let name = material.ok_or("set the part's material to use its sheet metal values")?;
        let (k, ratio) = crate::materials::sheet_metal_defaults(name)
            .ok_or_else(|| format!("{name} has no sheet metal values"))?;
        p.allowance = crate::sheet_metal::Allowance::KFactor { k };
        p.radius = ratio * p.thickness;
    }
    if !(p.thickness.is_finite() && p.thickness > 1e-6) {
        return Err("the sheet thickness must be more than zero".into());
    }
    if !(p.radius.is_finite() && p.radius > 1e-6) {
        return Err("the bend radius must be more than zero".into());
    }
    Ok(p)
}

/// The first sheet metal feature, or a tab on an existing sheet metal face.
pub(super) fn base_flange(
    fid: FeatureId,
    p: &BaseFlange,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let params = check_params(&p.sheet, prev.material.as_deref())?;
    let (plane, sk) = sketch_of(prev, p.sketch)?;
    let contours = sk.contours().map_err(geo)?;
    if !contours.regions.is_empty() {
        if !contours.open.is_empty() {
            return Err(
                "a base flange sketch must be closed contours or one open chain, not both".into(),
            );
        }
        return closed_base(fid, p, params, prev, state);
    }
    match contours.open.as_slice() {
        [chain] => open_base(
            fid,
            p,
            params,
            *plane,
            &chain.entities,
            &chain.curves,
            prev,
            state,
        ),
        [] => Err("the sketch has no contours".into()),
        _ => Err("an open base flange sketch must be a single chain of lines and arcs".into()),
    }
}

/// A sheet metal body with a planar face on `plane` facing either way.
fn tab_target(prev: &State, plane: &Plane) -> Option<(usize, DVec3)> {
    for (i, record) in prev.sheet_metal.iter().enumerate() {
        let Some(b) = record_body(prev, record) else {
            continue;
        };
        let shape = &prev.bodies[b].geometry.shape;
        for f in 0..shape.count(Kind::Face) {
            let Ok(info) = shape.face_info(f) else {
                continue;
            };
            if info.kind == of_geom::SurfaceKind::Plane
                && info.normal.cross(plane.normal).length() < 1e-9
                && plane.distance(info.point).abs() < 1e-6
            {
                return Some((i, info.normal));
            }
        }
    }
    None
}

fn closed_base(
    fid: FeatureId,
    p: &BaseFlange,
    params: SheetMetalParams,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let prof = sketch_profile(prev, p.sketch, None)?;
    if let Some((record, outward)) = tab_target(prev, &prof.plane) {
        // A tab: grow into the sheet's material from its face, at its thickness.
        let t = prev.sheet_metal[record].params.thickness;
        let (tool, names) = extrude_side(
            fid,
            &prof,
            -outward,
            &EndCondition::Blind { depth: t },
            "",
            prev,
        )?;
        return apply_tool(fid, state, tool, names, BoolOp::Fuse, true);
    }
    let t = params.thickness;
    let n = prof.plane.normal;
    let (dir, start) = match p.side {
        ThicknessSide::Normal => (n, 0.0),
        ThicknessSide::Reverse => (-n, 0.0),
        ThicknessSide::MidPlane => (n, -t / 2.0),
    };
    let prof = if start != 0.0 {
        ProfileShape {
            shape: translate(&prof.shape, n * start)?,
            plane: prof.plane.offset(start),
            edge_names: prof.edge_names,
        }
    } else {
        prof
    };
    let (tool, names) = extrude_side(fid, &prof, dir, &EndCondition::Blind { depth: t }, "", prev)?;
    apply_tool(fid, state, tool, names, BoolOp::Fuse, false)?;
    state.sheet_metal.push(SheetMetalBody {
        feature: fid,
        fixed_face: format!("{fid}.start"),
        params,
        bends: Vec::new(),
        bend_lines: Vec::new(),
        cross_breaks: Vec::new(),
        facets: Vec::new(),
        forms: Vec::new(),
        welds: Vec::new(),
        form_marks: Vec::new(),
    });
    Ok(())
}

/// A curve of an open profile, oriented along the chain.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Seg {
    Line {
        a: DVec2,
        b: DVec2,
    },
    /// Center, radius, start angle and signed sweep (positive counter-clockwise).
    Arc {
        c: DVec2,
        r: f64,
        a0: f64,
        sweep: f64,
    },
}

fn left(t: DVec2) -> DVec2 {
    DVec2::new(-t.y, t.x)
}

impl Seg {
    fn at(c: DVec2, r: f64, a: f64) -> DVec2 {
        c + DVec2::from_angle(a) * r
    }
    pub(crate) fn start(&self) -> DVec2 {
        match *self {
            Seg::Line { a, .. } => a,
            Seg::Arc { c, r, a0, .. } => Seg::at(c, r, a0),
        }
    }
    pub(crate) fn end(&self) -> DVec2 {
        match *self {
            Seg::Line { b, .. } => b,
            Seg::Arc { c, r, a0, sweep } => Seg::at(c, r, a0 + sweep),
        }
    }
    fn tangent_at(&self, end: bool) -> DVec2 {
        match *self {
            Seg::Line { a, b } => (b - a).normalize(),
            Seg::Arc { a0, sweep, .. } => {
                let a = if end { a0 + sweep } else { a0 };
                let t = DVec2::new(-a.sin(), a.cos());
                if sweep > 0.0 { t } else { -t }
            }
        }
    }
    pub(crate) fn length(&self) -> f64 {
        match *self {
            Seg::Line { a, b } => (b - a).length(),
            Seg::Arc { r, sweep, .. } => r * sweep.abs(),
        }
    }
    /// The point at fraction `f` of the length.
    pub(crate) fn point_at(&self, f: f64) -> DVec2 {
        match *self {
            Seg::Line { a, b } => a.lerp(b, f),
            Seg::Arc { c, r, a0, sweep } => Seg::at(c, r, a0 + sweep * f),
        }
    }
    fn reversed(&self) -> Seg {
        match *self {
            Seg::Line { a, b } => Seg::Line { a: b, b: a },
            Seg::Arc { c, r, a0, sweep } => Seg::Arc {
                c,
                r,
                a0: a0 + sweep,
                sweep: -sweep,
            },
        }
    }
    /// The curve moved `d` to its left.
    fn offset(&self, d: f64) -> Seg {
        match *self {
            Seg::Line { a, b } => {
                let n = left((b - a).normalize()) * d;
                Seg::Line { a: a + n, b: b + n }
            }
            Seg::Arc { c, r, a0, sweep } => Seg::Arc {
                c,
                r: r - sweep.signum() * d,
                a0,
                sweep,
            },
        }
    }
    fn to_geom(self) -> GCurve {
        match self {
            Seg::Line { a, b } => GCurve::Line { a, b },
            Seg::Arc { c, .. } => {
                let (s, e) = if self.sweep() > 0.0 {
                    (self.start(), self.end())
                } else {
                    (self.end(), self.start())
                };
                GCurve::Arc {
                    center: c,
                    start: s,
                    end: e,
                }
            }
        }
    }
    fn sweep(&self) -> f64 {
        match *self {
            Seg::Line { .. } => 0.0,
            Seg::Arc { sweep, .. } => sweep,
        }
    }
    fn distance(&self, p: DVec2) -> f64 {
        match *self {
            Seg::Line { a, b } => {
                let d = b - a;
                let t = ((p - a).dot(d) / d.length_squared()).clamp(0.0, 1.0);
                (a + d * t - p).length()
            }
            Seg::Arc { c, r, a0, sweep } => {
                let ang = (p - c).to_angle();
                let mut rel = (ang - a0) * sweep.signum();
                rel = rel.rem_euclid(TAU);
                if rel <= sweep.abs() + 1e-12 {
                    ((p - c).length() - r).abs()
                } else {
                    (self.start() - p).length().min((self.end() - p).length())
                }
            }
        }
    }
}

fn from_sketch(c: &SCurve) -> RResult<Seg> {
    match c {
        SCurve::Line { a, b } => Ok(Seg::Line { a: *a, b: *b }),
        SCurve::Arc { center, start, end } => {
            let a0 = (*start - *center).to_angle();
            let mut sweep = (*end - *center).to_angle() - a0;
            if sweep <= 1e-12 {
                sweep += TAU;
            }
            Ok(Seg::Arc {
                c: *center,
                r: (*start - *center).length(),
                a0,
                sweep,
            })
        }
        _ => Err("an open base flange profile may only contain lines and arcs".into()),
    }
}

/// The chain's curves, each oriented from the chain's start to its end.
pub(crate) fn oriented_chain(curves: &[SCurve]) -> RResult<Vec<Seg>> {
    let mut segs: Vec<Seg> = curves.iter().map(from_sketch).collect::<RResult<_>>()?;
    let near = |a: DVec2, b: DVec2| (a - b).length() < 1e-6;
    if segs.len() > 1 {
        let (s, n) = (segs[0], segs[1]);
        if near(s.start(), n.start()) || near(s.start(), n.end()) {
            segs[0] = s.reversed();
        }
    }
    for i in 1..segs.len() {
        let end = segs[i - 1].end();
        if near(segs[i].end(), end) {
            segs[i] = segs[i].reversed();
        } else if !near(segs[i].start(), end) {
            return Err("the profile is not one connected chain".into());
        }
    }
    Ok(segs)
}

/// One element of a thickened open profile.
pub(crate) struct Element {
    pub seg: Seg,
    /// `s{entity}` for a sketch curve, `c{n}` for a corner bend.
    pub name: String,
    /// Inside radius and angle when the element is a bend.
    pub bend: Option<(f64, f64)>,
}

/// The chain with sharp corners rounded so that the inside radius is
/// `radius`, for material between left offsets `lo` and `hi` (lo ≤ 0 ≤ hi).
pub(crate) fn round_corners(
    segs: &[Seg],
    names: &[String],
    radius: f64,
    lo: f64,
    hi: f64,
) -> RResult<Vec<Element>> {
    let n = segs.len();
    let mut trim = vec![(0.0, 0.0); n];
    let mut corners: Vec<Option<Element>> = Vec::with_capacity(n);
    let mut k = 0;
    for i in 0..n.saturating_sub(1) {
        let (t_in, t_out) = (segs[i].tangent_at(true), segs[i + 1].tangent_at(false));
        let cross = t_in.perp_dot(t_out);
        let dot = t_in.dot(t_out);
        if cross.abs() < 1e-9 && dot > 0.0 {
            corners.push(None);
            continue;
        }
        if dot < -1.0 + 1e-9 {
            return Err("the profile folds back on itself; use a hem".into());
        }
        if !matches!(segs[i], Seg::Line { .. }) || !matches!(segs[i + 1], Seg::Line { .. }) {
            return Err("a sharp corner must be between two lines; make arcs tangent".into());
        }
        let theta = cross.abs().atan2(dot);
        let turns_left = cross > 0.0;
        let rho = if turns_left { radius + hi } else { radius - lo };
        let d = rho * (theta / 2.0).tan();
        trim[i].1 += d;
        trim[i + 1].0 += d;
        let p = segs[i].end();
        let start = p - t_in * d;
        let side = if turns_left { 1.0 } else { -1.0 };
        let c = start + left(t_in) * rho * side;
        corners.push(Some(Element {
            seg: Seg::Arc {
                c,
                r: rho,
                a0: (start - c).to_angle(),
                sweep: theta * side,
            },
            name: format!("c{k}"),
            bend: Some((radius, theta)),
        }));
        k += 1;
    }
    let mut out = Vec::new();
    for i in 0..n {
        let seg = match segs[i] {
            Seg::Line { a, b } => {
                let len = (b - a).length();
                let (s, e) = trim[i];
                if s + e > len - 1e-6 {
                    return Err("a line is too short for the bends at its ends".into());
                }
                let t = (b - a) / len;
                Seg::Line {
                    a: a + t * s,
                    b: b - t * e,
                }
            }
            arc => arc,
        };
        let bend = match seg {
            Seg::Arc { r, sweep, .. } => {
                let inside = if sweep > 0.0 { r - hi } else { r + lo };
                if inside < 1e-6 {
                    return Err("an arc's radius is smaller than the sheet thickness".into());
                }
                Some((inside, sweep.abs()))
            }
            Seg::Line { .. } => None,
        };
        out.push(Element {
            seg,
            name: names[i].clone(),
            bend,
        });
        if let Some(Some(c)) = corners.get_mut(i).map(Option::take) {
            out.push(c);
        }
    }
    Ok(out)
}

/// The closed region between the left offsets `lo` and `hi` of a profile,
/// with each curve named `{element}.a` (left side), `{element}.b` (right
/// side), or `end0`/`end1` for the ends.
pub(crate) fn thickened(elements: &[Element], lo: f64, hi: f64) -> Vec<(Seg, String)> {
    let mut out = Vec::new();
    for e in elements {
        out.push((e.seg.offset(hi), format!("{}.a", e.name)));
    }
    let last = elements.last().unwrap().seg;
    out.push((
        Seg::Line {
            a: last.offset(hi).end(),
            b: last.offset(lo).end(),
        },
        "end1".into(),
    ));
    for e in elements.iter().rev() {
        out.push((e.seg.offset(lo).reversed(), format!("{}.b", e.name)));
    }
    let first = elements[0].seg;
    out.push((
        Seg::Line {
            a: first.offset(lo).start(),
            b: first.offset(hi).start(),
        },
        "end0".into(),
    ));
    out
}

/// A planar region from named curves, with each kernel edge named after the
/// nearest curve.
pub(crate) fn named_region(plane: Plane, curves: &[(Seg, String)]) -> RResult<ProfileShape> {
    let shape = Profile {
        plane,
        faces: vec![ProfileFace {
            outer: curves.iter().map(|(s, _)| s.to_geom()).collect(),
            holes: Vec::new(),
        }],
    }
    .to_shape()
    .map_err(geo)?;
    let edge_names = (0..shape.count(Kind::Edge))
        .map(|i| {
            let mid = plane.to_local(shape.edge_info(i).map(|e| e.mid).unwrap_or(plane.origin));
            curves
                .iter()
                .min_by(|a, b| a.0.distance(mid).total_cmp(&b.0.distance(mid)))
                .map(|c| c.1.clone())
                .unwrap_or_else(|| "e".into())
        })
        .collect();
    Ok(ProfileShape {
        shape,
        plane,
        edge_names,
    })
}

#[allow(clippy::too_many_arguments)]
fn open_base(
    fid: FeatureId,
    p: &BaseFlange,
    params: SheetMetalParams,
    plane: Plane,
    entities: &[of_sketch::Id],
    curves: &[SCurve],
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let t = params.thickness;
    let (lo, hi) = match p.side {
        ThicknessSide::Normal => (0.0, t),
        ThicknessSide::Reverse => (-t, 0.0),
        ThicknessSide::MidPlane => (-t / 2.0, t / 2.0),
    };
    let segs = oriented_chain(curves)?;
    let names: Vec<String> = entities.iter().map(|e| format!("s{e}")).collect();
    let elements = round_corners(&segs, &names, params.radius, lo, hi)?;
    let first_line = elements
        .iter()
        .find(|e| e.bend.is_none())
        .ok_or("the profile needs at least one straight line")?;
    let fixed = format!(
        "{fid}.{}.{}",
        first_line.name,
        if p.side == ThicknessSide::Reverse {
            "a"
        } else {
            "b"
        }
    );
    let prof = named_region(plane, &thickened(&elements, lo, hi))?;
    let end1 = p
        .end1
        .as_ref()
        .ok_or("an open base flange needs a depth (end1)")?;
    let dir = if p.reverse {
        -plane.normal
    } else {
        plane.normal
    };
    let (mut tool, mut names) = extrude_side(fid, &prof, dir, end1, "", prev)?;
    if let Some(end2) = &p.end2 {
        if matches!(end1, EndCondition::MidPlane { .. }) {
            return Err("mid plane can't be combined with a second direction".into());
        }
        let (t2, n2) = extrude_side(fid, &prof, -dir, end2, "d2.", prev)?;
        let r = tool.boolean(BoolOp::Fuse, &t2).map_err(geo)?;
        names = Naming::from_history(
            &r.shape,
            &r.history,
            Inputs {
                named: [Some(&names), Some(&n2)],
            },
            &mut |_| None,
            &fid.to_string(),
        );
        tool = r.shape;
    }
    apply_tool(fid, state, tool, names, BoolOp::Fuse, false)?;
    let bends = elements
        .iter()
        .filter_map(|e| e.bend.map(|b| (e, b)))
        .enumerate()
        .map(|(i, (e, (radius, angle)))| BendRecord {
            feature: fid,
            index: i as u32,
            faces: [format!("{fid}.{}.a", e.name), format!("{fid}.{}.b", e.name)],
            radius,
            angle,
            allowance: params.allowance.clone(),
        })
        .collect();
    state.sheet_metal.push(SheetMetalBody {
        feature: fid,
        fixed_face: fixed,
        params,
        bends,
        bend_lines: Vec::new(),
        cross_breaks: Vec::new(),
        facets: Vec::new(),
        forms: Vec::new(),
        welds: Vec::new(),
        form_marks: Vec::new(),
    });
    Ok(())
}

/// The frame of one flange: 2D profile coordinates (X outward from the edge
/// in the sheet's plane, Y in the rise direction) and Z along the edge.
pub(super) struct FlangeFrame {
    origin: DVec3,
    x: DVec3,
    y: DVec3,
    z: DVec3,
    length: f64,
    thickness: f64,
    record: usize,
    /// The two edge ends.
    ends: [DVec3; 2],
}

impl FlangeFrame {
    fn world(&self, x: f64, y: f64, z: f64) -> DVec3 {
        self.origin + self.x * x + self.y * y + self.z * z
    }
    fn local(&self, p: DVec3) -> (f64, f64, f64) {
        let d = p - self.origin;
        (d.dot(self.x), d.dot(self.y), d.dot(self.z))
    }
    /// The profile plane at distance `z` along the edge.
    fn plane(&self, z: f64) -> Plane {
        Plane::new(self.world(0.0, 0.0, z), self.x, self.x.cross(self.y))
    }
}

/// The largest distance of a face's vertices from a line.
fn face_width(shape: &Shape, face: u32, p0: DVec3, e: DVec3) -> f64 {
    let vf = shape.vertex_faces();
    let mut width: f64 = 0.0;
    for (v, faces) in vf.iter().enumerate() {
        if !faces.contains(&face) {
            continue;
        }
        if let Ok(p) = shape.vertex_point(v as u32) {
            let d = p - p0;
            width = width.max((d - e * d.dot(e)).length());
        }
    }
    width
}

fn flange_frame(prev: &State, r: &Ref, reverse: bool) -> RResult<FlangeFrame> {
    let body = body_of(prev)?;
    let Resolved::Edge { index, info } = resolve(prev, r)? else {
        return Err("select edges for the flange".into());
    };
    if info.kind != of_geom::CurveKind::Line {
        return Err("flanges need straight edges".into());
    }
    let faces = body.shape.edge_faces()[index as usize].clone();
    if faces.len() != 2 {
        return Err("the edge must lie between two faces".into());
    }
    let record = record_for_face(prev, faces[0])?;
    let t = prev.sheet_metal[record].params.thickness;
    let (p0, p1) = (info.start, info.end);
    let length = (p1 - p0).length();
    let e = (p1 - p0) / length;
    // The thickness face is the one about one thickness wide.
    let widths: Vec<f64> = faces
        .iter()
        .map(|f| face_width(&body.shape, *f, p0, e))
        .collect();
    let (thick, side) = if (widths[0] - t).abs() <= (widths[1] - t).abs() {
        (faces[0], faces[1])
    } else {
        (faces[1], faces[0])
    };
    let (ti, si) = (
        body.shape.face_info(thick).map_err(geo)?,
        body.shape.face_info(side).map_err(geo)?,
    );
    if ti.kind != of_geom::SurfaceKind::Plane
        || si.kind != of_geom::SurfaceKind::Plane
        || (widths[0].min(widths[1]) - t).abs() > 1e-3 * t.max(1.0)
    {
        return Err("select an edge where a flat face of the sheet meets its thickness".into());
    }
    let w = ti.normal;
    let n = si.normal;
    if w.dot(n).abs() > 1e-6 {
        return Err("the sheet's edge face is not square to it".into());
    }
    let (y, origin) = if reverse { (-n, p0 - n * t) } else { (n, p0) };
    Ok(FlangeFrame {
        origin,
        x: w,
        y,
        z: e,
        length,
        thickness: t,
        record,
        ends: [p0, p1],
    })
}

fn rect(x0: f64, x1: f64, y0: f64, y1: f64) -> Vec<(Seg, String)> {
    let c = [
        DVec2::new(x0, y0),
        DVec2::new(x1, y0),
        DVec2::new(x1, y1),
        DVec2::new(x0, y1),
    ];
    (0..4)
        .map(|i| {
            (
                Seg::Line {
                    a: c[i],
                    b: c[(i + 1) % 4],
                },
                format!("r{i}"),
            )
        })
        .collect()
}

/// Extrude a named 2D region in a flange frame from `z0` to `z1`.
fn flange_prism(
    fid: FeatureId,
    prefix: &str,
    f: &FlangeFrame,
    curves: &[(Seg, String)],
    z0: f64,
    z1: f64,
) -> RResult<(Shape, Naming)> {
    let prof = named_region(f.plane(z0), curves)?;
    let r = prof.shape.prism(f.z * (z1 - z0)).map_err(geo)?;
    let mut generate = sweep_names(fid, prefix, &prof.edge_names);
    let names = Naming::from_history(
        &r.shape,
        &r.history,
        Inputs {
            named: [None, None],
        },
        &mut generate,
        &fid.to_string(),
    );
    Ok((r.shape, names))
}

fn fuse_named(fid: FeatureId, a: (Shape, Naming), b: (Shape, Naming)) -> RResult<(Shape, Naming)> {
    let r = a.0.boolean(BoolOp::Fuse, &b.0).map_err(geo)?;
    let names = Naming::from_history(
        &r.shape,
        &r.history,
        Inputs {
            named: [Some(&a.1), Some(&b.1)],
        },
        &mut |_| None,
        &fid.to_string(),
    );
    Ok((r.shape, names))
}

fn common_named(fid: FeatureId, a: (Shape, Naming), tool: &Shape) -> RResult<(Shape, Naming)> {
    let r = a.0.boolean(BoolOp::Common, tool).map_err(geo)?;
    if r.shape.count(Kind::Solid) == 0 {
        return Err("flanges meeting at a corner leave nothing of one of them".into());
    }
    let names = Naming::from_history(
        &r.shape,
        &r.history,
        Inputs {
            named: [Some(&a.1), None],
        },
        &mut |_| None,
        &fid.to_string(),
    );
    Ok((r.shape, names))
}

pub(super) fn edge_flange(
    fid: FeatureId,
    p: &EdgeFlange,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let theta = p.angle;
    if !(theta > 1e-6 && theta < PI - 1e-6) {
        return Err("the flange angle must be between 0° and 180°; use a hem to fold back".into());
    }
    let frames: Vec<FlangeFrame> = p
        .edges
        .iter()
        .map(|r| flange_frame(prev, r, p.reverse))
        .collect::<RResult<_>>()?;
    let mut cuts = Vec::new();
    let mut flanges = Vec::new();
    let mut new_bends = Vec::new();
    for (k, f) in frames.iter().enumerate() {
        let params = &prev.sheet_metal[f.record].params;
        let t = f.thickness;
        let r = p.bend.radius.unwrap_or(params.radius);
        if !(r.is_finite() && r > 1e-6) {
            return Err("the bend radius must be more than zero".into());
        }
        let allowance = p.bend.allowance.clone().unwrap_or(params.allowance.clone());
        let relief = p.bend.relief.unwrap_or(params.relief);
        let (s, c) = (theta.sin(), theta.cos());
        let half = (theta / 2.0).tan();
        let x_end = match p.position {
            FlangePosition::MaterialInside => (r * c - r - t) / s,
            FlangePosition::MaterialOutside => -(r * (1.0 - c) - t * c) / s,
            FlangePosition::BendOutside => 0.0,
            FlangePosition::BendFromVirtualSharp => -(r + t) * half,
            FlangePosition::TangentToBend => {
                if theta >= PI / 2.0 {
                    -(r + t)
                } else {
                    -(r + t) * s
                }
            }
        } - p.position_offset;
        let center = DVec2::new(x_end, r);
        let radial = |rad: f64| center + DVec2::new(s, -c) * rad;
        let d = DVec2::new(c, s);
        let (b_in, b_out) = (radial(r), radial(r + t));
        let straight = match &p.length {
            FlangeLength::Blind { length } => {
                length
                    - match p.measure {
                        LengthMeasure::OuterVirtualSharp => (r + t) * half,
                        LengthMeasure::InnerVirtualSharp => r * half,
                        LengthMeasure::TangentToBend => 0.0,
                    }
            }
            FlangeLength::UpToVertex { target } | FlangeLength::UpToEdgeAndMerge { target } => {
                let q = match resolve(prev, target)? {
                    Resolved::Edge { info, .. } => info.mid,
                    other => other.point()?,
                };
                let (x, y, _) = f.local(q);
                (DVec2::new(x, y) - b_in).dot(d)
            }
            FlangeLength::UpToSurface { target }
            | FlangeLength::OffsetFromSurface { target, .. } => {
                let plane = resolve(prev, target)?.plane()?;
                let start = f.world(b_in.x, b_in.y, 0.0);
                let dir = f.x * d.x + f.y * d.y;
                let along = dir.dot(plane.normal);
                if along.abs() < 1e-9 {
                    return Err("the flange runs parallel to the surface".into());
                }
                let reach = (plane.origin - start).dot(plane.normal) / along;
                match &p.length {
                    FlangeLength::OffsetFromSurface { offset, .. } => reach - offset,
                    _ => reach,
                }
            }
        };
        if straight <= 1e-6 {
            return Err("the flange is too short for its bend".into());
        }
        let (z0, z1) = (p.start_offset, f.length - p.end_offset);
        if z1 - z0 <= 1e-6 {
            return Err("the offsets leave nothing of the flange".into());
        }
        let x0 = x_end.min(0.0);
        let a = DVec2::new(x_end, 0.0);
        let d0 = DVec2::new(x_end, -t);
        let mut profile = Vec::new();
        if x0 < x_end - 1e-9 {
            profile.push((
                Seg::Line {
                    a: DVec2::new(x0, 0.0),
                    b: a,
                },
                "ext.i".into(),
            ));
        }
        profile.push((
            Seg::Arc {
                c: center,
                r,
                a0: -PI / 2.0,
                sweep: theta,
            },
            "bi".into(),
        ));
        profile.push((
            Seg::Line {
                a: b_in,
                b: b_in + d * straight,
            },
            "fi".into(),
        ));
        profile.push((
            Seg::Line {
                a: b_in + d * straight,
                b: b_out + d * straight,
            },
            "tip".into(),
        ));
        profile.push((
            Seg::Line {
                a: b_out + d * straight,
                b: b_out,
            },
            "fo".into(),
        ));
        profile.push((
            Seg::Arc {
                c: center,
                r: r + t,
                a0: -PI / 2.0 + theta,
                sweep: -theta,
            },
            "bo".into(),
        ));
        if x0 < x_end - 1e-9 {
            profile.push((
                Seg::Line {
                    a: d0,
                    b: DVec2::new(x0, -t),
                },
                "ext.o".into(),
            ));
        }
        profile.push((
            Seg::Line {
                a: DVec2::new(x0, -t),
                b: DVec2::new(x0, 0.0),
            },
            "root".into(),
        ));
        let prefix = format!("e{k}.");
        let tool = flange_prism(fid, &prefix, f, &profile, z0, z1)?;
        let tool = corner_trims(fid, tool, k, &frames, p.gap)?;
        flanges.push(tool);
        cuts.extend(bend_cuts(fid, &prefix, f, x_end, z0, z1, relief)?);
        new_bends.push((
            f.record,
            BendRecord {
                feature: fid,
                index: k as u32,
                faces: [format!("{fid}.{prefix}bi"), format!("{fid}.{prefix}bo")],
                radius: r,
                angle: theta,
                allowance,
            },
        ));
    }
    apply_flanges(fid, state, cuts, flanges, new_bends)
}

/// Cut away the material bends replace, add the flanges and record the bends.
pub(super) fn apply_flanges(
    fid: FeatureId,
    state: &mut State,
    cuts: Vec<(Shape, Naming)>,
    flanges: Vec<(Shape, Naming)>,
    bends: Vec<(usize, BendRecord)>,
) -> RResult<()> {
    let mut cut: Option<(Shape, Naming)> = None;
    for piece in cuts {
        cut = Some(match cut {
            None => piece,
            Some(c) => fuse_named(fid, c, piece)?,
        });
    }
    if let Some((tool, names)) = cut {
        apply_tool(fid, state, tool, names, BoolOp::Cut, true)?;
    }
    for (tool, names) in flanges {
        apply_tool(fid, state, tool, names, BoolOp::Fuse, true)?;
    }
    for (record, bend) in bends {
        state.sheet_metal[record].bends.push(bend);
    }
    merge_records(state);
    Ok(())
}

/// Records whose bodies were joined into one become one record, keeping the
/// first one's parameters and fixed face.
pub(super) fn merge_records(state: &mut State) {
    let mut i = 0;
    while i < state.sheet_metal.len() {
        let body = record_body(state, &state.sheet_metal[i]);
        let mut j = i + 1;
        while j < state.sheet_metal.len() {
            if body.is_some() && record_body(state, &state.sheet_metal[j]) == body {
                let other = state.sheet_metal.remove(j);
                let first = &mut state.sheet_metal[i];
                first.bends.extend(other.bends);
                first.cross_breaks.extend(other.cross_breaks);
                first.facets.extend(other.facets);
            } else {
                j += 1;
            }
        }
        i += 1;
    }
}

/// Split the corners a flange shares with the other flanges of its feature,
/// leaving `gap` between them.
pub(super) fn corner_trims(
    fid: FeatureId,
    mut tool: (Shape, Naming),
    k: usize,
    frames: &[FlangeFrame],
    gap: f64,
) -> RResult<(Shape, Naming)> {
    let f = &frames[k];
    for (j, g) in frames.iter().enumerate() {
        if j == k {
            continue;
        }
        let Some(corner) = f
            .ends
            .iter()
            .find(|e| g.ends.iter().any(|o| (**e - *o).length() < 1e-6))
        else {
            continue;
        };
        let m = (f.x - g.x).normalize_or_zero();
        if m == DVec3::ZERO {
            continue;
        }
        let plane = Plane::new(*corner + m * (gap / 2.0), f.y, m);
        let half = Shape::half_space(&Shape::plane_face(&plane).map_err(geo)?, *corner + m)
            .map_err(geo)?;
        tool = common_named(fid, tool, &half)?;
    }
    Ok(tool)
}

/// The sheet a bend starting at `x_end < 0` replaces, from `z0` to `z1` along
/// its edge, and reliefs where the bend stops short of the edge's ends.
pub(super) fn bend_cuts(
    fid: FeatureId,
    prefix: &str,
    f: &FlangeFrame,
    x_end: f64,
    z0: f64,
    z1: f64,
    relief: crate::sheet_metal::Relief,
) -> RResult<Vec<(Shape, Naming)>> {
    if x_end >= -1e-9 {
        return Ok(Vec::new());
    }
    let t = f.thickness;
    let eps = 1e-3 * t.max(1.0);
    let za = if z0 < eps { -eps } else { z0 };
    let zb = if f.length - z1 < eps {
        f.length + eps
    } else {
        z1
    };
    let mut pieces = vec![flange_prism(
        fid,
        &format!("{prefix}cut."),
        f,
        &rect(x_end, eps, -t - eps, eps),
        za,
        zb,
    )?];
    if relief.kind == ReliefKind::None {
        return Ok(pieces);
    }
    let (w, depth) = match relief.kind {
        ReliefKind::Tear => ((t / 100.0).max(0.01), 0.0),
        _ => (relief.width(t), relief.depth(t)),
    };
    let reach = x_end - depth;
    for (at, from, to) in [(z0 > eps, z0 - w, z0), (z1 < f.length - eps, z1, z1 + w)] {
        if !at {
            continue;
        }
        pieces.push(flange_prism(
            fid,
            &format!("{prefix}relief."),
            f,
            &rect(reach, eps, -t - eps, eps),
            from,
            to,
        )?);
        if relief.kind == ReliefKind::Obround {
            // Round the relief's end: a cylinder through the sheet.
            let mid = (from + to) / 2.0;
            let disc = Profile {
                plane: Plane::new(f.world(reach, -t - eps, mid), f.x, f.y),
                faces: vec![ProfileFace {
                    outer: vec![GCurve::Circle {
                        center: DVec2::ZERO,
                        radius: w / 2.0,
                    }],
                    holes: Vec::new(),
                }],
            }
            .to_shape()
            .map_err(geo)?;
            let r = disc.prism(f.y * (t + 2.0 * eps)).map_err(geo)?;
            let names = Naming::by_index(&r.shape, &format!("{fid}.{prefix}relief.o"));
            pieces.push((r.shape, names));
        }
    }
    Ok(pieces)
}

/// Flatten every sheet metal body.
pub(crate) fn flat_pattern(
    _fid: FeatureId,
    p: &FlatPattern,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    if prev.sheet_metal.is_empty() {
        return Err("the part has no sheet metal bodies".into());
    }
    let fixed = match &p.fixed_face {
        Some(r) => match prev.index_of(r)? {
            (Kind::Face, i) => Some(i),
            _ => return Err("select a face to stay fixed".into()),
        },
        None => None,
    };
    let whole = body_of(prev)?.shape.clone();
    for (i, record) in prev.sheet_metal.iter().enumerate() {
        let Some(b) = record_body(prev, record) else {
            continue;
        };
        let body: &Body = &prev.bodies[b].geometry;
        let local = match fixed {
            Some(global) => body
                .shape
                .indices_in(&whole, Kind::Face)
                .map_err(geo)?
                .iter()
                .position(|g| *g == global)
                .map(|l| l as u32),
            None => None,
        };
        let flat = flatten(body, record, local)?;
        let mut geometry = Body {
            shape: flat.shape,
            names: flat.names,
        };
        if let Some(corner) = p.break_corners {
            let t = record.params.thickness;
            let outside: Vec<u32> = corners::outline_corners(&geometry, t)?
                .into_iter()
                .filter(|(_, convex)| *convex)
                .map(|(e, _)| e)
                .collect();
            let edges =
                corners::fitting_corners(&geometry, t, &outside, corners::break_size(corner))?;
            geometry = corners::break_edges(_fid, &geometry, &edges, corner)?;
        }
        state.bodies[b].geometry = geometry;
        state.sheet_metal[i].bend_lines = flat.bend_lines;
        state.sheet_metal[i].form_marks = flat.marks;
    }
    state.aggregate_bodies()
}

/// Record a cross break on a flat face of a sheet metal body.
pub(crate) fn cross_break(
    fid: FeatureId,
    p: &CrossBreak,
    prev: &State,
    state: &mut State,
) -> RResult<()> {
    let (Kind::Face, index) = prev.index_of(&p.face)? else {
        return Err("select a flat face for the cross break".into());
    };
    let record = record_for_face(prev, index)?;
    let body = body_of(prev)?;
    if body.shape.face_info(index).map_err(geo)?.kind != of_geom::SurfaceKind::Plane {
        return Err("a cross break goes on a flat face".into());
    }
    let face = body
        .names
        .face_name(index)
        .ok_or("the face has no name")?
        .to_string();
    state.sheet_metal[record]
        .cross_breaks
        .push(CrossBreakRecord {
            feature: fid,
            face,
            angle: p.angle,
            radius: p.radius,
            up: !p.reverse,
        });
    Ok(())
}

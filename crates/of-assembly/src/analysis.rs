//! Interference, clearance, collision while dragging and mass properties.

use of_mate::{DMat3, DVec3, Pose};
use of_model::geom::{Kind, Shape};
use serde::Serialize;

use crate::assembly::{Assembly, DragTarget, Instance};

/// A part instance's solid in the assembly frame.
pub fn world_shape(i: &Instance) -> Option<Shape> {
    let shape = i.part()?.shape()?;
    shape.located(&i.pose.affine()).ok()
}

/// Volumes below this fraction of the smaller solid count as touching.
const OVERLAP: f64 = 1e-7;

fn overlap_tolerance(a: &Shape, b: &Shape) -> f64 {
    let v = |s: &Shape| s.mass_props().map_or(0.0, |m| m.volume.abs());
    (OVERLAP * v(a).min(v(b))).max(1e-6)
}

fn boxes_touch(a: &Shape, b: &Shape, gap: f64) -> bool {
    match (a.bbox(), b.bbox()) {
        (Some(p), Some(q)) => {
            p.min.x <= q.max.x + gap
                && q.min.x <= p.max.x + gap
                && p.min.y <= q.max.y + gap
                && q.min.y <= p.max.y + gap
                && p.min.z <= q.max.z + gap
                && q.min.z <= p.max.z + gap
        }
        _ => false,
    }
}

#[derive(Debug, Clone, Default)]
pub struct InterferenceOptions {
    /// Check only among these components (and their children); empty: all.
    pub components: Vec<Vec<String>>,
    /// Report touching components as interferences with zero volume.
    pub treat_coincidence: bool,
    /// Don't check components of the same subassembly against each other.
    pub subassemblies_as_components: bool,
    pub include_hidden: bool,
}

pub struct Interference {
    pub a: Vec<String>,
    pub b: Vec<String>,
    /// Shared volume, mm³ (0 for touching with `treat_coincidence`).
    pub volume: f64,
    /// The shared material, for display.
    pub shape: Option<Shape>,
    pub ignored: bool,
}

fn under(key: &[String], roots: &[Vec<String>]) -> bool {
    roots.is_empty() || roots.iter().any(|r| key.starts_with(r))
}

/// Sorted pair of joined keys, as stored in `ignored_interferences`.
pub fn pair_key(a: &[String], b: &[String]) -> [String; 2] {
    let (a, b) = (a.join("/"), b.join("/"));
    if a <= b { [a, b] } else { [b, a] }
}

/// Find overlapping (or, optionally, touching) part instances.
pub fn interference(asm: &mut Assembly, options: &InterferenceOptions) -> Vec<Interference> {
    let instances: Vec<Instance> = asm
        .instances()
        .into_iter()
        .filter(|i| i.part().is_some())
        .filter(|i| options.include_hidden || !i.hidden)
        .filter(|i| under(&i.key, &options.components))
        .collect();
    let shapes: Vec<Option<Shape>> = instances.iter().map(world_shape).collect();
    let ignored = &asm.doc.ignored_interferences;
    let mut out = Vec::new();
    for i in 0..instances.len() {
        for j in i + 1..instances.len() {
            let (a, b) = (&instances[i], &instances[j]);
            if options.subassemblies_as_components
                && a.key.len() > 1
                && b.key.len() > 1
                && a.key[0] == b.key[0]
            {
                continue;
            }
            let (Some(sa), Some(sb)) = (&shapes[i], &shapes[j]) else {
                continue;
            };
            if !boxes_touch(sa, sb, 1e-6) {
                continue;
            }
            let volume = sa.common_volume(sb).unwrap_or(0.0);
            let is_ignored = ignored.contains(&pair_key(&a.key, &b.key));
            if volume > overlap_tolerance(sa, sb) {
                out.push(Interference {
                    a: a.key.clone(),
                    b: b.key.clone(),
                    volume,
                    shape: sa.common(sb).ok(),
                    ignored: is_ignored,
                });
            } else if options.treat_coincidence && sa.distance(sb).is_ok_and(|d| d.value < 1e-6) {
                out.push(Interference {
                    a: a.key.clone(),
                    b: b.key.clone(),
                    volume: 0.0,
                    shape: None,
                    ignored: is_ignored,
                });
            }
        }
    }
    out
}

#[derive(Debug, Clone, Serialize)]
pub struct Clearance {
    pub a: Vec<String>,
    pub b: Vec<String>,
    pub distance: f64,
    pub point_a: [f64; 3],
    pub point_b: [f64; 3],
}

/// Gaps below `minimum` between part instances under `components` (all if
/// empty), smallest first. Interfering pairs report 0.
pub fn clearance(asm: &mut Assembly, components: &[Vec<String>], minimum: f64) -> Vec<Clearance> {
    let instances: Vec<Instance> = asm
        .instances()
        .into_iter()
        .filter(|i| i.part().is_some() && under(&i.key, components))
        .collect();
    let shapes: Vec<Option<Shape>> = instances.iter().map(world_shape).collect();
    let mut out = Vec::new();
    for i in 0..instances.len() {
        for j in i + 1..instances.len() {
            let (Some(sa), Some(sb)) = (&shapes[i], &shapes[j]) else {
                continue;
            };
            if !boxes_touch(sa, sb, minimum) {
                continue;
            }
            if let Ok(d) = sa.distance(sb)
                && d.value < minimum
            {
                out.push(Clearance {
                    a: instances[i].key.clone(),
                    b: instances[j].key.clone(),
                    distance: d.value,
                    point_a: d.a.to_array(),
                    point_b: d.b.to_array(),
                });
            }
        }
    }
    out.sort_by(|a, b| a.distance.total_cmp(&b.distance));
    out
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CollisionOptions {
    /// Check only the dragged component against the others.
    pub dragged_only: bool,
}

/// Where a drag ended and what it ran into.
#[derive(Debug, Clone, Default)]
pub struct DragOutcome {
    pub moved: bool,
    /// Components that collided, if the drag stopped at a collision.
    pub collision: Option<(Vec<String>, Vec<String>)>,
    /// Faces touching the collision, per component key, by part face index.
    pub faces: Vec<(Vec<String>, Vec<u32>)>,
}

fn interpolate(a: &Pose, b: &Pose, t: f64) -> Pose {
    Pose::new(
        a.translation.lerp(b.translation, t),
        a.rotation.slerp(b.rotation, t),
    )
}

impl Assembly {
    /// Drag a component toward a target within its mates. With collision
    /// detection the motion stops just before parts would overlap.
    pub fn drag(
        &mut self,
        key: &[String],
        target: DragTarget,
        collision: Option<CollisionOptions>,
    ) -> Result<DragOutcome, String> {
        let first = key.first().ok_or("select a component to move")?;
        let c = self
            .doc
            .component(first)
            .ok_or_else(|| format!("component {first} no longer exists"))?;
        if c.fixed && key.len() == 1 {
            return Err(format!("{} is fixed; float it to move it", c.name));
        }
        let key = if key.len() >= 2 && c.flexible {
            key[..2].to_vec()
        } else {
            vec![first.clone()]
        };
        let before = self.doc.clone();
        let old = self.instances();
        self.solve(&[(key.clone(), target)], false);
        let mut outcome = DragOutcome::default();
        let new = self.instances();
        outcome.moved = old.iter().zip(&new).any(|(a, b)| {
            a.pose.translation.distance(b.pose.translation) > 1e-9
                || a.pose.rotation.angle_between(b.pose.rotation) > 1e-12
        });
        let Some(options) = collision else {
            return Ok(outcome);
        };
        if !outcome.moved {
            return Ok(outcome);
        }
        // Pairs already overlapping before the drag are not collisions.
        let existing = colliding_pairs(&old, &key, options, &old);
        let solved = self.doc.clone();
        // The positions part way along the motion, solved back onto the mates.
        let state_at = |asm: &mut Assembly, t: f64| {
            asm.doc = before.clone();
            for (c, s) in asm.doc.components.iter_mut().zip(&solved.components) {
                let (a, b): (Pose, Pose) = (c.transform.into(), s.transform.into());
                c.transform = interpolate(&a, &b, t).into();
                c.flexible_poses = s.flexible_poses.clone();
            }
            if t < 1.0 {
                asm.solve(&[], false);
            }
            let inst = asm.instances();
            let pairs: Vec<_> = colliding_pairs(&inst, &key, options, &old)
                .into_iter()
                .filter(|p| !existing.contains(p))
                .collect();
            (pairs, inst)
        };
        // Sample the path finely enough that no part can pass through another
        // between samples: a quarter of the smallest moving part per step.
        let travel = old
            .iter()
            .zip(&new)
            .map(|(a, b)| {
                let size = a
                    .part()
                    .and_then(|p| p.shape())
                    .and_then(|s| s.bbox())
                    .map_or(1.0, |b| b.diagonal());
                a.pose.translation.distance(b.pose.translation)
                    + a.pose.rotation.angle_between(b.pose.rotation) * size
            })
            .fold(0.0f64, f64::max);
        let smallest = new
            .iter()
            .filter(|i| i.key.starts_with(&key))
            .filter_map(|i| i.part().and_then(|p| p.shape()).and_then(|s| s.bbox()))
            .map(|b| (b.max - b.min).min_element().max(1e-3))
            .fold(f64::MAX, f64::min);
        let samples = if smallest.is_finite() && smallest < f64::MAX {
            ((travel / (0.25 * smallest)).ceil() as usize).clamp(1, 64)
        } else {
            1
        };
        let (mut lo, mut hi) = (0.0f64, 1.0f64);
        let mut hit = None;
        let mut collided = false;
        for k in 1..=samples {
            let t = k as f64 / samples as f64;
            let (pairs, inst) = state_at(self, t);
            if pairs.is_empty() {
                lo = t;
            } else {
                hi = t;
                hit = pairs.into_iter().next().map(|p| (p, inst));
                collided = true;
                break;
            }
        }
        if !collided {
            self.doc = solved;
            return Ok(outcome);
        }
        // Back off to the last collision-free position before the contact.
        let mut best = None;
        for _ in 0..10 {
            let t = (lo + hi) / 2.0;
            let (pairs, inst) = state_at(self, t);
            if pairs.is_empty() {
                lo = t;
                best = Some(self.doc.clone());
            } else {
                hi = t;
                hit = pairs.into_iter().next().map(|p| (p, inst));
            }
        }
        self.doc = match best {
            Some(doc) => doc,
            None => {
                state_at(self, lo);
                self.doc.clone()
            }
        };
        self.solve(&[], false);
        if let Some(((a, b), inst)) = hit {
            outcome.faces = touching_faces(&inst, &a, &b);
            outcome.collision = Some((a, b));
        }
        Ok(outcome)
    }
}

/// Pairs of part instances that overlap, involving the moving component
/// (only the dragged one with `dragged_only`), as key pairs.
fn colliding_pairs(
    instances: &[Instance],
    dragged: &[String],
    options: CollisionOptions,
    reference: &[Instance],
) -> Vec<(Vec<String>, Vec<String>)> {
    let moved = |i: &Instance| {
        if options.dragged_only {
            return i.key.starts_with(dragged);
        }
        reference.iter().find(|r| r.key == i.key).is_none_or(|r| {
            r.pose.translation.distance(i.pose.translation) > 1e-9
                || r.pose.rotation.angle_between(i.pose.rotation) > 1e-12
        }) || i.key.starts_with(dragged)
    };
    let parts: Vec<&Instance> = instances.iter().filter(|i| i.part().is_some()).collect();
    let shapes: Vec<Option<Shape>> = parts.iter().map(|i| world_shape(i)).collect();
    let mut out = Vec::new();
    for i in 0..parts.len() {
        for j in i + 1..parts.len() {
            if !moved(parts[i]) && !moved(parts[j]) {
                continue;
            }
            if parts[i].key[0] == parts[j].key[0] && parts[i].key.len() > 1 {
                continue;
            }
            let (Some(a), Some(b)) = (&shapes[i], &shapes[j]) else {
                continue;
            };
            if !boxes_touch(a, b, 0.0) {
                continue;
            }
            if a.common_volume(b).unwrap_or(0.0) > overlap_tolerance(a, b) {
                out.push((parts[i].key.clone(), parts[j].key.clone()));
            }
        }
    }
    out
}

/// Faces of two instances that touch the material they share.
fn touching_faces(
    instances: &[Instance],
    a: &[String],
    b: &[String],
) -> Vec<(Vec<String>, Vec<u32>)> {
    let find = |k: &[String]| instances.iter().find(|i| i.key == k);
    let (Some(ia), Some(ib)) = (find(a), find(b)) else {
        return Vec::new();
    };
    let (Some(sa), Some(sb)) = (world_shape(ia), world_shape(ib)) else {
        return Vec::new();
    };
    let Ok(common) = sa.common(&sb) else {
        return Vec::new();
    };
    let faces = |s: &Shape| -> Vec<u32> {
        (0..s.count(Kind::Face))
            .filter(|f| {
                s.sub_shape(Kind::Face, *f)
                    .and_then(|face| face.distance(&common))
                    .is_ok_and(|d| d.value < 1e-6)
            })
            .collect()
    };
    vec![(a.to_vec(), faces(&sa)), (b.to_vec(), faces(&sb))]
}

#[derive(Debug, Clone, Serialize)]
pub struct AssemblyMass {
    /// kg
    pub mass: f64,
    /// mm³
    pub volume: f64,
    /// mm²
    pub area: f64,
    /// mm, assembly frame
    pub center: [f64; 3],
    /// kg·mm² about the center of mass, assembly axes, row-major.
    pub inertia: [[f64; 3]; 3],
    pub components: usize,
}

/// Mass properties of every part instance, each with its own material.
pub fn mass_properties(asm: &mut Assembly) -> Result<AssemblyMass, String> {
    let mut parts = Vec::new();
    for i in asm.instances() {
        let Some(p) = i.part() else { continue };
        let Ok(mp) = of_model::measure::mass_properties(p.doc(), p.state()) else {
            continue;
        };
        parts.push((i.pose, mp));
    }
    if parts.is_empty() {
        return Err("the assembly has no solid parts".into());
    }
    let mass: f64 = parts.iter().map(|(_, m)| m.mass).sum();
    let volume: f64 = parts.iter().map(|(_, m)| m.volume).sum();
    let area: f64 = parts.iter().map(|(_, m)| m.area).sum();
    let center = if mass > 0.0 {
        parts
            .iter()
            .map(|(p, m)| p.point(m.center) * m.mass)
            .sum::<DVec3>()
            / mass
    } else {
        DVec3::ZERO
    };
    let mut inertia = DMat3::ZERO;
    for (p, m) in &parts {
        let r = DMat3::from_quat(p.rotation);
        let d = p.point(m.center) - center;
        let shift = DMat3::from_diagonal(DVec3::splat(d.length_squared()))
            - DMat3::from_cols(d * d.x, d * d.y, d * d.z);
        inertia += r * m.inertia * r.transpose() + shift * m.mass;
    }
    let rows = |m: DMat3| {
        let t = m.transpose();
        [
            t.x_axis.to_array(),
            t.y_axis.to_array(),
            t.z_axis.to_array(),
        ]
    };
    Ok(AssemblyMass {
        mass,
        volume,
        area,
        center: center.to_array(),
        inertia: rows(inertia),
        components: parts.len(),
    })
}

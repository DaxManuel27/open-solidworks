//! Exploded views: display-only offsets applied step by step on top of the
//! solved positions, with lines tracing each component's travel.

use std::collections::HashMap;

use of_mate::{DQuat, DVec3, Pose};

use crate::assembly::Instance;
use crate::document::{ExplodeMotion, ExplodeStep};

pub struct Exploded {
    /// Display pose of every instance, by key.
    pub poses: HashMap<Vec<String>, Pose>,
    /// One polyline per component per translating step: start and end of
    /// the component's center.
    pub lines: Vec<[DVec3; 2]>,
}

/// World bounding-box center of an instance and its children.
fn center(instances: &[Instance], poses: &HashMap<Vec<String>, Pose>, root: &[String]) -> DVec3 {
    let mut lo = DVec3::splat(f64::MAX);
    let mut hi = DVec3::splat(f64::MIN);
    for i in instances.iter().filter(|i| i.key.starts_with(root)) {
        let Some(b) = i.part().and_then(|p| p.shape()).and_then(|s| s.bbox()) else {
            continue;
        };
        let pose = poses.get(&i.key).copied().unwrap_or(i.pose);
        for x in [b.min.x, b.max.x] {
            for y in [b.min.y, b.max.y] {
                for z in [b.min.z, b.max.z] {
                    let p = pose.point(DVec3::new(x, y, z));
                    lo = lo.min(p);
                    hi = hi.max(p);
                }
            }
        }
    }
    if lo.x > hi.x {
        return poses.get(root).map_or(DVec3::ZERO, |p| p.translation);
    }
    (lo + hi) * 0.5
}

/// Apply the first `count` steps (all when `None`).
pub fn explode(instances: &[Instance], steps: &[ExplodeStep], count: Option<usize>) -> Exploded {
    let mut poses: HashMap<Vec<String>, Pose> =
        instances.iter().map(|i| (i.key.clone(), i.pose)).collect();
    let mut lines = Vec::new();
    for step in steps.iter().take(count.unwrap_or(steps.len())) {
        for (n, root) in step.components.iter().enumerate() {
            let before = center(instances, &poses, root);
            let t = match &step.motion {
                ExplodeMotion::Translate {
                    direction,
                    distance,
                    auto_space,
                } => {
                    let d = DVec3::from(*direction).normalize_or_zero();
                    let k = if *auto_space { (n + 1) as f64 } else { 1.0 };
                    Pose::from_translation(d * distance * k)
                }
                ExplodeMotion::Rotate {
                    origin,
                    axis,
                    angle,
                } => {
                    let (o, a) = (DVec3::from(*origin), DVec3::from(*axis).normalize_or_zero());
                    let q = DQuat::from_axis_angle(a, *angle);
                    Pose::new(o - q * o, q)
                }
                ExplodeMotion::Radial {
                    origin,
                    axis,
                    distance,
                } => {
                    let (o, a) = (DVec3::from(*origin), DVec3::from(*axis).normalize_or_zero());
                    let out = (before - o).reject_from_normalized(a);
                    let d = if out.length() < 1e-9 {
                        a.any_orthonormal_vector()
                    } else {
                        out.normalize()
                    };
                    Pose::from_translation(d * *distance)
                }
            };
            for (key, pose) in poses.iter_mut() {
                if key.starts_with(root) {
                    *pose = t.then(pose);
                }
            }
            if !matches!(step.motion, ExplodeMotion::Rotate { .. }) {
                lines.push([before, center(instances, &poses, root)]);
            }
        }
    }
    Exploded { poses, lines }
}

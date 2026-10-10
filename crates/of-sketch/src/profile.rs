//! Finding closed contours in a sketch and the regions they bound.

use std::collections::{BTreeMap, HashMap};

use glam::DVec2;
use serde::{Deserialize, Serialize};

use crate::geom2::{Curve2, intersect, point_in_polygon, signed_area};
use crate::{ConstraintKind, Geometry, Id, Sketch};

/// A chain of sketch curves joined end to end.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chain {
    pub entities: Vec<Id>,
    pub curves: Vec<Curve2>,
}

/// A closed region: an outer loop and the loops cut out of it. Indices
/// point into [`Contours::loops`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Region {
    pub outer: usize,
    pub holes: Vec<usize>,
    pub area: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contours {
    /// Closed loops.
    pub loops: Vec<Chain>,
    /// Open chains, usable by thin features.
    pub open: Vec<Chain>,
    /// Regions bounded by the loops, alternating material and holes from
    /// the outside in.
    pub regions: Vec<Region>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize, Deserialize)]
pub enum ProfileError {
    #[error("the sketch has no geometry")]
    Empty,
    #[error("contours branch at {} point(s); more than two curves meet there", .0.len())]
    Branching(Vec<DVec2>),
    #[error("contours cross each other at {} point(s)", .0.len())]
    SelfIntersecting(Vec<DVec2>),
}

struct UnionFind(HashMap<Id, Id>);

impl UnionFind {
    fn find(&mut self, x: Id) -> Id {
        let p = *self.0.get(&x).unwrap_or(&x);
        if p == x {
            return x;
        }
        let r = self.find(p);
        self.0.insert(x, r);
        r
    }
    fn union(&mut self, a: Id, b: Id) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.0.insert(ra.max(rb), ra.min(rb));
        }
    }
}

/// Endpoints of an open curve entity.
fn ends(sk: &Sketch, id: Id) -> Option<(Id, Id)> {
    match &sk.entities[&id].geo {
        Geometry::Line { a, b } => Some((*a, *b)),
        Geometry::Arc { start, end, .. } => Some((*start, *end)),
        Geometry::Spline {
            points,
            closed: false,
        } => Some((points[0], *points.last().unwrap())),
        _ => None,
    }
}

pub(crate) fn contours(sk: &Sketch) -> Result<Contours, ProfileError> {
    let ids: Vec<Id> = sk
        .curve_ids()
        .filter(|id| !sk.entities[id].construction)
        .filter(|id| sk.curve(*id).map(|c| c.length() > 1e-9).unwrap_or(false))
        .collect();
    if ids.is_empty() {
        return Err(ProfileError::Empty);
    }

    // Points joined by coincident relations, or at the same place, are one node.
    let mut uf = UnionFind(HashMap::new());
    for c in sk.constraints.values() {
        if let ConstraintKind::Coincident { a, b } = c.kind {
            uf.union(a, b);
        }
    }
    let endpoints: Vec<Id> = ids
        .iter()
        .filter_map(|id| ends(sk, *id))
        .flat_map(|(a, b)| [a, b])
        .collect();
    for (i, &a) in endpoints.iter().enumerate() {
        for &b in &endpoints[i + 1..] {
            if (sk.point(a).unwrap() - sk.point(b).unwrap()).length() < 1e-6 {
                uf.union(a, b);
            }
        }
    }

    let mut loops = Vec::new();
    let mut open = Vec::new();
    let mut adjacency: BTreeMap<Id, Vec<Id>> = BTreeMap::new();
    for &id in &ids {
        match ends(sk, id) {
            Some((a, b)) => {
                let (a, b) = (uf.find(a), uf.find(b));
                adjacency.entry(a).or_default().push(id);
                adjacency.entry(b).or_default().push(id);
            }
            None => loops.push(Chain {
                entities: vec![id],
                curves: vec![sk.curve(id).unwrap()],
            }),
        }
    }

    let branching: Vec<DVec2> = adjacency
        .iter()
        .filter(|(_, e)| e.len() > 2)
        .map(|(p, _)| sk.point(*p).unwrap())
        .collect();
    if !branching.is_empty() {
        return Err(ProfileError::Branching(branching));
    }

    // Walk chains, starting from dead ends first so open chains come out whole.
    let mut used: Vec<Id> = Vec::new();
    let mut starts: Vec<Id> = adjacency
        .iter()
        .filter(|(_, e)| e.len() == 1)
        .map(|(p, _)| *p)
        .collect();
    starts.extend(adjacency.keys().copied());
    for start in starts {
        while let Some(&first) = adjacency[&start].iter().find(|e| !used.contains(e)) {
            let mut chain = Chain {
                entities: vec![],
                curves: vec![],
            };
            let mut node = start;
            let mut edge = first;
            loop {
                used.push(edge);
                let (a, b) = ends(sk, edge).unwrap();
                let (ra, rb) = (uf.find(a), uf.find(b));
                let mut curve = sk.curve(edge).unwrap();
                let next = if ra == node { rb } else { ra };
                if ra != node
                    && let Curve2::Line { a, b } = curve
                {
                    curve = Curve2::Line { a: b, b: a };
                }
                chain.entities.push(edge);
                chain.curves.push(curve);
                node = next;
                match adjacency[&node].iter().find(|e| !used.contains(e)) {
                    Some(&e) => edge = e,
                    None => break,
                }
            }
            if node == start && adjacency[&start].len() == 2 {
                loops.push(chain);
            } else {
                open.push(chain);
            }
        }
    }

    // Profile curves may only meet at shared endpoints.
    let all: Vec<(Id, Curve2)> = ids.iter().map(|id| (*id, sk.curve(*id).unwrap())).collect();
    let mut crossings = Vec::new();
    for (i, (_, a)) in all.iter().enumerate() {
        for (_, b) in &all[i + 1..] {
            for p in intersect(a, b) {
                let at_end = |c: &Curve2| {
                    !c.is_closed()
                        && ((c.start() - p).length() < 1e-6 || (c.end() - p).length() < 1e-6)
                };
                if !(at_end(a) && at_end(b)) {
                    crossings.push(p);
                }
            }
        }
    }
    if !crossings.is_empty() {
        return Err(ProfileError::SelfIntersecting(crossings));
    }

    let regions = regions(&loops);
    Ok(Contours {
        loops,
        open,
        regions,
    })
}

fn polygon(chain: &Chain) -> Vec<DVec2> {
    let mut pts: Vec<DVec2> = Vec::new();
    for c in &chain.curves {
        let mut s = c.sample(0.01);
        // Orient each piece to follow the chain.
        if let Some(last) = pts.last()
            && (s[0] - *last).length() > (s[s.len() - 1] - *last).length()
        {
            s.reverse();
        }
        pts.extend(s);
    }
    pts
}

fn regions(loops: &[Chain]) -> Vec<Region> {
    let polys: Vec<Vec<DVec2>> = loops.iter().map(polygon).collect();
    let areas: Vec<f64> = polys.iter().map(|p| signed_area(p).abs()).collect();
    // A loop's parent is the smallest loop containing it.
    let parent: Vec<Option<usize>> = (0..loops.len())
        .map(|i| {
            let probe = polys[i][polys[i].len() / 2];
            (0..loops.len())
                .filter(|&j| j != i && areas[j] > areas[i] && point_in_polygon(probe, &polys[j]))
                .min_by(|&a, &b| areas[a].total_cmp(&areas[b]))
        })
        .collect();
    let depth = |mut i: usize| {
        let mut d = 0;
        while let Some(p) = parent[i] {
            d += 1;
            i = p;
        }
        d
    };
    (0..loops.len())
        .filter(|&i| depth(i) % 2 == 0)
        .map(|i| {
            let holes: Vec<usize> = (0..loops.len()).filter(|&j| parent[j] == Some(i)).collect();
            let area = areas[i] - holes.iter().map(|&h| areas[h]).sum::<f64>();
            Region {
                outer: i,
                holes,
                area,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools;

    #[test]
    fn plate_with_hole_and_island() {
        let mut sk = Sketch::new();
        tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(100.0, 50.0));
        sk.add_circle_at(DVec2::new(30.0, 25.0), 15.0);
        sk.add_circle_at(DVec2::new(30.0, 25.0), 5.0); // island inside the hole
        let c = sk.contours().unwrap();
        assert_eq!(c.loops.len(), 3);
        assert_eq!(c.regions.len(), 2);
        let plate = c.regions.iter().find(|r| r.holes.len() == 1).unwrap();
        let expected = 5000.0 - std::f64::consts::PI * 225.0;
        assert!((plate.area - expected).abs() < 2.0, "{}", plate.area);
    }

    #[test]
    fn open_chain_and_errors() {
        let mut sk = Sketch::new();
        let (_, _, b) = sk.add_line_at(DVec2::ZERO, DVec2::new(10.0, 0.0));
        let c = sk.add_point(DVec2::new(10.0, 10.0));
        sk.add_line(b, c).unwrap();
        let k = sk.contours().unwrap();
        assert_eq!(k.open.len(), 1);
        assert_eq!(k.open[0].entities.len(), 2);
        assert!(k.regions.is_empty());

        let d = sk.add_point(DVec2::new(20.0, 0.0));
        sk.add_line(b, d).unwrap();
        assert!(matches!(sk.contours(), Err(ProfileError::Branching(_))));

        let mut sk = Sketch::new();
        sk.add_line_at(DVec2::new(-5.0, 0.0), DVec2::new(5.0, 0.0));
        sk.add_circle_at(DVec2::ZERO, 2.0);
        assert!(matches!(
            sk.contours(),
            Err(ProfileError::SelfIntersecting(_))
        ));
    }

    #[test]
    fn construction_geometry_is_ignored() {
        let mut sk = Sketch::new();
        tools::corner_rectangle(&mut sk, DVec2::ZERO, DVec2::new(10.0, 10.0));
        let (l, _, _) = sk.add_line_at(DVec2::ZERO, DVec2::new(10.0, 10.0));
        sk.entities.get_mut(&l).unwrap().construction = true;
        assert_eq!(sk.contours().unwrap().regions.len(), 1);
    }
}

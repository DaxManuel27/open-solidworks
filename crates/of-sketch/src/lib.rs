//! 2D sketches: entities, dimensions and relations, constraint solving,
//! and finding closed profiles for features to consume.
//!
//! Lines, arcs and splines refer to point entities by id, so connected
//! geometry shares points and stays connected while it is solved. Every
//! sketch has a fixed origin point with id [`ORIGIN`].

mod constraint;
pub mod geom2;
mod profile;
mod solver;
pub mod tools;

pub use constraint::{Constraint, ConstraintKind};
pub use geom2::Curve2;
pub use glam::DVec2;
pub use profile::{Chain, Contours, ProfileError, Region};
pub use solver::{Definition, SolveReport, SolveStatus};

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Entity and constraint id. Entities and constraints share one id space.
pub type Id = u32;

/// The sketch origin point, always present and fixed at (0, 0).
pub const ORIGIN: Id = 0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Geometry {
    Point {
        p: DVec2,
    },
    Line {
        a: Id,
        b: Id,
    },
    Circle {
        center: Id,
        radius: f64,
    },
    /// Counter-clockwise from `start` to `end` around `center`.
    Arc {
        center: Id,
        start: Id,
        end: Id,
    },
    /// Interpolating spline through `points`.
    Spline {
        points: Vec<Id>,
        closed: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub geo: Geometry,
    /// Construction geometry helps constrain the sketch but is not part of
    /// any profile.
    #[serde(default)]
    pub construction: bool,
    /// Set for geometry converted from model edges; the model updates it on
    /// rebuild and it is held fixed while solving.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external: Option<String>,
}

impl Entity {
    pub fn new(geo: Geometry) -> Entity {
        Entity {
            geo,
            construction: false,
            external: None,
        }
    }

    /// Points this entity is built on (its own id for a point).
    pub fn point_refs(&self) -> Vec<Id> {
        match &self.geo {
            Geometry::Point { .. } => vec![],
            Geometry::Line { a, b } => vec![*a, *b],
            Geometry::Circle { center, .. } => vec![*center],
            Geometry::Arc { center, start, end } => vec![*center, *start, *end],
            Geometry::Spline { points, .. } => points.clone(),
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SketchError {
    #[error("no sketch entity {0}")]
    NoEntity(Id),
    #[error("entity {0} is not a {1}")]
    WrongKind(Id, &'static str),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, SketchError>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sketch {
    pub entities: BTreeMap<Id, Entity>,
    pub constraints: BTreeMap<Id, Constraint>,
    next_id: Id,
}

impl Default for Sketch {
    fn default() -> Self {
        Self::new()
    }
}

impl Sketch {
    pub fn new() -> Sketch {
        let mut entities = BTreeMap::new();
        entities.insert(ORIGIN, Entity::new(Geometry::Point { p: DVec2::ZERO }));
        Sketch {
            entities,
            constraints: BTreeMap::new(),
            next_id: 1,
        }
    }

    fn alloc(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn entity(&self, id: Id) -> Result<&Entity> {
        self.entities.get(&id).ok_or(SketchError::NoEntity(id))
    }

    pub fn add_entity(&mut self, e: Entity) -> Result<Id> {
        for p in e.point_refs() {
            match self.entity(p)?.geo {
                Geometry::Point { .. } => {}
                _ => return Err(SketchError::WrongKind(p, "point")),
            }
        }
        let id = self.alloc();
        self.entities.insert(id, e);
        Ok(id)
    }

    pub fn add_point(&mut self, p: DVec2) -> Id {
        let id = self.alloc();
        self.entities.insert(id, Entity::new(Geometry::Point { p }));
        id
    }

    /// Add a line between two existing points.
    pub fn add_line(&mut self, a: Id, b: Id) -> Result<Id> {
        self.add_entity(Entity::new(Geometry::Line { a, b }))
    }

    /// Add a line with new endpoints at `a` and `b`. Returns (line, a, b).
    pub fn add_line_at(&mut self, a: DVec2, b: DVec2) -> (Id, Id, Id) {
        let pa = self.add_point(a);
        let pb = self.add_point(b);
        (self.add_line(pa, pb).expect("new points"), pa, pb)
    }

    pub fn add_circle(&mut self, center: Id, radius: f64) -> Result<Id> {
        self.add_entity(Entity::new(Geometry::Circle { center, radius }))
    }

    pub fn add_circle_at(&mut self, center: DVec2, radius: f64) -> Id {
        let c = self.add_point(center);
        self.add_circle(c, radius).expect("new point")
    }

    pub fn add_arc(&mut self, center: Id, start: Id, end: Id) -> Result<Id> {
        self.add_entity(Entity::new(Geometry::Arc { center, start, end }))
    }

    pub fn add_spline(&mut self, points: Vec<Id>, closed: bool) -> Result<Id> {
        if points.len() < 2 {
            return Err(SketchError::Invalid(
                "a spline needs at least two points".into(),
            ));
        }
        self.add_entity(Entity::new(Geometry::Spline { points, closed }))
    }

    pub fn add_constraint(&mut self, kind: ConstraintKind) -> Result<Id> {
        let mut c = Constraint::new(kind);
        c.validate(self)?;
        c.prepare(self);
        let id = self.alloc();
        self.constraints.insert(id, c);
        Ok(id)
    }

    /// Add a constraint whose value (if any) is taken from the current
    /// geometry, so adding it doesn't move anything. Used for dimensions
    /// placed by the user before typing a value.
    pub fn add_measured(&mut self, kind: ConstraintKind) -> Result<Id> {
        let mut c = Constraint::new(kind);
        c.validate(self)?;
        c.measure_from(self);
        let id = self.alloc();
        self.constraints.insert(id, c);
        Ok(id)
    }

    /// Change a dimension's value.
    pub fn set_value(&mut self, constraint: Id, value: f64) -> Result<()> {
        let c = self
            .constraints
            .get_mut(&constraint)
            .ok_or(SketchError::NoEntity(constraint))?;
        c.kind.set_value(value).map_err(SketchError::Invalid)
    }

    /// Make a dimension driven (reference only) or driving.
    pub fn set_driving(&mut self, constraint: Id, driving: bool) -> Result<()> {
        let c = self
            .constraints
            .get_mut(&constraint)
            .ok_or(SketchError::NoEntity(constraint))?;
        c.driving = driving;
        Ok(())
    }

    /// Delete entities or constraints. Deleting an entity also deletes the
    /// constraints on it, and deleting a point deletes the curves built on it.
    /// Points left unused by deleted curves are removed too.
    pub fn delete(&mut self, ids: &[Id]) {
        let mut doomed: BTreeSet<Id> = ids.iter().copied().filter(|&i| i != ORIGIN).collect();
        // Curves built on deleted points.
        loop {
            let more: Vec<Id> = self
                .entities
                .iter()
                .filter(|(id, e)| {
                    !doomed.contains(id) && e.point_refs().iter().any(|p| doomed.contains(p))
                })
                .map(|(id, _)| *id)
                .collect();
            if more.is_empty() {
                break;
            }
            doomed.extend(more);
        }
        // Points that only served deleted curves.
        let orphan_candidates: BTreeSet<Id> = doomed
            .iter()
            .filter_map(|id| self.entities.get(id))
            .flat_map(|e| e.point_refs())
            .collect();
        for id in &doomed {
            self.entities.remove(id);
            self.constraints.remove(id);
        }
        for p in orphan_candidates {
            if p != ORIGIN && !self.entities.values().any(|e| e.point_refs().contains(&p)) {
                self.entities.remove(&p);
            }
        }
        self.constraints
            .retain(|_, c| c.kind.refs().iter().all(|r| self.entities.contains_key(r)));
    }

    /// Position of a point entity.
    pub fn point(&self, id: Id) -> Result<DVec2> {
        match self.entity(id)?.geo {
            Geometry::Point { p } => Ok(p),
            _ => Err(SketchError::WrongKind(id, "point")),
        }
    }

    pub fn set_point(&mut self, id: Id, p: DVec2) -> Result<()> {
        match &mut self
            .entities
            .get_mut(&id)
            .ok_or(SketchError::NoEntity(id))?
            .geo
        {
            Geometry::Point { p: q } => {
                *q = p;
                Ok(())
            }
            _ => Err(SketchError::WrongKind(id, "point")),
        }
    }

    /// Endpoints of a line.
    pub fn line(&self, id: Id) -> Result<(DVec2, DVec2)> {
        match self.entity(id)?.geo {
            Geometry::Line { a, b } => Ok((self.point(a)?, self.point(b)?)),
            _ => Err(SketchError::WrongKind(id, "line")),
        }
    }

    /// Center and radius of a circle or arc.
    pub fn circle(&self, id: Id) -> Result<(DVec2, f64)> {
        match self.entity(id)?.geo {
            Geometry::Circle { center, radius } => Ok((self.point(center)?, radius)),
            Geometry::Arc { center, start, .. } => {
                let c = self.point(center)?;
                Ok((c, (self.point(start)? - c).length()))
            }
            _ => Err(SketchError::WrongKind(id, "circle or arc")),
        }
    }

    /// The 2D curve for a non-point entity, using current positions.
    pub fn curve(&self, id: Id) -> Result<Curve2> {
        Ok(match &self.entity(id)?.geo {
            Geometry::Point { .. } => return Err(SketchError::WrongKind(id, "curve")),
            Geometry::Line { a, b } => Curve2::Line {
                a: self.point(*a)?,
                b: self.point(*b)?,
            },
            Geometry::Circle { center, radius } => Curve2::Circle {
                center: self.point(*center)?,
                radius: *radius,
            },
            Geometry::Arc { center, start, end } => Curve2::Arc {
                center: self.point(*center)?,
                start: self.point(*start)?,
                end: self.point(*end)?,
            },
            Geometry::Spline { points, closed } => Curve2::Spline {
                points: points
                    .iter()
                    .map(|p| self.point(*p))
                    .collect::<Result<_>>()?,
                closed: *closed,
            },
        })
    }

    /// Ids of curve entities (everything but points).
    pub fn curve_ids(&self) -> impl Iterator<Item = Id> + '_ {
        self.entities
            .iter()
            .filter(|(_, e)| !matches!(e.geo, Geometry::Point { .. }))
            .map(|(id, _)| *id)
    }

    /// Solve all constraints, moving geometry as little as possible.
    pub fn solve(&mut self) -> SolveReport {
        solver::solve(self, &[])
    }

    /// Move a point toward `target` while keeping constraints satisfied.
    pub fn drag(&mut self, point: Id, target: DVec2) -> SolveReport {
        solver::solve(self, &[(point, target)])
    }

    /// Analyse which geometry is fully defined, without moving anything.
    pub fn analyse(&self) -> SolveReport {
        solver::analyse(self)
    }

    /// Find closed contours and the regions they bound.
    pub fn contours(&self) -> std::result::Result<Contours, ProfileError> {
        profile::contours(self)
    }
}

//! An open assembly: its document, loaded files and solved positions.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use of_mate::{DVec3, Pose, System, Term};
use of_model::{Document, Model, State};

use crate::cache::{Cache, Part, SubAssembly, canonical};
use crate::document::{AssemblyDocument, CompRef, Component, Mate, MateKind};
use crate::geometry::{self, Geo};
use crate::mates::{self, Context, Located};
use crate::patterns::{self, Derived};

/// What a component's file turned out to be.
#[derive(Clone)]
pub enum Loaded {
    Part(Rc<Part>),
    Assembly(Rc<SubAssembly>),
    Missing(String),
}

/// One component instance anywhere in the tree, at its world pose.
#[derive(Clone)]
pub struct Instance {
    /// Instance keys from the top.
    pub key: Vec<String>,
    pub name: String,
    pub pose: Pose,
    pub loaded: Loaded,
    pub file: PathBuf,
    pub configuration: Option<u32>,
    pub hidden: bool,
    pub fixed: bool,
    pub exclude_from_bom: bool,
    /// Pattern or mirror feature that generated this instance.
    pub derived: Option<u32>,
}

impl Instance {
    pub fn part(&self) -> Option<&Rc<Part>> {
        match &self.loaded {
            Loaded::Part(p) => Some(p),
            _ => None,
        }
    }

    pub fn key_string(&self) -> String {
        self.key.join("/")
    }
}

/// Solve results for display: per-component freedom and mate errors.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Diagnostics {
    pub solved: bool,
    /// Remaining DOF by component key path (`"3"`, `"4/1"`).
    pub dof: BTreeMap<String, u32>,
    pub free: u32,
    /// Mate ID (top-level) to the reason it fails.
    pub mate_errors: BTreeMap<u32, String>,
    pub conflicting: Vec<u32>,
    pub redundant: Vec<u32>,
    /// Component key to load or build errors.
    pub component_errors: BTreeMap<String, String>,
}

/// A request to drag a component while solving.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DragTarget {
    /// Move the component toward this whole world pose.
    Pose(Pose),
    /// Move a point on the component (in its own frame) toward a world point.
    Point { local: DVec3, target: DVec3 },
}

/// Solver bodies: top-level components and flexible subassembly children.
pub(crate) struct Bodies {
    pub keys: Vec<Vec<String>>,
    pub poses: Vec<Pose>,
    pub fixed: Vec<bool>,
    index: HashMap<Vec<String>, usize>,
    /// Locks holding fixed flexible children to their subassembly.
    pub locks: Vec<Term>,
    /// Flexible subassemblies' own mates, with component paths prefixed.
    pub sub_mates: Vec<(Vec<String>, Mate)>,
}

impl Bodies {
    pub fn body(&self, key: &[String]) -> Option<usize> {
        self.index.get(key).copied()
    }
}

/// Mates whose terms were built, in solver order.
struct Built {
    mates: Vec<of_mate::Mate>,
    /// Solver mate index -> (subassembly prefix, mate ID).
    origin: Vec<(Vec<String>, u32)>,
    captured: Vec<crate::Captured>,
}

pub struct Assembly {
    pub doc: AssemblyDocument,
    path: Option<PathBuf>,
    pub cache: Cache,
    reference: Option<(Document, Model)>,
    pub diagnostics: Diagnostics,
}

impl Default for Assembly {
    fn default() -> Self {
        Assembly::new(AssemblyDocument::new())
    }
}

/// `path` relative to `base`, with forward slashes; absolute if no common root.
pub fn relative_to(path: &Path, base: &Path) -> String {
    let path = canonical(path);
    let base = canonical(base);
    let p: Vec<_> = path.components().collect();
    let b: Vec<_> = base.components().collect();
    let common = p.iter().zip(&b).take_while(|(x, y)| x == y).count();
    if common == 0 {
        return path.to_string_lossy().replace('\\', "/");
    }
    let mut parts: Vec<String> = Vec::new();
    for _ in common..b.len() {
        parts.push("..".into());
    }
    for c in &p[common..] {
        parts.push(c.as_os_str().to_string_lossy().into_owned());
    }
    parts.join("/")
}

impl Assembly {
    pub fn new(doc: AssemblyDocument) -> Assembly {
        Assembly {
            doc,
            path: None,
            cache: Cache::default(),
            reference: None,
            diagnostics: Diagnostics {
                solved: true,
                ..Default::default()
            },
        }
    }

    pub fn open(path: &Path) -> Result<Assembly, String> {
        let doc = crate::file::load(path)?;
        let mut a = Assembly::new(doc);
        a.path = Some(path.to_path_buf());
        Ok(a)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Where relative component paths are resolved from.
    pub fn base_dir(&self) -> PathBuf {
        self.path
            .as_ref()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
    }

    /// Save, rewriting component paths relative to the new location.
    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        let new_dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let old_dir = self.base_dir();
        let mut doc = self.doc.clone();
        for c in &mut doc.components {
            let abs = resolve_file(&old_dir, &c.path);
            if abs.exists() {
                c.path = relative_to(&abs, &new_dir);
            }
        }
        for m in &mut doc.mirrors {
            for item in &mut m.components {
                if let Some(f) = &item.opposite_hand {
                    let abs = resolve_file(&old_dir, f);
                    if abs.exists() {
                        item.opposite_hand = Some(relative_to(&abs, &new_dir));
                    }
                }
            }
        }
        crate::file::save(&doc, path)?;
        self.doc = doc;
        self.path = Some(path.to_path_buf());
        Ok(())
    }

    /// Absolute path of a component's file.
    pub fn file_of(&self, c: &Component) -> PathBuf {
        resolve_file(&self.base_dir(), &c.path)
    }

    /// The assembly's own reference geometry, built from `doc.reference`.
    pub fn reference_state(&mut self) -> &State {
        let stale = self
            .reference
            .as_ref()
            .is_none_or(|(d, _)| *d != self.doc.reference);
        if stale {
            let mut m = Model::new(self.doc.reference.clone());
            m.rebuild();
            self.reference = Some((self.doc.reference.clone(), m));
        }
        self.reference.as_ref().unwrap().1.state()
    }

    pub fn reference_model(&mut self) -> &Model {
        self.reference_state();
        &self.reference.as_ref().unwrap().1
    }

    /// Load a component's file relative to `dir`, in `configuration`.
    pub fn load_in(
        cache: &mut Cache,
        dir: &Path,
        c: &Component,
        configuration: Option<u32>,
    ) -> Loaded {
        let file = resolve_file(dir, &c.path);
        let result = if crate::file::is_assembly(&file) {
            cache.assembly(&file).map(Loaded::Assembly)
        } else {
            cache.part(&file, configuration).map(Loaded::Part)
        };
        result.unwrap_or_else(Loaded::Missing)
    }

    pub fn load(&mut self, c: &Component) -> Loaded {
        let dir = self.base_dir();
        let configuration = self.doc.part_configuration(c);
        Assembly::load_in(&mut self.cache, &dir, c, configuration)
    }

    /// Every component instance, depth first, with subassembly nodes before
    /// their children, followed by pattern and mirror instances.
    pub fn instances(&mut self) -> Vec<Instance> {
        let mut out = Vec::new();
        let dir = self.base_dir();
        let doc = self.doc.clone();
        let mut stack = Vec::new();
        if let Some(p) = &self.path {
            stack.push(canonical(p));
        }
        expand(
            &mut self.cache,
            &doc,
            &dir,
            &[],
            Pose::IDENTITY,
            &mut stack,
            &mut out,
            true,
        );
        let derived = self.derived(&out);
        for d in derived {
            out.extend(d.instances);
        }
        out
    }

    /// Pattern and mirror instances from current component poses.
    pub(crate) fn derived(&mut self, base: &[Instance]) -> Vec<Derived> {
        let mut out = Vec::new();
        let doc = self.doc.clone();
        for p in doc.patterns.iter().filter(|p| !p.suppressed) {
            match patterns::pattern(self, p, base) {
                Ok(d) => out.push(d),
                Err(e) => {
                    self.diagnostics
                        .component_errors
                        .insert(format!("pattern {}", p.id), e);
                }
            }
        }
        for m in doc.mirrors.iter().filter(|m| !m.suppressed) {
            match patterns::mirror(self, m, base) {
                Ok(d) => out.push(d),
                Err(e) => {
                    self.diagnostics
                        .component_errors
                        .insert(format!("mirror {}", m.id), e);
                }
            }
        }
        out
    }

    /// Solver bodies for the current document.
    pub(crate) fn bodies(&mut self) -> Bodies {
        let mut b = Bodies {
            keys: Vec::new(),
            poses: Vec::new(),
            fixed: Vec::new(),
            index: HashMap::new(),
            locks: Vec::new(),
            sub_mates: Vec::new(),
        };
        let components: Vec<Component> = self
            .doc
            .components
            .iter()
            .filter(|c| !self.doc.is_suppressed(c))
            .cloned()
            .collect();
        for c in components {
            let key = vec![c.id.to_string()];
            let pose: Pose = c.transform.into();
            let i = b.keys.len();
            b.index.insert(key.clone(), i);
            b.keys.push(key.clone());
            b.poses.push(pose);
            b.fixed.push(c.fixed);
            if !c.flexible {
                continue;
            }
            let Loaded::Assembly(sub) = self.load(&c) else {
                continue;
            };
            for child in sub
                .doc
                .components
                .iter()
                .filter(|k| !sub.doc.is_suppressed(k))
            {
                let child_key = child.id.to_string();
                let rel: Pose = c
                    .flexible_poses
                    .get(&child_key)
                    .copied()
                    .unwrap_or(child.transform)
                    .into();
                let j = b.keys.len();
                let key = vec![c.id.to_string(), child_key];
                b.index.insert(key.clone(), j);
                b.keys.push(key);
                b.poses.push(pose.then(&rel));
                b.fixed.push(false);
                if child.fixed {
                    b.locks.push(Term::Lock {
                        a: Some(i),
                        b: Some(j),
                        relative: child.transform.into(),
                    });
                }
            }
            for m in sub
                .doc
                .mates
                .iter()
                .filter(|m| !sub.doc.is_mate_suppressed(m))
            {
                b.sub_mates.push((vec![c.id.to_string()], m.clone()));
            }
        }
        b
    }

    /// Resolve a reference for solving against the given bodies.
    pub(crate) fn locate(
        &mut self,
        r: &CompRef,
        bodies: &Bodies,
        derived: &[Instance],
    ) -> Result<Located, String> {
        if r.component.is_empty() {
            let state = self.reference_state();
            let geo = geometry::from_resolved(&of_model::resolve(state, &r.entity)?, Some(state))?;
            return Ok(Located {
                body: None,
                geo,
                pose: Pose::IDENTITY,
                part: None,
                part_to_body: Pose::IDENTITY,
            });
        }
        let first = &r.component[0];
        if let Some(inst) = derived.iter().find(|i| i.key == r.component) {
            // Pattern and mirror instances are positioned by their feature:
            // mates read them as fixed geometry where they are now.
            let part = inst.part().ok_or("the instance's file is missing")?.clone();
            let geo = geometry::from_resolved(
                &of_model::resolve(part.state(), &r.entity)?,
                Some(part.state()),
            )?
            .transformed(&inst.pose);
            return Ok(Located {
                body: None,
                geo,
                pose: Pose::IDENTITY,
                part: Some(part),
                part_to_body: inst.pose,
            });
        }
        let c = self
            .doc
            .component(first)
            .cloned()
            .ok_or_else(|| format!("component {first} no longer exists"))?;
        if self.doc.is_suppressed(&c) {
            return Err(format!("{} is suppressed", c.name));
        }
        let mut body = bodies
            .body(std::slice::from_ref(first))
            .ok_or_else(|| format!("{} is not in the assembly", c.name))?;
        let mut local = Pose::IDENTITY;
        let mut loaded = self.load(&c);
        let mut rest = &r.component[1..];
        let mut flexible = c.flexible;
        let mut name = c.name.clone();
        loop {
            match loaded {
                Loaded::Missing(e) => return Err(format!("{name}: {e}")),
                Loaded::Part(p) => {
                    if !rest.is_empty() {
                        return Err(format!("{name} is a part, not an assembly"));
                    }
                    let state = p.state();
                    let geo =
                        geometry::from_resolved(&of_model::resolve(state, &r.entity)?, Some(state))
                            .map_err(|e| format!("{name}: {e}"))?;
                    return Ok(Located {
                        body: Some(body),
                        geo: geo.transformed(&local),
                        pose: bodies.poses[body],
                        part: Some(p),
                        part_to_body: local,
                    });
                }
                Loaded::Assembly(sub) => {
                    let sub_dir = sub.path.parent().map(Path::to_path_buf).unwrap_or_default();
                    let Some(next) = rest.first() else {
                        // The subassembly's own planes and axes.
                        let mut m = Model::new(sub.doc.reference.clone());
                        m.rebuild();
                        let state = m.state();
                        let geo = geometry::from_resolved(
                            &of_model::resolve(state, &r.entity)?,
                            Some(state),
                        )?;
                        return Ok(Located {
                            body: Some(body),
                            geo: geo.transformed(&local),
                            pose: bodies.poses[body],
                            part: None,
                            part_to_body: local,
                        });
                    };
                    let child = sub
                        .doc
                        .component(next)
                        .cloned()
                        .ok_or_else(|| format!("{name} has no component {next}"))?;
                    if flexible {
                        let key: Vec<String> =
                            r.component[..r.component.len() - rest.len() + 1].to_vec();
                        body = bodies
                            .body(&key)
                            .ok_or_else(|| format!("{} is suppressed", child.name))?;
                        local = Pose::IDENTITY;
                    } else {
                        local = local.then(&child.transform.into());
                    }
                    flexible = false;
                    name = child.name.clone();
                    let configuration = sub.doc.part_configuration(&child);
                    loaded = Assembly::load_in(&mut self.cache, &sub_dir, &child, configuration);
                    rest = &rest[1..];
                }
            }
        }
    }

    /// Build every mate's terms. Mates that can't be built get an error.
    fn build(
        &mut self,
        bodies: &Bodies,
        derived: &[Instance],
        errors: &mut BTreeMap<u32, String>,
    ) -> Built {
        let mut built = Built {
            mates: Vec::new(),
            origin: Vec::new(),
            captured: Vec::new(),
        };
        let mut all: Vec<(Vec<String>, Mate)> = self
            .doc
            .mates
            .iter()
            .filter(|m| !self.doc.is_mate_suppressed(m))
            .map(|m| (Vec::new(), m.clone()))
            .collect();
        all.extend(bodies.sub_mates.iter().cloned());
        for (prefix, mate) in all {
            let kind = if prefix.is_empty() {
                mate.kind.clone()
            } else {
                prefixed(&mate.kind, &prefix)
            };
            let mut ctx = Ctx {
                asm: self,
                bodies,
                derived,
            };
            match mates::build(&kind, Some(&mate.captured), &mut ctx) {
                Ok((terms, captured)) => {
                    let mut terms = terms;
                    let mut captured = captured;
                    mates::settle_couplers(&mut terms, &mut captured, &bodies.poses);
                    built.mates.push(of_mate::Mate {
                        id: built.origin.len() as u32,
                        terms,
                    });
                    built.origin.push((prefix, mate.id));
                    built.captured.push(captured);
                }
                Err(e) => {
                    if prefix.is_empty() {
                        errors.insert(mate.id, e);
                    }
                }
            }
        }
        if !bodies.locks.is_empty() {
            built.mates.push(of_mate::Mate {
                id: built.origin.len() as u32,
                terms: bodies.locks.clone(),
            });
            built.origin.push((vec!["fixed".into()], 0));
            built.captured.push(Default::default());
        }
        built
    }

    /// Solve all mates, write poses back and update diagnostics. Drags pull
    /// components (top-level keys, or flexible children) toward targets.
    pub fn solve(&mut self, drags: &[(Vec<String>, DragTarget)], analyse: bool) -> &Diagnostics {
        let mut bodies = self.bodies();
        let mut errors = BTreeMap::new();
        let mut component_errors = BTreeMap::new();
        for c in self.doc.components.clone() {
            if self.doc.is_suppressed(&c) {
                continue;
            }
            match self.load(&c) {
                Loaded::Missing(e) => {
                    component_errors.insert(c.id.to_string(), e);
                }
                Loaded::Part(p) => {
                    if let Some(e) = &p.error {
                        component_errors.insert(c.id.to_string(), e.clone());
                    }
                }
                Loaded::Assembly(_) => {}
            }
        }
        // Mates to pattern instances read them where they are, so solve,
        // place the instances, and solve again if any mate uses them.
        let mut derived: Vec<Instance> = Vec::new();
        let uses_derived = self.doc.mates.iter().any(|m| mate_uses_derived(&m.kind));
        let rounds = if uses_derived { 2 } else { 1 };
        let mut report = of_mate::Report::default();
        let mut built = Built {
            mates: Vec::new(),
            origin: Vec::new(),
            captured: Vec::new(),
        };
        for round in 0..rounds {
            if uses_derived {
                let base = self.instances_without_derived();
                derived = self
                    .derived(&base)
                    .into_iter()
                    .flat_map(|d| d.instances)
                    .collect();
            }
            errors.clear();
            built = self.build(&bodies, &derived, &mut errors);
            let solver_drags: Vec<of_mate::Drag> = drags
                .iter()
                .filter_map(|(key, t)| {
                    let body = bodies.body(key)?;
                    Some(match t {
                        DragTarget::Pose(p) => of_mate::Drag::Pose { body, pose: *p },
                        DragTarget::Point { local, target } => of_mate::Drag::Point {
                            body,
                            local: *local,
                            target: *target,
                        },
                    })
                })
                .collect();
            let system = System {
                poses: bodies.poses.clone(),
                fixed: bodies.fixed.clone(),
                mates: built.mates.clone(),
            };
            let (poses, r) = of_mate::solve(&system, &solver_drags, analyse && round + 1 == rounds);
            bodies.poses = poses;
            report = r;
            self.write_back(&bodies);
        }
        // Store captured values and coupler continuity on top-level mates.
        for (i, (prefix, id)) in built.origin.iter().enumerate() {
            if !prefix.is_empty() {
                continue;
            }
            if let Some(m) = self.doc.mates.iter_mut().find(|m| m.id == *id) {
                let mut captured = built.captured[i].clone();
                if let Some(values) = report.couplers.get(&(i as u32)) {
                    captured.motions = values.clone();
                }
                m.captured = captured;
            }
        }
        let top = |i: u32| -> Option<u32> {
            built
                .origin
                .get(i as usize)
                .filter(|(p, _)| p.is_empty())
                .map(|(_, id)| *id)
        };
        for i in &report.failed {
            if let Some(id) = top(*i) {
                errors
                    .entry(id)
                    .or_insert_with(|| "this mate can't be satisfied".into());
            }
        }
        let conflicting: Vec<u32> = report.conflicting.iter().filter_map(|i| top(*i)).collect();
        for id in &conflicting {
            errors.insert(*id, "over defined: conflicts with other mates".into());
        }
        self.diagnostics = Diagnostics {
            solved: report.solved && errors.is_empty(),
            dof: if analyse {
                bodies
                    .keys
                    .iter()
                    .zip(&report.dof)
                    .map(|(k, d)| (k.join("/"), *d))
                    .collect()
            } else {
                self.diagnostics.dof.clone()
            },
            free: if analyse {
                report.free
            } else {
                self.diagnostics.free
            },
            mate_errors: errors,
            conflicting,
            redundant: report.redundant.iter().filter_map(|i| top(*i)).collect(),
            component_errors,
        };
        &self.diagnostics
    }

    fn instances_without_derived(&mut self) -> Vec<Instance> {
        let mut out = Vec::new();
        let dir = self.base_dir();
        let doc = self.doc.clone();
        let mut stack = Vec::new();
        expand(
            &mut self.cache,
            &doc,
            &dir,
            &[],
            Pose::IDENTITY,
            &mut stack,
            &mut out,
            true,
        );
        out
    }

    /// Copy solved poses into the document.
    fn write_back(&mut self, bodies: &Bodies) {
        for (key, pose) in bodies.keys.iter().zip(&bodies.poses) {
            match key.as_slice() {
                [id] => {
                    if let Some(c) = self.doc.component_mut(id) {
                        c.transform = (*pose).into();
                    }
                }
                [id, child] => {
                    let parent = bodies
                        .body(std::slice::from_ref(id))
                        .map(|i| bodies.poses[i]);
                    if let (Some(parent), Some(c)) = (parent, self.doc.component_mut(id)) {
                        c.flexible_poses
                            .insert(child.clone(), parent.inverse().then(pose).into());
                    }
                }
                _ => {}
            }
        }
    }

    /// Make a mate from current positions. Fails, leaving the document
    /// unchanged, if it can't be built or can't be satisfied with the others.
    pub fn add_mate(&mut self, name: Option<String>, kind: MateKind) -> Result<u32, String> {
        let saved = self.doc.clone();
        let bodies = self.bodies();
        let derived = if mate_uses_derived(&kind) {
            let base = self.instances_without_derived();
            self.derived(&base)
                .into_iter()
                .flat_map(|d| d.instances)
                .collect()
        } else {
            Vec::new()
        };
        let mut ctx = Ctx {
            asm: self,
            bodies: &bodies,
            derived: &derived,
        };
        let (mut terms, mut captured) = mates::build(&kind, None, &mut ctx)?;
        mates::settle_couplers(&mut terms, &mut captured, &bodies.poses);
        let id = self.doc.take_id();
        let name = name.unwrap_or_else(|| self.next_mate_name(&kind));
        self.doc.mates.push(Mate {
            id,
            name,
            kind,
            suppressed: false,
            captured,
        });
        self.solve(&[], true);
        if self.diagnostics.mate_errors.contains_key(&id)
            || self.diagnostics.conflicting.contains(&id)
        {
            let reason = self.conflict_message(id);
            self.doc = saved;
            self.solve(&[], true);
            return Err(reason);
        }
        Ok(id)
    }

    /// Replace a mate's definition, recapturing its values.
    pub fn update_mate(&mut self, id: u32, kind: MateKind) -> Result<(), String> {
        let saved = self.doc.clone();
        let index = self
            .doc
            .mates
            .iter()
            .position(|m| m.id == id)
            .ok_or_else(|| format!("no mate {id}"))?;
        let bodies = self.bodies();
        let derived = if mate_uses_derived(&kind) {
            let base = self.instances_without_derived();
            self.derived(&base)
                .into_iter()
                .flat_map(|d| d.instances)
                .collect()
        } else {
            Vec::new()
        };
        let mut ctx = Ctx {
            asm: self,
            bodies: &bodies,
            derived: &derived,
        };
        let (mut terms, mut captured) = mates::build(&kind, None, &mut ctx)?;
        mates::settle_couplers(&mut terms, &mut captured, &bodies.poses);
        self.doc.mates[index].kind = kind;
        self.doc.mates[index].captured = captured;
        self.solve(&[], true);
        if self.diagnostics.mate_errors.contains_key(&id) {
            let reason = self.conflict_message(id);
            self.doc = saved;
            self.solve(&[], true);
            return Err(reason);
        }
        Ok(())
    }

    fn conflict_message(&self, id: u32) -> String {
        let names: Vec<String> = self
            .diagnostics
            .conflicting
            .iter()
            .filter(|m| **m != id)
            .filter_map(|m| self.doc.mate(*m).map(|m| m.name.clone()))
            .collect();
        let base = self
            .diagnostics
            .mate_errors
            .get(&id)
            .cloned()
            .unwrap_or_else(|| "the mate can't be satisfied".into());
        if names.is_empty() {
            base
        } else {
            format!("{base}; it conflicts with {}", names.join(", "))
        }
    }

    pub fn next_mate_name(&self, kind: &MateKind) -> String {
        let base = mate_base_name(kind);
        let mut n = 1;
        while self
            .doc
            .mates
            .iter()
            .any(|m| m.name == format!("{base}{n}"))
        {
            n += 1;
        }
        format!("{base}{n}")
    }

    /// The world pose of a component path (subassembly children included).
    pub fn world_pose(&mut self, key: &[String]) -> Option<Pose> {
        self.instances()
            .into_iter()
            .find(|i| i.key == key)
            .map(|i| i.pose)
    }

    /// Resolve a reference to world geometry, for measuring and directions.
    pub fn world_geometry(&mut self, r: &CompRef) -> Result<Geo, String> {
        let bodies = self.bodies();
        let derived: Vec<Instance> = if r.component.first().is_some_and(|k| k.contains('@')) {
            let base = self.instances_without_derived();
            self.derived(&base)
                .into_iter()
                .flat_map(|d| d.instances)
                .collect()
        } else {
            Vec::new()
        };
        let l = self.locate(r, &bodies, &derived)?;
        Ok(l.world())
    }

    /// Insert a component. The first component is fixed at the origin.
    pub fn insert(
        &mut self,
        file: &Path,
        transform: Option<Pose>,
        configuration: Option<u32>,
    ) -> Result<String, String> {
        if !file.exists() {
            return Err(format!("{} does not exist", file.display()));
        }
        let ext = file
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ext != "ofp" && ext != "ofa" {
            return Err("insert an OpenForge part (.ofp) or assembly (.ofa)".into());
        }
        if let Some(own) = &self.path
            && canonical(own) == canonical(file)
        {
            return Err("an assembly can't contain itself".into());
        }
        if ext == "ofa" {
            check_cycle(&mut self.cache, file, self.path.as_deref())?;
        }
        let first = self.doc.components.is_empty();
        let path = match &self.path {
            Some(_) => relative_to(file, &self.base_dir()),
            None => canonical(file).to_string_lossy().replace('\\', "/"),
        };
        let id = self.doc.take_id();
        let name = self.doc.next_name(&path);
        let hash = self.cache.hash(&canonical(file)).ok();
        let c = Component {
            id,
            name,
            path,
            hash,
            configuration,
            transform: transform.unwrap_or(Pose::IDENTITY).into(),
            fixed: first,
            suppressed: false,
            hidden: false,
            exclude_from_bom: false,
            flexible: false,
            flexible_poses: BTreeMap::new(),
        };
        if let Loaded::Missing(e) = self.load(&c) {
            self.doc.next_id -= 1;
            return Err(e);
        }
        self.doc.components.push(c);
        Ok(id.to_string())
    }

    /// Delete components and every mate, pattern seed and mirror item that
    /// uses them. Returns deleted mate IDs.
    pub fn delete_components(&mut self, keys: &[String]) -> Vec<u32> {
        self.doc
            .components
            .retain(|c| !keys.contains(&c.id.to_string()));
        let uses = |r: &CompRef| {
            r.component.first().is_some_and(|k| {
                keys.iter()
                    .any(|d| k == d || k.starts_with(&format!("{d}@")))
            })
        };
        let mut deleted = Vec::new();
        self.doc.mates.retain(|m| {
            let gone = mate_refs(&m.kind).iter().any(|r| uses(r))
                || matches!(&m.kind, MateKind::Lock { a, b } if a.first().is_some_and(|k| keys.contains(k)) || b.first().is_some_and(|k| keys.contains(k)));
            if gone {
                deleted.push(m.id);
            }
            !gone
        });
        for p in &mut self.doc.patterns {
            p.seeds.retain(|s| !keys.contains(s));
        }
        self.doc.patterns.retain(|p| !p.seeds.is_empty());
        for m in &mut self.doc.mirrors {
            m.components.retain(|i| !keys.contains(&i.component));
        }
        self.doc.mirrors.retain(|m| !m.components.is_empty());
        for c in &mut self.doc.configurations {
            c.suppressed.retain(|k, _| !keys.contains(k));
            c.part_configurations.retain(|k, _| !keys.contains(k));
            for s in &mut c.explode {
                s.components
                    .retain(|p| p.first().is_none_or(|k| !keys.contains(k)));
            }
            c.explode.retain(|s| !s.components.is_empty());
        }
        deleted
    }

    /// Reload files whose contents changed. Returns the components reloaded.
    pub fn refresh(&mut self) -> Vec<String> {
        let mut changed = Vec::new();
        for c in self.doc.components.clone() {
            let file = self.file_of(&c);
            let hash = self.cache.hash(&canonical(&file)).ok();
            if hash != c.hash {
                changed.push(c.id.to_string());
                if let Some(cm) = self.doc.component_mut(&c.id.to_string()) {
                    cm.hash = hash;
                }
            }
        }
        changed
    }
}

/// A component path resolved against a directory.
pub fn resolve_file(dir: &Path, path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        dir.join(p)
    }
}

/// Fail if inserting `file` would make an assembly contain itself.
fn check_cycle(cache: &mut Cache, file: &Path, own: Option<&Path>) -> Result<(), String> {
    let Some(own) = own else { return Ok(()) };
    let own = canonical(own);
    let mut stack = vec![canonical(file)];
    let mut seen = std::collections::HashSet::new();
    while let Some(f) = stack.pop() {
        if f == own {
            return Err("that assembly contains this one; an assembly can't contain itself".into());
        }
        if !seen.insert(f.clone()) {
            continue;
        }
        if let Ok(sub) = cache.assembly(&f) {
            let dir = f.parent().map(Path::to_path_buf).unwrap_or_default();
            for c in &sub.doc.components {
                let p = resolve_file(&dir, &c.path);
                if crate::file::is_assembly(&p) {
                    stack.push(canonical(&p));
                }
            }
        }
    }
    Ok(())
}

/// Flatten a document's components into `out`.
#[allow(clippy::too_many_arguments)]
fn expand(
    cache: &mut Cache,
    doc: &AssemblyDocument,
    dir: &Path,
    prefix: &[String],
    parent: Pose,
    stack: &mut Vec<PathBuf>,
    out: &mut Vec<Instance>,
    top: bool,
) {
    for c in doc.components.iter().filter(|c| !doc.is_suppressed(c)) {
        let mut key = prefix.to_vec();
        key.push(c.id.to_string());
        let configuration = doc.part_configuration(c);
        let file = resolve_file(dir, &c.path);
        let loaded = if stack.contains(&canonical(&file)) {
            Loaded::Missing("this assembly contains itself".into())
        } else {
            Assembly::load_in(cache, dir, c, configuration)
        };
        let pose = parent.then(&c.transform.into());
        out.push(Instance {
            key: key.clone(),
            name: c.name.clone(),
            pose,
            loaded: loaded.clone(),
            file: file.clone(),
            configuration,
            hidden: c.hidden,
            fixed: c.fixed,
            exclude_from_bom: c.exclude_from_bom,
            derived: None,
        });
        if let Loaded::Assembly(sub) = &loaded {
            let sub_dir = sub.path.parent().map(Path::to_path_buf).unwrap_or_default();
            stack.push(canonical(&file));
            let mut sub_doc = sub.doc.clone();
            if top && c.flexible {
                for child in &mut sub_doc.components {
                    if let Some(t) = c.flexible_poses.get(&child.id.to_string()) {
                        child.transform = *t;
                    }
                }
            }
            let start = out.len();
            expand(cache, &sub_doc, &sub_dir, &key, pose, stack, out, false);
            if c.hidden {
                for i in &mut out[start..] {
                    i.hidden = true;
                }
            }
            stack.pop();
        }
    }
}

/// Mates of a flexible subassembly, with paths prefixed by its key.
fn prefixed(kind: &MateKind, prefix: &[String]) -> MateKind {
    let mut v = serde_json::to_value(kind).expect("mates serialize");
    fn walk(v: &mut serde_json::Value, prefix: &[String]) {
        match v {
            serde_json::Value::Object(o) => {
                let is_ref = o.contains_key("entity");
                if is_ref && let Some(serde_json::Value::Array(c)) = o.get_mut("component") {
                    let mut p: Vec<serde_json::Value> = prefix
                        .iter()
                        .map(|s| serde_json::Value::String(s.clone()))
                        .collect();
                    p.append(c);
                    *c = p;
                    return;
                }
                for (k, x) in o.iter_mut() {
                    if (k == "a" || k == "b")
                        && let serde_json::Value::Array(path) = x
                        && path.iter().all(|e| e.is_string())
                    {
                        let mut p: Vec<serde_json::Value> = prefix
                            .iter()
                            .map(|s| serde_json::Value::String(s.clone()))
                            .collect();
                        p.append(path);
                        *path = p;
                        continue;
                    }
                    walk(x, prefix);
                }
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(|x| walk(x, prefix)),
            _ => {}
        }
    }
    walk(&mut v, prefix);
    serde_json::from_value(v).expect("prefixing keeps mates valid")
}

/// Every component reference a mate uses.
pub fn mate_refs(kind: &MateKind) -> Vec<&CompRef> {
    use MateKind as K;
    match kind {
        K::Coincident { a, b, .. }
        | K::Parallel { a, b, .. }
        | K::Perpendicular { a, b }
        | K::Tangent { a, b, .. }
        | K::Concentric { a, b, .. }
        | K::Distance { a, b, .. }
        | K::Angle { a, b, .. }
        | K::ProfileCenter { a, b, .. }
        | K::LinearCoupler { a, b, .. }
        | K::Gear { a, b, .. }
        | K::Screw { a, b, .. } => vec![a, b],
        K::Lock { .. } => vec![],
        K::Symmetric { a, b, plane } => vec![a, b, plane],
        K::Width { width, tab, .. } => width.iter().chain(tab.iter()).collect(),
        K::Path {
            point,
            path,
            follow,
            roll,
            ..
        } => {
            let mut v: Vec<&CompRef> = vec![point];
            v.extend(path.iter());
            v.extend(follow.iter());
            if let Some(r) = roll {
                v.push(&r.axis);
                v.push(&r.up);
            }
            v
        }
        K::Cam { cam, follower } => vec![cam, follower],
        K::Slot { slot, pin, .. } => vec![slot, pin],
        K::Hinge { axis, faces, limit } => {
            let mut v: Vec<&CompRef> = axis.iter().chain(faces.iter()).collect();
            if let Some(l) = limit {
                v.extend(l.faces.iter());
            }
            v
        }
        K::RackPinion { rack, pinion, .. } => vec![rack, pinion],
        K::UniversalJoint { a, b, joint_point } => {
            let mut v = vec![a, b];
            v.extend(joint_point.iter());
            v
        }
    }
}

/// Top-level component keys a mate involves.
pub fn mate_components(kind: &MateKind) -> Vec<String> {
    let mut out: Vec<String> = match kind {
        MateKind::Lock { a, b } => a.iter().take(1).chain(b.iter().take(1)).cloned().collect(),
        _ => mate_refs(kind)
            .iter()
            .filter_map(|r| r.component.first().cloned())
            .collect(),
    };
    out.sort();
    out.dedup();
    out
}

fn mate_uses_derived(kind: &MateKind) -> bool {
    mate_refs(kind)
        .iter()
        .any(|r| r.component.first().is_some_and(|k| k.contains('@')))
}

pub fn mate_base_name(kind: &MateKind) -> &'static str {
    use MateKind as K;
    match kind {
        K::Coincident { .. } => "Coincident",
        K::Parallel { .. } => "Parallel",
        K::Perpendicular { .. } => "Perpendicular",
        K::Tangent { .. } => "Tangent",
        K::Concentric { .. } => "Concentric",
        K::Lock { .. } => "Lock",
        K::Distance { limit: Some(_), .. } => "DistanceLimit",
        K::Distance { .. } => "Distance",
        K::Angle { limit: Some(_), .. } => "AngleLimit",
        K::Angle { .. } => "Angle",
        K::ProfileCenter { .. } => "ProfileCenter",
        K::Symmetric { .. } => "Symmetric",
        K::Width { .. } => "Width",
        K::Path { .. } => "PathMate",
        K::LinearCoupler { .. } => "LinearCoupler",
        K::Cam { .. } => "Cam",
        K::Slot { .. } => "Slot",
        K::Hinge { .. } => "Hinge",
        K::Gear { .. } => "Gear",
        K::RackPinion { .. } => "RackPinion",
        K::Screw { .. } => "Screw",
        K::UniversalJoint { .. } => "UniversalJoint",
    }
}

struct Ctx<'a> {
    asm: &'a mut Assembly,
    bodies: &'a Bodies,
    derived: &'a [Instance],
}

impl Context for Ctx<'_> {
    fn locate(&mut self, r: &CompRef) -> Result<Located, String> {
        self.asm.locate(r, self.bodies, self.derived)
    }

    fn body(&mut self, path: &[String]) -> Result<(of_mate::Body, Pose), String> {
        let first = path.first().ok_or("select a component")?;
        // Locking into a rigid subassembly locks the subassembly.
        let key = if path.len() >= 2 && self.bodies.body(&path[..2]).is_some() {
            path[..2].to_vec()
        } else {
            vec![first.clone()]
        };
        let b = self
            .bodies
            .body(&key)
            .ok_or_else(|| format!("component {} is not in the assembly", key.join("/")))?;
        Ok((Some(b), self.bodies.poses[b]))
    }
}

//! A drawing being edited: the document, its loaded models, and the sheet
//! display lists rebuilt from them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use of_model::geom::{DVec2, HlrEdge, HlrQuality, Shape};
use schemars::JsonSchema;
use serde::Serialize;

use crate::clip::Gap;
use crate::document::*;
use crate::formats::{self, Fields, Format};
use crate::graphics::{Graphics, P2};
use crate::model::Sources;
use crate::viewdraw::gaps;
use crate::views::{self, Frame};

/// How a view looks at its model, after following its parent chain.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub frame: Frame,
    /// Paper mm per model mm.
    pub scale: f64,
    /// Sheet position of the model origin.
    pub origin: DVec2,
    pub model: ModelSource,
    pub exploded: bool,
    pub display: DisplayStyle,
    pub tangent: TangentEdges,
    /// Break view bands (view-plane mm), highest first.
    pub breaks: Vec<Gap>,
}

impl Resolved {
    /// Sheet position of a point in view-plane coordinates (model mm),
    /// with break bands closed up.
    pub fn to_sheet(&self, p: DVec2) -> P2 {
        let mut q = [p.x, p.y];
        for g in &self.breaks {
            q = g.shift_clamped(q);
        }
        self.place([q[0], q[1]])
    }

    /// Sheet position of a view-plane point already moved for breaks.
    pub fn place(&self, p: P2) -> P2 {
        let q = self.origin + DVec2::from(p) * self.scale;
        [q.x, q.y]
    }

    pub fn from_sheet(&self, p: P2) -> DVec2 {
        (DVec2::from(p) - self.origin) / self.scale
    }
}

/// What the desktop needs to know about a drawn view.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct ViewInfo {
    pub id: u32,
    /// Sheet-space box around the view's geometry; `None` when it has none.
    pub bbox: Option<[P2; 2]>,
    /// Paper mm per model mm.
    pub scale: f64,
    /// Sheet position of the model origin after alignment with the parent
    /// (annotations that move with the view are placed relative to it).
    pub origin: P2,
    /// Paper axes and view direction in model space.
    pub x: [f64; 3],
    pub y: [f64; 3],
    pub toward: [f64; 3],
    /// Why the view could not be drawn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The model changed on disk since the drawing last recorded it.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stale: bool,
}

/// A rendered sheet.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct SheetRender {
    pub graphics: Graphics,
    pub views: Vec<ViewInfo>,
    /// The format's inner border and title block.
    pub frame: [P2; 2],
    pub title_block: [P2; 2],
    /// Annotations whose model anchor no longer resolves; they are drawn at
    /// their last sheet position.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dangling: Vec<Dangling>,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct Dangling {
    pub annotation: u32,
    pub reason: String,
}

pub struct Drawing {
    pub doc: DrawingDocument,
    path: Option<PathBuf>,
    pub sources: Sources,
    pub(crate) hlr: HashMap<String, Rc<Vec<HlrEdge>>>,
    /// Number of hidden-line computations run, for cache tests.
    pub hlr_runs: usize,
    /// Sheet boxes of the views drawn by the last render.
    pub(crate) boxes: HashMap<u32, [P2; 2]>,
}

impl Default for Drawing {
    fn default() -> Self {
        Drawing::new(DrawingDocument::default())
    }
}

impl Drawing {
    pub fn new(doc: DrawingDocument) -> Drawing {
        Drawing {
            doc,
            path: None,
            sources: Sources::default(),
            hlr: HashMap::new(),
            hlr_runs: 0,
            boxes: HashMap::new(),
        }
    }

    pub fn open(path: &Path) -> Result<Drawing, String> {
        let doc = crate::file::load(path)?;
        let mut d = Drawing::new(doc);
        d.path = Some(path.to_path_buf());
        Ok(d)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Where relative model paths are resolved from.
    pub fn base_dir(&self) -> PathBuf {
        self.path
            .as_ref()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
    }

    /// Save, rewriting model paths relative to the new location.
    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        let new_dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let old_dir = self.base_dir();
        let mut doc = self.doc.clone();
        for s in &mut doc.sheets {
            for v in &mut s.views {
                if let ViewKind::Model { model, .. } = &mut v.kind {
                    let abs = of_assembly::resolve_file(&old_dir, &model.path);
                    model.path = of_assembly::relative_to(&abs, &new_dir);
                }
            }
        }
        doc.validate()?;
        crate::file::save(&doc, path)?;
        self.doc = doc;
        self.path = Some(path.to_path_buf());
        Ok(())
    }

    /// Absolute path of a model source.
    pub fn model_path(&self, m: &ModelSource) -> PathBuf {
        of_assembly::resolve_file(&self.base_dir(), &m.path)
    }

    /// Follow a view's parent chain to its frame, scale and model.
    pub fn resolve(&mut self, sheet: &Sheet, id: u32) -> Result<Resolved, String> {
        self.resolve_depth(sheet, id, 0)
    }

    fn resolve_depth(&mut self, sheet: &Sheet, id: u32, depth: usize) -> Result<Resolved, String> {
        if depth > sheet.views.len() {
            return Err("views form a cycle".into());
        }
        let v = sheet.view(id).ok_or_else(|| format!("no view {id}"))?;
        let own = DVec2::from(v.origin);
        let mut r = match &v.kind {
            ViewKind::Model {
                model,
                orientation,
                exploded,
            } => Resolved {
                frame: match orientation {
                    _ if model.flat.is_some() => self.flat_frame(model, orientation)?,
                    Orientation::Relative { front, top } => {
                        self.relative_frame(model, *exploded, front, top)?
                    }
                    o => views::orientation(o),
                },
                scale: v.scale.unwrap_or(sheet.scale).factor(),
                origin: own,
                model: model.clone(),
                exploded: *exploded,
                display: v.display.unwrap_or_default(),
                tangent: v.tangent.unwrap_or_default(),
                breaks: Vec::new(),
            },
            ViewKind::Projected { parent, placement } => {
                let p = self.resolve_depth(sheet, *parent, depth + 1)?;
                let d = views::placement_dir(*placement);
                let origin = if !v.aligned {
                    own
                } else if d.y == 0.0 {
                    DVec2::new(own.x, p.origin.y)
                } else if d.x == 0.0 {
                    DVec2::new(p.origin.x, own.y)
                } else {
                    own
                };
                Resolved {
                    frame: views::projected(&p.frame, *placement, sheet.projection),
                    scale: v.scale.map(|s| s.factor()).unwrap_or(p.scale),
                    origin,
                    display: v.display.unwrap_or(p.display),
                    tangent: v.tangent.unwrap_or(p.tangent),
                    ..p
                }
            }
            ViewKind::Auxiliary { parent, edge, flip } => {
                let p = self.resolve_depth(sheet, *parent, depth + 1)?;
                let along = self.edge_direction(&p, edge)?;
                let mut side = along.perp();
                // Fold toward where the view sits; `flip` folds the other way.
                if (own - p.origin).dot(side) < 0.0 {
                    side = -side;
                }
                if *flip {
                    side = -side;
                }
                let origin = if v.aligned {
                    let s = side.normalize();
                    p.origin + s * (own - p.origin).dot(s)
                } else {
                    own
                };
                Resolved {
                    frame: views::folded(&p.frame, side, sheet.projection),
                    scale: v.scale.map(|s| s.factor()).unwrap_or(p.scale),
                    origin,
                    display: v.display.unwrap_or(p.display),
                    tangent: v.tangent.unwrap_or(p.tangent),
                    ..p
                }
            }
            ViewKind::Section {
                parent, line, flip, ..
            } => {
                let p = self.resolve_depth(sheet, *parent, depth + 1)?;
                let (a, b) = match line.points.as_slice() {
                    [a, b, ..] => (DVec2::from(*a), DVec2::from(*b)),
                    _ => return Err("a cutting line needs two points".into()),
                };
                let along = (b - a)
                    .try_normalize()
                    .ok_or("the cutting line has no length")?;
                let mut side = along.perp();
                if (own - p.origin).dot(side) < 0.0 {
                    side = -side;
                }
                if *flip {
                    side = -side;
                }
                let origin = if v.aligned {
                    p.origin + side * (own - p.origin).dot(side)
                } else {
                    own
                };
                Resolved {
                    frame: views::folded(&p.frame, side, sheet.projection),
                    scale: v.scale.map(|s| s.factor()).unwrap_or(p.scale),
                    origin,
                    display: v.display.unwrap_or(p.display),
                    tangent: v.tangent.unwrap_or(p.tangent),
                    ..p
                }
            }
            ViewKind::Detail { parent, .. } => {
                let p = self.resolve_depth(sheet, *parent, depth + 1)?;
                Resolved {
                    scale: v.scale.map(|s| s.factor()).unwrap_or(p.scale * 2.0),
                    origin: own,
                    display: v.display.unwrap_or(p.display),
                    tangent: v.tangent.unwrap_or(p.tangent),
                    ..p
                }
            }
        };
        r.breaks = gaps(v, r.scale);
        Ok(r)
    }

    /// Frame of a flat pattern view: the flat's front faces the viewer (its
    /// back for `Back`), the flat's own y axis up.
    fn flat_frame(&mut self, model: &ModelSource, o: &Orientation) -> Result<Frame, String> {
        let path = self.model_path(model);
        let part = self.sources.part(&path, model.configuration, true)?;
        let plane = part
            .state()
            .sheet_metal
            .iter()
            .find_map(|r| r.flat_plane)
            .ok_or("the part has no flat pattern")?;
        let toward = if matches!(o, Orientation::Back) {
            -plane.normal
        } else {
            plane.normal
        };
        Ok(Frame::looking(toward, plane.y_dir()))
    }

    /// Frame of a relative-to-model view: `front` faces the viewer, `top`
    /// faces up.
    fn relative_frame(
        &mut self,
        model: &ModelSource,
        exploded: bool,
        front: &of_assembly::CompRef,
        top: &of_assembly::CompRef,
    ) -> Result<Frame, String> {
        let path = self.model_path(model);
        let mut normal = |r: &of_assembly::CompRef| -> Result<of_model::geom::DVec3, String> {
            let (part, pose) = self.sources.locate(
                &path,
                model.configuration,
                model.flat.is_some(),
                exploded,
                r,
            )?;
            let state = part.state();
            let (kind, i) = state.index_of(&r.entity).map_err(|e| e.to_string())?;
            if kind != of_model::geom::Kind::Face {
                return Err("a relative view needs two faces".into());
            }
            let body = state
                .body
                .as_ref()
                .ok_or("the part has no solid geometry")?;
            let info = body.shape.face_info(i).map_err(|e| e.to_string())?;
            if info.kind != of_model::geom::SurfaceKind::Plane {
                return Err("a relative view needs planar faces".into());
            }
            Ok(pose.rotation * info.normal)
        };
        let t = normal(front)?;
        let u = normal(top)?;
        if t.cross(u).length() < 1e-6 {
            return Err("the two faces of a relative view are parallel".into());
        }
        Ok(Frame::looking(t, u))
    }

    /// Paper direction of a straight model edge seen in a view.
    fn edge_direction(
        &mut self,
        view: &Resolved,
        edge: &of_assembly::CompRef,
    ) -> Result<DVec2, String> {
        let path = self.model_path(&view.model);
        let (part, pose) = self
            .sources
            .locate(
                &path,
                view.model.configuration,
                view.model.flat.is_some(),
                view.exploded,
                edge,
            )
            .map_err(|e| format!("the auxiliary view's edge: {e}"))?;
        let state = part.state();
        let (_, i) = state.index_of(&edge.entity).map_err(|e| e.to_string())?;
        let body = state
            .body
            .as_ref()
            .ok_or("the part has no solid geometry")?;
        let info = body.shape.edge_info(i).map_err(|e| e.to_string())?;
        let dir = pose.rotation * (info.end - info.start);
        view.frame
            .project(dir)
            .try_normalize()
            .ok_or_else(|| "the edge is seen end-on in the parent view".to_string())
    }

    /// Hidden-line edges of a shape seen in a frame, cached by geometry,
    /// direction and accuracy.
    pub(crate) fn projection(
        &mut self,
        key: &str,
        shape: &Shape,
        frame: &Frame,
        hidden: bool,
        scale: f64,
    ) -> Result<Rc<Vec<HlrEdge>>, String> {
        // 0.01 mm on paper.
        let deflection = (0.01 / scale).max(1e-4);
        let key = format!(
            "{key}|{:.12?}|{:.12?}|{hidden}|{deflection:.6}",
            frame.x.to_array(),
            frame.toward.to_array()
        );
        if let Some(e) = self.hlr.get(&key) {
            return Ok(e.clone());
        }
        self.hlr_runs += 1;
        let edges = shape
            .hidden_lines(&frame.plane(), HlrQuality::Exact, hidden, deflection)
            .map_err(|e| e.to_string())?;
        let edges = Rc::new(edges);
        if self.hlr.len() > 256 {
            self.hlr.clear();
        }
        self.hlr.insert(key, edges.clone());
        Ok(edges)
    }

    /// The format drawn on a sheet.
    pub fn format(&self, sheet: &Sheet) -> Result<Format, String> {
        if let Some(f) = formats::builtin(&sheet.format.template, sheet) {
            return Ok(f);
        }
        let path = of_assembly::resolve_file(&self.base_dir(), &sheet.format.template);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("could not open sheet format {}: {e}", path.display()))?;
        #[derive(serde::Deserialize)]
        struct File {
            format: String,
            sheet_format: Format,
        }
        let f: File = serde_json::from_str(&text)
            .map_err(|e| format!("not a valid sheet format file: {e}"))?;
        if f.format != formats::FORMAT_FILE {
            return Err("not an OpenForge sheet format file".into());
        }
        Ok(f.sheet_format)
    }

    pub(crate) fn fields(&mut self, sheet: &Sheet) -> Fields {
        let index = self
            .doc
            .sheets
            .iter()
            .position(|s| s.id == sheet.id)
            .unwrap_or(0);
        let mut f = Fields {
            drawing: self.doc.properties.clone(),
            sheet: index + 1,
            sheets: self.doc.sheets.len(),
            scale: sheet.scale.label(),
            size: sheet.size.name(),
            file_name: self
                .path
                .as_ref()
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string(),
            ..Default::default()
        };
        // A revision table's latest revision feeds the title block.
        let current = self
            .doc
            .sheets
            .iter()
            .flat_map(|s| &s.annotations)
            .find_map(|a| match &a.kind {
                crate::annotations::AnnotationKind::Table(t) => match &t.kind {
                    crate::tables::TableKind::Revision { rows, numeric } => {
                        crate::tables::revision_cells(rows, *numeric).1
                    }
                    _ => None,
                },
                _ => None,
            });
        if let Some(rev) = current {
            f.drawing.insert("Revision".into(), rev);
        }
        let view = sheet
            .properties_view
            .and_then(|id| sheet.view(id))
            .or(sheet.views.first());
        if let Some(v) = view
            && let Ok(r) = self.resolve(sheet, v.id)
        {
            let path = self.model_path(&r.model);
            if let Ok(p) = self.sources.properties(&path, r.model.configuration) {
                f.model = p;
            }
            if !crate::model::is_assembly(&path)
                && let Ok(part) =
                    self.sources
                        .part(&path, r.model.configuration, r.model.flat.is_some())
            {
                f.material = of_model::properties::evaluate(part.doc(), part.state(), "{material}");
                if let Ok(m) = of_model::measure::mass_properties(part.doc(), part.state()) {
                    f.mass = format!("{:.3} kg", m.mass);
                }
            }
        }
        f
    }

    /// Whether a view's model changed since the drawing recorded its hash.
    fn is_stale(&mut self, r: &Resolved) -> bool {
        let Some(saved) = r.model.hash.clone() else {
            return false;
        };
        let path = self.model_path(&r.model);
        self.sources.hash(&path).is_ok_and(|h| h != saved)
    }

    /// Record the current hash of every model shown, so views stop being
    /// stale. Returns whether anything changed.
    pub fn record_hashes(&mut self) -> bool {
        let base = self.base_dir();
        let mut changed = false;
        for si in 0..self.doc.sheets.len() {
            for vi in 0..self.doc.sheets[si].views.len() {
                let path = match &self.doc.sheets[si].views[vi].kind {
                    ViewKind::Model { model, .. } => of_assembly::resolve_file(&base, &model.path),
                    _ => continue,
                };
                let Ok(h) = self.sources.hash(&path) else {
                    continue;
                };
                if let ViewKind::Model { model, .. } = &mut self.doc.sheets[si].views[vi].kind
                    && model.hash.as_deref() != Some(&h)
                {
                    model.hash = Some(h);
                    changed = true;
                }
            }
        }
        changed
    }

    /// Draw a sheet.
    pub fn render(&mut self, sheet_id: u32) -> Result<SheetRender, String> {
        let sheet = self
            .doc
            .sheet(sheet_id)
            .cloned()
            .ok_or_else(|| format!("no sheet {sheet_id}"))?;
        let (w, h) = sheet.paper();
        let mut g = Graphics::new(w, h);
        let format = self.format(&sheet)?;
        if sheet.format.visible {
            let fields = self.fields(&sheet);
            formats::draw(
                &format,
                &mut g,
                &fields,
                sheet.projection,
                &self.doc.standard,
            );
        }
        let mut infos = Vec::new();
        self.boxes.clear();
        for v in &sheet.views {
            let result = self.draw_view(&sheet, v, &mut g);
            let resolved = self.resolve(&sheet, v.id).ok();
            let bbox = result.as_ref().ok().copied().flatten();
            if let Some(b) = bbox {
                self.boxes.insert(v.id, b);
            }
            let stale = resolved.as_ref().is_some_and(|r| self.is_stale(r));
            infos.push(ViewInfo {
                id: v.id,
                bbox,
                scale: resolved.as_ref().map_or(1.0, |r| r.scale),
                origin: resolved
                    .as_ref()
                    .map_or(v.origin, |r| [r.origin.x, r.origin.y]),
                x: resolved
                    .as_ref()
                    .map_or([1.0, 0.0, 0.0], |r| r.frame.x.to_array()),
                y: resolved
                    .as_ref()
                    .map_or([0.0, 1.0, 0.0], |r| r.frame.y.to_array()),
                toward: resolved
                    .as_ref()
                    .map_or([0.0, 0.0, 1.0], |r| r.frame.toward.to_array()),
                error: result.err(),
                stale,
            });
        }
        for v in &sheet.views {
            if v.parent().is_some_and(|p| self.boxes.contains_key(&p)) {
                self.draw_marks(&sheet, v, &mut g);
            }
        }
        let mut dangling = Vec::new();
        for a in &sheet.annotations {
            if let Err(reason) = self.draw_annotation(&sheet, a, &mut g) {
                dangling.push(Dangling {
                    annotation: a.id,
                    reason,
                });
            }
        }
        Ok(SheetRender {
            graphics: g,
            views: infos,
            frame: format.frame,
            title_block: format.title_block,
            dangling,
        })
    }

    /// Sheet position of an anchor.
    pub fn anchor_point(&mut self, sheet: &Sheet, a: &Anchor) -> Option<P2> {
        match a {
            Anchor::Sheet { at } => Some(*at),
            Anchor::Model { target, at } => {
                let r = self.resolve(sheet, target.view).ok()?;
                Some(r.to_sheet(DVec2::from(*at)))
            }
        }
    }
}

pub(crate) fn bounds(items: &[crate::graphics::Item]) -> Option<[P2; 2]> {
    let g = Graphics {
        width: 0.0,
        height: 0.0,
        items: items.to_vec(),
    };
    g.extents().map(|(a, b)| [a, b])
}

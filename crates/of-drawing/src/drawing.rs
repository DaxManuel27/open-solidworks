//! A drawing being edited: the document, its loaded models, and the sheet
//! display lists rebuilt from them.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use of_model::geom::{Curve2d, DVec2, EdgeClass, HlrEdge, HlrQuality};
use schemars::JsonSchema;
use serde::Serialize;

use crate::annotations::AnnotationKind;
use crate::document::*;
use crate::formats::{self, Fields, Format};
use crate::graphics::{Graphics, HAlign, LineStyle, Owner, P2, Pen, Prim, layers};
use crate::model::{Scene, Sources};
use crate::text;
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
}

impl Resolved {
    /// Sheet position of a point in view-plane coordinates (model mm).
    pub fn to_sheet(&self, p: DVec2) -> P2 {
        let q = self.origin + p * self.scale;
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
    /// Paper axes and view direction in model space.
    pub x: [f64; 3],
    pub y: [f64; 3],
    pub toward: [f64; 3],
    /// Why the view could not be drawn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A rendered sheet.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct SheetRender {
    pub graphics: Graphics,
    pub views: Vec<ViewInfo>,
    /// The format's inner border and title block.
    pub frame: [P2; 2],
    pub title_block: [P2; 2],
}

pub struct Drawing {
    pub doc: DrawingDocument,
    path: Option<PathBuf>,
    pub sources: Sources,
    hlr: HashMap<String, Rc<Vec<HlrEdge>>>,
    /// Number of hidden-line computations run, for cache tests.
    pub hlr_runs: usize,
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
    pub fn resolve(&self, sheet: &Sheet, id: u32) -> Result<Resolved, String> {
        self.resolve_depth(sheet, id, 0)
    }

    fn resolve_depth(&self, sheet: &Sheet, id: u32, depth: usize) -> Result<Resolved, String> {
        if depth > sheet.views.len() {
            return Err("views form a cycle".into());
        }
        let v = sheet.view(id).ok_or_else(|| format!("no view {id}"))?;
        let own = DVec2::from(v.origin);
        Ok(match &v.kind {
            ViewKind::Model {
                model,
                orientation,
                exploded,
            } => Resolved {
                frame: views::orientation(orientation),
                scale: v.scale.unwrap_or(sheet.scale).factor(),
                origin: own,
                model: model.clone(),
                exploded: *exploded,
                display: v.display.unwrap_or_default(),
                tangent: v.tangent.unwrap_or_default(),
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
        })
    }

    /// Paper direction of a straight model edge seen in a view.
    fn edge_direction(
        &self,
        view: &Resolved,
        edge: &of_assembly::CompRef,
    ) -> Result<DVec2, String> {
        // Needs the model loaded; resolving is read-only on the cache.
        let path = self.model_path(&view.model);
        let mut sources = Sources::default();
        let scene = sources.scene(&path, view.model.configuration, view.exploded, &[])?;
        let (state, pose) = scene
            .locate(edge)
            .ok_or("the auxiliary view's edge is not in the model")?;
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

    /// Hidden-line edges of a scene seen in a frame, cached by geometry,
    /// direction and accuracy.
    fn projection(
        &mut self,
        scene: &Scene,
        frame: &Frame,
        hidden: bool,
        scale: f64,
    ) -> Result<Rc<Vec<HlrEdge>>, String> {
        // 0.01 mm on paper.
        let deflection = (0.01 / scale).max(1e-4);
        let key = format!(
            "{}|{:.12?}|{:.12?}|{hidden}|{deflection:.6}",
            scene.key,
            frame.x.to_array(),
            frame.toward.to_array()
        );
        if let Some(e) = self.hlr.get(&key) {
            return Ok(e.clone());
        }
        self.hlr_runs += 1;
        let edges = scene
            .shape
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

    fn fields(&mut self, sheet: &Sheet) -> Fields {
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
                && let Ok(part) = self.sources.part(&path, r.model.configuration)
            {
                f.material = of_model::properties::evaluate(part.doc(), part.state(), "{material}");
                if let Ok(m) = of_model::measure::mass_properties(part.doc(), part.state()) {
                    f.mass = format!("{:.3} kg", m.mass);
                }
            }
        }
        f
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
        for v in &sheet.views {
            let start = g.items.len();
            let result = self.draw_view(&sheet, v, &mut g);
            let resolved = self.resolve(&sheet, v.id).ok();
            let bbox = bounds(&g.items[start..]);
            infos.push(ViewInfo {
                id: v.id,
                bbox,
                scale: resolved.as_ref().map_or(1.0, |r| r.scale),
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
            });
        }
        for a in &sheet.annotations {
            self.draw_annotation(&sheet, a, &mut g);
        }
        Ok(SheetRender {
            graphics: g,
            views: infos,
            frame: format.frame,
            title_block: format.title_block,
        })
    }

    fn draw_view(&mut self, sheet: &Sheet, v: &View, g: &mut Graphics) -> Result<(), String> {
        let r = self.resolve(sheet, v.id)?;
        match &v.kind {
            ViewKind::Section { .. } => return Err("section views are not drawn yet".into()),
            ViewKind::Detail { .. } => return Err("detail views are not drawn yet".into()),
            _ => {}
        }
        let path = self.model_path(&r.model);
        let scene = self.sources.scene(
            &path,
            r.model.configuration,
            r.exploded,
            &v.hidden_components,
        )?;
        let show_hidden = matches!(
            r.display,
            DisplayStyle::HiddenVisible | DisplayStyle::Wireframe
        );
        let edges = self.projection(&scene, &r.frame, show_hidden, r.scale)?;
        let hidden_edges: HashSet<u32> = v
            .hidden_edges
            .iter()
            .filter_map(|e| scene.edge_index(e))
            .collect();
        let std = &self.doc.standard;
        let thick = Pen::new(LineStyle::Continuous, std.thick_line());
        let thin = Pen::new(LineStyle::Continuous, std.thin_line());
        for e in edges.iter() {
            if e.source.is_some_and(|s| hidden_edges.contains(&s)) {
                continue;
            }
            let (pen, layer) = match (e.visible, e.class) {
                (_, EdgeClass::Seam) => continue,
                (true, EdgeClass::Smooth) | (false, EdgeClass::Smooth)
                    if r.tangent == TangentEdges::Removed =>
                {
                    continue;
                }
                (true, EdgeClass::Smooth) => match r.tangent {
                    TangentEdges::Font => (
                        Pen::new(LineStyle::Phantom, std.thin_line()),
                        layers::TANGENT,
                    ),
                    _ => (thin, layers::TANGENT),
                },
                (true, _) => (thick, layers::VISIBLE),
                (false, _) if r.display == DisplayStyle::Wireframe => (thick, layers::VISIBLE),
                (false, _) => (Pen::new(LineStyle::Hidden, std.thin_line()), layers::HIDDEN),
            };
            let owner = Owner::View {
                view: v.id,
                entity: e.source.and_then(|s| scene.edge_ref(s)),
            };
            g.push(to_prim(&e.curve, &r), pen, layer, &owner);
        }
        if r.exploded {
            let pen = Pen::new(LineStyle::Phantom, std.thin_line());
            let owner = Owner::View {
                view: v.id,
                entity: None,
            };
            for [a, b] in &scene.explode_lines {
                let (a, b) = (r.frame.project(*a), r.frame.project(*b));
                if a.distance(b) * r.scale > 0.5 {
                    g.line(r.to_sheet(a), r.to_sheet(b), pen, "EXPLODE", &owner);
                }
            }
        }
        Ok(())
    }

    fn draw_annotation(
        &mut self,
        sheet: &Sheet,
        a: &crate::annotations::Annotation,
        g: &mut Graphics,
    ) {
        let std = self.doc.standard;
        let owner = Owner::Annotation { id: a.id };
        let thin = Pen::new(LineStyle::Continuous, std.thin_line());
        match &a.kind {
            AnnotationKind::Note {
                at,
                text: t,
                height,
                leader,
            } => {
                let fields = self.fields(sheet);
                let value = formats::fill(t, &fields);
                let h = height.unwrap_or(std.text_height());
                for p in text::layout(&value, *at, h, 0.0, HAlign::Left) {
                    g.push(p, thin, layers::NOTES, &owner);
                }
                if let Some(target) = leader.as_ref().and_then(|l| self.anchor_point(sheet, l)) {
                    let (w, bh) = text::block_size(&value, h);
                    // Leave from the nearer end of the first line.
                    let start = if target[0] < at[0] + w / 2.0 {
                        [at[0] - 1.0, at[1] + h / 2.0]
                    } else {
                        [at[0] + w + 1.0, at[1] + h / 2.0]
                    };
                    let _ = bh;
                    crate::annotations::leader(g, start, target, std.arrow_length(), thin, &owner);
                }
            }
        }
    }

    /// Sheet position of an anchor.
    pub fn anchor_point(&self, sheet: &Sheet, a: &Anchor) -> Option<P2> {
        match a {
            Anchor::Sheet { at } => Some(*at),
            Anchor::Model { target, at } => {
                let r = self.resolve(sheet, target.view).ok()?;
                Some(r.to_sheet(DVec2::from(*at)))
            }
        }
    }
}

/// A projected curve placed on the sheet.
fn to_prim(c: &Curve2d, r: &Resolved) -> Prim {
    match c {
        Curve2d::Line(a, b) => Prim::Line {
            a: r.to_sheet(*a),
            b: r.to_sheet(*b),
        },
        Curve2d::Arc {
            center,
            radius,
            start,
            end,
        } => Prim::Arc {
            center: r.to_sheet(*center),
            radius: radius * r.scale,
            start: *start,
            end: *end,
        },
        Curve2d::Polyline(p) => Prim::Polyline {
            points: p.iter().map(|q| r.to_sheet(*q)).collect(),
            closed: false,
        },
    }
}

fn bounds(items: &[crate::graphics::Item]) -> Option<[P2; 2]> {
    let g = Graphics {
        width: 0.0,
        height: 0.0,
        items: items.to_vec(),
    };
    g.extents().map(|(a, b)| [a, b])
}

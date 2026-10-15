//! An open drawing and the `drawing.*` methods clients call on it.

use std::collections::HashSet;
use std::path::Path;

use of_drawing::annotations::{Annotation, AnnotationKind};
use of_drawing::drawing::Drawing;
use of_drawing::standard::{Standard, StandardBase};
use of_drawing::{
    DrawingDocument, ModelSource, Projection, Scale, Sheet, SheetFormat, SheetSize, View, ViewKind,
    dxf, formats, pdf,
};
use of_protocol::{ErrorCode, RpcError};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::tools::drawing_params::*;

const UNDO_LIMIT: usize = 100;

type R = Result<Value, RpcError>;

fn args<T: DeserializeOwned>(params: Value) -> Result<T, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    serde_json::from_value(params).map_err(|e| RpcError::params(e.to_string()))
}

fn fail(e: impl std::fmt::Display) -> RpcError {
    RpcError::failed(e.to_string())
}

fn unknown(what: &str, id: impl std::fmt::Display) -> RpcError {
    RpcError::coded(ErrorCode::UnknownRef, format!("no {what} {id}"))
}

pub struct DrawSession {
    pub drawing: Drawing,
    dirty: bool,
    undo: Vec<DrawingDocument>,
    redo: Vec<DrawingDocument>,
    revision: u64,
}

/// A drawing with one empty sheet.
fn blank(p: &NewDrawing) -> DrawingDocument {
    let standard = match p.standard.unwrap_or_default() {
        StandardBase::Iso => Standard::iso(),
        StandardBase::Ansi => Standard::ansi(),
    };
    let ansi = standard.base == StandardBase::Ansi;
    let mut doc = DrawingDocument {
        standard,
        ..Default::default()
    };
    let id = doc.alloc_id();
    doc.sheets.push(Sheet {
        id,
        name: "Sheet1".into(),
        size: p.size.unwrap_or(if ansi {
            SheetSize::AnsiB
        } else {
            SheetSize::A3
        }),
        portrait: false,
        scale: Scale::ONE,
        projection: p.projection.unwrap_or(if ansi {
            Projection::Third
        } else {
            Projection::First
        }),
        format: SheetFormat {
            template: formats::template_for(&standard).into(),
            visible: true,
        },
        properties_view: None,
        views: Vec::new(),
        annotations: Vec::new(),
    });
    doc
}

impl DrawSession {
    pub fn new(p: &NewDrawing) -> Result<DrawSession, RpcError> {
        let mut s = DrawSession::from_drawing(Drawing::new(blank(p)));
        if let Some(model) = &p.model {
            let source = model_source(
                &ModelSource {
                    path: model.to_string_lossy().into_owned(),
                    hash: None,
                    configuration: p.configuration,
                },
                &s.drawing.base_dir(),
                false,
            )?;
            let sheet = s.drawing.doc.sheets[0].id;
            s.drawing
                .add_standard_views(sheet, source, true, None)
                .map_err(fail)?;
            s.drawing.record_hashes();
            s.dirty = true;
        }
        Ok(s)
    }

    fn from_drawing(drawing: Drawing) -> DrawSession {
        DrawSession {
            drawing,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
        }
    }

    pub fn open(path: &Path) -> Result<DrawSession, String> {
        Ok(DrawSession::from_drawing(Drawing::open(path)?))
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn doc(&self) -> &DrawingDocument {
        &self.drawing.doc
    }

    /// Run a change that needs the models (layouts); undone like any other.
    fn change_drawing(
        &mut self,
        f: impl FnOnce(&mut Drawing) -> Result<(), RpcError>,
    ) -> Result<(), RpcError> {
        let before = self.drawing.doc.clone();
        if let Err(e) =
            f(&mut self.drawing).and_then(|()| self.drawing.doc.validate().map_err(fail))
        {
            self.drawing.doc = before;
            return Err(e);
        }
        if self.drawing.doc != before {
            self.undo.push(before);
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
            self.redo.clear();
            self.dirty = true;
        }
        Ok(())
    }

    /// Run a change on a copy of the document; keep it only if it succeeds
    /// and the result is valid.
    fn change(
        &mut self,
        f: impl FnOnce(&mut DrawingDocument, &Path) -> Result<(), RpcError>,
    ) -> Result<(), RpcError> {
        let mut doc = self.drawing.doc.clone();
        f(&mut doc, &self.drawing.base_dir())?;
        doc.validate().map_err(fail)?;
        if doc == self.drawing.doc {
            return Ok(());
        }
        self.undo
            .push(std::mem::replace(&mut self.drawing.doc, doc));
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
        Ok(())
    }

    pub fn handle(&mut self, method: &str, params: Value) -> R {
        let before = self.drawing.doc.clone();
        let result = self.handle_inner(method, params);
        if self.drawing.doc != before || matches!(method, "doc.undo" | "doc.redo") {
            self.revision += 1;
        }
        result.map(|mut v| {
            if let Value::Object(o) = &mut v {
                o.insert("revision".into(), json!(self.revision));
            }
            v
        })
    }

    fn handle_inner(&mut self, method: &str, params: Value) -> R {
        match method {
            "doc.info" | "drawing.info" => Ok(self.full()),
            "doc.save" => {
                let p: crate::tools::params::SavePath = args(params)?;
                let path = p
                    .path
                    .or_else(|| self.drawing.path().map(Path::to_path_buf))
                    .ok_or_else(|| RpcError::params("choose where to save"))?;
                self.drawing.record_hashes();
                self.drawing.save(&path).map_err(fail)?;
                self.dirty = false;
                Ok(self.full())
            }
            "doc.undo" | "doc.redo" => {
                let (from, to) = if method == "doc.undo" {
                    (&mut self.undo, &mut self.redo)
                } else {
                    (&mut self.redo, &mut self.undo)
                };
                if let Some(doc) = from.pop() {
                    to.push(std::mem::replace(&mut self.drawing.doc, doc));
                    self.dirty = true;
                }
                Ok(self.full())
            }
            "export" => {
                let p: crate::tools::params::Export = args(params)?;
                if p.configuration.is_some() {
                    return Err(RpcError::params(
                        "a drawing shows the configurations its views name",
                    ));
                }
                self.export(&p.path)?;
                Ok(json!({ "path": p.path }))
            }
            "drawing.render" => {
                let p: RenderSheet = args(params)?;
                let id = match p.sheet {
                    Some(id) => id,
                    None => self.drawing.doc.sheets[0].id,
                };
                if self.drawing.doc.sheet(id).is_none() {
                    return Err(unknown("sheet", id));
                }
                let r = self.drawing.render(id).map_err(fail)?;
                if p.summary {
                    let mut v =
                        json!({ "sheet": id, "views": r.views, "items": r.graphics.items.len() });
                    if !r.dangling.is_empty() {
                        v["dangling"] = json!(r.dangling);
                    }
                    Ok(v)
                } else {
                    let mut v = json!(r);
                    v["sheet"] = json!(id);
                    Ok(v)
                }
            }
            "drawing.model.faces" => {
                let p: ModelFaces = args(params)?;
                let source = model_source(
                    &ModelSource {
                        path: p.model.to_string_lossy().into_owned(),
                        hash: None,
                        configuration: p.configuration,
                    },
                    &self.drawing.base_dir(),
                    false,
                )?;
                let path = self.drawing.model_path(&source);
                let faces = self
                    .drawing
                    .sources
                    .planar_faces(&path, p.configuration)
                    .map_err(fail)?;
                Ok(json!({ "faces": faces }))
            }
            "drawing.update" => {
                // Views always show the models as saved; this clears their
                // "needs update" marks.
                self.change_drawing(|d| {
                    d.record_hashes();
                    Ok(())
                })?;
                Ok(self.full())
            }
            "drawing.views.standard" => {
                let p: StandardViews = args(params)?;
                let saved = self.drawing.path().is_some();
                let mut added = None;
                self.change_drawing(|d| {
                    let source = model_source(
                        &ModelSource {
                            path: p.model.to_string_lossy().into_owned(),
                            hash: None,
                            configuration: p.configuration,
                        },
                        &d.base_dir(),
                        saved,
                    )?;
                    if d.doc.sheet(p.sheet).is_none() {
                        return Err(unknown("sheet", p.sheet));
                    }
                    added = Some(
                        d.add_standard_views(p.sheet, source, p.isometric, p.scale)
                            .map_err(fail)?,
                    );
                    Ok(())
                })?;
                let a = added.expect("set on success");
                let mut v = self.full();
                v["views"] = json!({
                    "front": a.front,
                    "top": a.top,
                    "side": a.side,
                    "isometric": a.isometric,
                });
                v["scale"] = json!(a.scale);
                Ok(v)
            }
            "drawing.sheet.add" => {
                let p: AddSheet = args(params)?;
                let mut id = 0;
                self.change(|doc, _| {
                    let last = doc.sheets.last().cloned().expect("a drawing has a sheet");
                    id = doc.alloc_id();
                    let n = doc.sheets.len() + 1;
                    doc.sheets.push(Sheet {
                        id,
                        name: p.name.unwrap_or_else(|| format!("Sheet{n}")),
                        size: p.size.unwrap_or(last.size),
                        portrait: p.portrait.unwrap_or(last.portrait),
                        scale: p.scale.unwrap_or(last.scale),
                        projection: p.projection.unwrap_or(last.projection),
                        format: SheetFormat {
                            template: p.template.unwrap_or(last.format.template),
                            visible: true,
                        },
                        properties_view: None,
                        views: Vec::new(),
                        annotations: Vec::new(),
                    });
                    Ok(())
                })?;
                let mut v = self.full();
                v["id"] = json!(id);
                Ok(v)
            }
            "drawing.sheet.update" => {
                let p: UpdateSheet = args(params)?;
                self.change(|doc, _| {
                    let at = doc
                        .sheets
                        .iter()
                        .position(|s| s.id == p.id)
                        .ok_or_else(|| unknown("sheet", p.id))?;
                    let s = &mut doc.sheets[at];
                    if let Some(n) = p.name {
                        if n.trim().is_empty() {
                            return Err(RpcError::params("the name cannot be empty"));
                        }
                        s.name = n.trim().to_string();
                    }
                    if let Some(v) = p.size {
                        s.size = v;
                    }
                    if let Some(v) = p.portrait {
                        s.portrait = v;
                    }
                    if let Some(v) = p.scale {
                        s.scale = v;
                    }
                    if let Some(v) = p.projection {
                        s.projection = v;
                    }
                    if let Some(v) = p.template {
                        s.format.template = v;
                    }
                    if let Some(v) = p.format_visible {
                        s.format.visible = v;
                    }
                    if let Some(v) = p.properties_view {
                        if v.is_some_and(|id| s.view(id).is_none()) {
                            return Err(unknown("view on this sheet", v.unwrap()));
                        }
                        s.properties_view = v;
                    }
                    if let Some(i) = p.index {
                        let sheet = doc.sheets.remove(at);
                        doc.sheets.insert(i.min(doc.sheets.len()), sheet);
                    }
                    Ok(())
                })?;
                Ok(self.full())
            }
            "drawing.sheet.delete" => {
                let p: crate::tools::asm_params::ItemIds = args(params)?;
                self.change(|doc, _| {
                    for id in &p.ids {
                        if doc.sheet(*id).is_none() {
                            return Err(unknown("sheet", id));
                        }
                    }
                    doc.sheets.retain(|s| !p.ids.contains(&s.id));
                    if doc.sheets.is_empty() {
                        return Err(RpcError::coded(
                            ErrorCode::NotAllowed,
                            "a drawing keeps at least one sheet",
                        ));
                    }
                    Ok(())
                })?;
                Ok(self.full())
            }
            "drawing.view.add" => {
                let p: AddView = args(params)?;
                let saved = self.drawing.path().is_some();
                let mut id = 0;
                self.change(|doc, base| {
                    let mut kind = p.view;
                    if let ViewKind::Model { model, .. } = &mut kind {
                        *model = model_source(model, base, saved)?;
                    }
                    id = doc.alloc_id();
                    let name = p.name.unwrap_or_else(|| default_name(&kind, id));
                    let labelled = matches!(
                        kind,
                        ViewKind::Section { .. }
                            | ViewKind::Detail { .. }
                            | ViewKind::Auxiliary { .. }
                    );
                    let label = labelled.then(|| of_drawing::viewdraw::next_label(doc));
                    let sheet = doc
                        .sheet_mut(p.sheet)
                        .ok_or_else(|| unknown("sheet", p.sheet))?;
                    let mut v = View::new(id, name, kind, p.origin);
                    v.label = label;
                    v.scale = p.scale;
                    v.display = p.display;
                    v.tangent = p.tangent;
                    sheet.views.push(v);
                    Ok(())
                })?;
                let mut v = self.full();
                v["id"] = json!(id);
                Ok(v)
            }
            "drawing.view.update" => {
                let p: UpdateView = args(params)?;
                let saved = self.drawing.path().is_some();
                self.change(|doc, base| {
                    let sheet = doc
                        .sheets
                        .iter_mut()
                        .find(|s| s.view(p.id).is_some())
                        .ok_or_else(|| unknown("view", p.id))?;
                    let v = sheet.view_mut(p.id).unwrap();
                    if let Some(n) = p.name {
                        if n.trim().is_empty() {
                            return Err(RpcError::params("the name cannot be empty"));
                        }
                        v.name = n.trim().to_string();
                    }
                    if let Some(mut kind) = p.view {
                        if let ViewKind::Model { model, .. } = &mut kind {
                            *model = model_source(model, base, saved)?;
                        }
                        v.kind = kind;
                    }
                    if let Some(o) = p.origin {
                        v.origin = o;
                    }
                    if let Some(s) = p.scale {
                        v.scale = s;
                    }
                    if let Some(d) = p.display {
                        v.display = d;
                    }
                    if let Some(t) = p.tangent {
                        v.tangent = t;
                    }
                    if let Some(l) = p.label {
                        v.label = l;
                    }
                    if let Some(a) = p.aligned {
                        v.aligned = a;
                    }
                    if let Some(e) = p.hidden_edges {
                        v.hidden_edges = e;
                    }
                    if let Some(c) = p.hidden_components {
                        v.hidden_components = c;
                    }
                    if let Some(b) = p.breaks {
                        for g in &b {
                            if !(g.from.is_finite() && g.to.is_finite()) || g.to <= g.from {
                                return Err(RpcError::params("a break needs `from` below `to`"));
                            }
                        }
                        v.breaks = b;
                    }
                    if let Some(st) = p.break_style {
                        v.break_style = st;
                    }
                    if let Some(gap) = p.break_gap {
                        v.break_gap = gap;
                    }
                    if let Some(c) = p.crop {
                        v.crop = c;
                    }
                    if let Some(b) = p.broken_out {
                        if b.iter().any(|x| !(x.depth.is_finite() && x.depth > 0.0)) {
                            return Err(RpcError::params(
                                "a broken-out section needs a depth above zero",
                            ));
                        }
                        v.broken_out = b;
                    }
                    if let Some(h) = p.hatch {
                        if !(h.scale.is_finite() && h.scale > 0.0) {
                            return Err(RpcError::params("the hatch scale must be above zero"));
                        }
                        v.hatch = h;
                    }
                    Ok(())
                })?;
                Ok(self.full())
            }
            "drawing.view.delete" => {
                let p: crate::tools::asm_params::ItemIds = args(params)?;
                let mut deleted = Vec::new();
                self.change(|doc, _| {
                    for id in &p.ids {
                        if doc.sheet_of_view(*id).is_none() {
                            return Err(unknown("view", id));
                        }
                    }
                    deleted = delete_views(doc, &p.ids);
                    Ok(())
                })?;
                let mut v = self.full();
                v["deleted"] = json!(deleted);
                Ok(v)
            }
            "drawing.annotation.add" => {
                let p: AddAnnotation = args(params)?;
                let mut id = 0;
                self.change(|doc, _| {
                    id = doc.alloc_id();
                    let sheet = doc
                        .sheet_mut(p.sheet)
                        .ok_or_else(|| unknown("sheet", p.sheet))?;
                    let a = Annotation {
                        id,
                        attach: p.view,
                        layer: p.layer,
                        kind: p.annotation,
                    };
                    check_anchors(sheet, &a)?;
                    sheet.annotations.push(a);
                    Ok(())
                })?;
                let mut v = self.full();
                v["id"] = json!(id);
                Ok(v)
            }
            "drawing.annotation.update" => {
                let p: UpdateAnnotation = args(params)?;
                // Moving between a view and the sheet keeps the drawn place.
                let shift = match p.view {
                    Some(to) => {
                        let (sheet, a) = self
                            .find_annotation(p.id)
                            .ok_or_else(|| unknown("annotation", p.id))?;
                        let from = self.view_origin(&sheet, a.attach)?;
                        let to = self.view_origin(&sheet, to)?;
                        Some([from[0] - to[0], from[1] - to[1]])
                    }
                    None => None,
                };
                self.change(|doc, _| {
                    let sheet = doc
                        .sheets
                        .iter_mut()
                        .find(|s| s.annotations.iter().any(|a| a.id == p.id))
                        .ok_or_else(|| unknown("annotation", p.id))?;
                    let i = sheet.annotations.iter().position(|a| a.id == p.id).unwrap();
                    let mut a = sheet.annotations[i].clone();
                    if let Some(k) = p.annotation {
                        a.kind = k;
                    }
                    if let (Some(v), Some(d)) = (p.view, shift) {
                        a.attach = v;
                        a.kind.translate(d);
                    }
                    if let Some(l) = p.layer {
                        a.layer = l;
                    }
                    check_anchors(sheet, &a)?;
                    sheet.annotations[i] = a;
                    Ok(())
                })?;
                Ok(self.full())
            }
            "drawing.annotation.move" => {
                let p: MoveAnnotations = args(params)?;
                self.change(|doc, _| {
                    for id in &p.ids {
                        let a = doc
                            .sheets
                            .iter_mut()
                            .flat_map(|s| s.annotations.iter_mut())
                            .find(|a| a.id == *id)
                            .ok_or_else(|| unknown("annotation", id))?;
                        a.kind.translate(p.by);
                    }
                    Ok(())
                })?;
                Ok(self.full())
            }
            "drawing.model_items" => {
                let p: ModelItemsParams = args(params)?;
                let mut added = Vec::new();
                self.change_drawing(|d| {
                    let sheet = d
                        .doc
                        .sheet(p.sheet)
                        .ok_or_else(|| unknown("sheet", p.sheet))?;
                    let views: Vec<u32> = match &p.views {
                        Some(v) => {
                            for id in v {
                                if sheet.view(*id).is_none() {
                                    return Err(unknown("view on this sheet", id));
                                }
                            }
                            v.clone()
                        }
                        None => sheet.views.iter().map(|v| v.id).collect(),
                    };
                    let new = d.model_items(p.sheet, &views, &p.items).map_err(fail)?;
                    d.doc.next_id = d
                        .doc
                        .next_id
                        .max(new.iter().map(|a| a.id + 1).max().unwrap_or(0));
                    added = new.iter().map(|a| a.id).collect();
                    d.doc.sheet_mut(p.sheet).unwrap().annotations.extend(new);
                    Ok(())
                })?;
                let mut v = self.full();
                v["added"] = json!(added);
                Ok(v)
            }
            "drawing.dimension.drive" => {
                let p: DriveDimension = args(params)?;
                if !p.value.is_finite() || p.value <= 0.0 {
                    return Err(RpcError::params("a dimension value must be above zero"));
                }
                let path = self
                    .drawing
                    .set_model_dimension(p.id, p.value)
                    .map_err(fail)?;
                // The drawing made this change, so its views are current.
                self.change_drawing(|d| {
                    d.record_hashes();
                    Ok(())
                })?;
                let mut v = self.full();
                v["saved_models"] = json!([path]);
                Ok(v)
            }
            "drawing.balloons.auto" => {
                let p: AutoBalloons = args(params)?;
                let mut added = Vec::new();
                self.change_drawing(|d| {
                    let sheet = d
                        .doc
                        .sheet(p.sheet)
                        .ok_or_else(|| unknown("sheet", p.sheet))?;
                    if sheet.view(p.view).is_none() {
                        return Err(unknown("view on this sheet", p.view));
                    }
                    let new = d
                        .auto_balloons(p.sheet, p.view, p.layout, p.ignore_multiple)
                        .map_err(fail)?;
                    if new.is_empty() {
                        return Err(fail("the view shows no assembly components to balloon"));
                    }
                    d.doc.next_id = d
                        .doc
                        .next_id
                        .max(new.iter().map(|a| a.id + 1).max().unwrap_or(0));
                    added = new.iter().map(|a| a.id).collect();
                    d.doc.sheet_mut(p.sheet).unwrap().annotations.extend(new);
                    Ok(())
                })?;
                let mut v = self.full();
                v["added"] = json!(added);
                Ok(v)
            }
            "drawing.table.csv" => {
                let p: TableCsv = args(params)?;
                let (sheet, a) = self
                    .find_annotation(p.id)
                    .ok_or_else(|| unknown("annotation", p.id))?;
                let AnnotationKind::Table(t) = &a.kind else {
                    return Err(RpcError::params(format!(
                        "annotation {} is not a table",
                        p.id
                    )));
                };
                let csv = self.drawing.table_cells(&sheet, t).map_err(fail)?.csv();
                match p.path {
                    Some(path) => {
                        std::fs::write(&path, &csv).map_err(|e| {
                            fail(format!("could not write {}: {e}", path.display()))
                        })?;
                        Ok(json!({ "path": path }))
                    }
                    None => Ok(json!({ "csv": csv })),
                }
            }
            "drawing.standard.set" => {
                let p: SetStandard = args(params)?;
                let standard = match (p.standard, &p.load) {
                    (Some(_), Some(_)) => {
                        return Err(RpcError::params("give `standard` or `load`, not both"));
                    }
                    (Some(s), None) => s,
                    (None, Some(path)) => Standard::load(path).map_err(fail)?,
                    (None, None) => self.drawing.doc.standard,
                };
                self.change(|doc, _| {
                    doc.standard = standard;
                    Ok(())
                })?;
                if let Some(path) = &p.save {
                    standard.save(path).map_err(fail)?;
                }
                Ok(self.full())
            }
            "drawing.export" => {
                let p: ExportDrawing = args(params)?;
                let written = self.export_with(&p)?;
                Ok(json!({ "paths": written }))
            }
            "drawing.annotation.delete" => {
                let p: crate::tools::asm_params::ItemIds = args(params)?;
                self.change(|doc, _| {
                    for id in &p.ids {
                        if !doc
                            .sheets
                            .iter()
                            .any(|s| s.annotations.iter().any(|a| a.id == *id))
                        {
                            return Err(unknown("annotation", id));
                        }
                    }
                    for s in &mut doc.sheets {
                        s.annotations.retain(|a| !p.ids.contains(&a.id));
                    }
                    Ok(())
                })?;
                Ok(self.full())
            }
            "drawing.properties.set" => {
                let p: DrawingProperties = args(params)?;
                self.change(|doc, _| {
                    for (k, v) in p.properties {
                        if k.trim().is_empty() {
                            return Err(RpcError::params("a property needs a name"));
                        }
                        match v {
                            Some(v) => doc.properties.insert(k, v),
                            None => doc.properties.remove(&k),
                        };
                    }
                    Ok(())
                })?;
                Ok(self.full())
            }
            _ => Err(RpcError::coded(
                ErrorCode::NotAllowed,
                format!("{method} is not available while a drawing is open"),
            )),
        }
    }

    /// Write every sheet to a PDF or DXF file, chosen by extension.
    fn export(&mut self, path: &Path) -> Result<(), RpcError> {
        self.export_with(&ExportDrawing {
            path: path.to_path_buf(),
            sheets: None,
            monochrome: false,
            per_sheet: false,
            dxf_version: None,
        })
        .map(drop)
    }

    /// Write sheets to PDF or DXF; returns the files written.
    fn export_with(&mut self, p: &ExportDrawing) -> Result<Vec<std::path::PathBuf>, RpcError> {
        let path = &p.path;
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if ext != "pdf" && ext != "dxf" {
            return Err(RpcError::params("a drawing exports to .pdf or .dxf"));
        }
        let ids: Vec<u32> = match &p.sheets {
            Some(ids) => {
                if ids.is_empty() {
                    return Err(RpcError::params("choose at least one sheet"));
                }
                for id in ids {
                    if self.drawing.doc.sheet(*id).is_none() {
                        return Err(unknown("sheet", id));
                    }
                }
                // In sheet order.
                self.drawing
                    .doc
                    .sheets
                    .iter()
                    .map(|s| s.id)
                    .filter(|id| ids.contains(id))
                    .collect()
            }
            None => self.drawing.doc.sheets.iter().map(|s| s.id).collect(),
        };
        let mut sheets = Vec::new();
        for id in &ids {
            let r = self.drawing.render(*id).map_err(fail)?;
            if let Some(v) = r.views.iter().find(|v| v.error.is_some()) {
                return Err(fail(format!(
                    "view {} could not be drawn: {}",
                    v.id,
                    v.error.as_deref().unwrap_or_default()
                )));
            }
            sheets.push(r.graphics);
        }
        let standard = self.drawing.doc.standard;
        let write = |path: &Path, bytes: Vec<u8>| {
            std::fs::write(path, bytes)
                .map_err(|e| fail(format!("could not write {}: {e}", path.display())))
        };
        if ext == "pdf" {
            let title = self
                .drawing
                .path()
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .map(String::from);
            let bytes = pdf::write(
                &sheets,
                &standard,
                &pdf::PdfOptions {
                    title,
                    monochrome: p.monochrome,
                },
            )
            .map_err(fail)?;
            write(path, bytes)?;
            return Ok(vec![path.clone()]);
        }
        let version = p.dxf_version.unwrap_or_default();
        let dxf_sheets: Vec<dxf::DxfSheet> = sheets
            .iter()
            .zip(&ids)
            .map(|(g, id)| {
                let s = self.drawing.doc.sheet(*id).expect("checked above");
                dxf::DxfSheet {
                    graphics: g,
                    name: s.name.clone(),
                    scale: s.scale.factor(),
                    view_annotations: s
                        .annotations
                        .iter()
                        .filter(|a| a.attach.is_some())
                        .map(|a| a.id)
                        .collect(),
                }
            })
            .collect();
        if !p.per_sheet || sheets.len() == 1 {
            write(
                path,
                dxf::write_version(&dxf_sheets, &standard, version)
                    .map_err(fail)?
                    .into_bytes(),
            )?;
            return Ok(vec![path.clone()]);
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("drawing");
        let mut out = Vec::new();
        for d in &dxf_sheets {
            let name = &d.name;
            let safe: String = name
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            let file = path.with_file_name(format!("{stem}-{safe}.dxf"));
            write(
                &file,
                dxf::write_version(std::slice::from_ref(d), &standard, version)
                    .map_err(fail)?
                    .into_bytes(),
            )?;
            out.push(file);
        }
        Ok(out)
    }

    fn find_annotation(&self, id: u32) -> Option<(Sheet, Annotation)> {
        self.drawing.doc.sheets.iter().find_map(|s| {
            s.annotations
                .iter()
                .find(|a| a.id == id)
                .map(|a| (s.clone(), a.clone()))
        })
    }

    /// Sheet position of a view's origin, or the sheet's for `None`.
    fn view_origin(&mut self, sheet: &Sheet, view: Option<u32>) -> Result<[f64; 2], RpcError> {
        match view {
            None => Ok([0.0, 0.0]),
            Some(v) => {
                if sheet.view(v).is_none() {
                    return Err(unknown("view on this sheet", v));
                }
                let r = self.drawing.resolve(sheet, v).map_err(fail)?;
                Ok([r.origin.x, r.origin.y])
            }
        }
    }

    pub fn full(&self) -> Value {
        json!({
            "kind": "drawing",
            "drawing": self.drawing.doc,
            "path": self.drawing.path(),
            "dirty": self.dirty,
            "can_undo": !self.undo.is_empty(),
            "can_redo": !self.redo.is_empty(),
        })
    }
}

/// A model source with its path checked and stored relative to a saved
/// drawing (absolute while unsaved).
fn model_source(m: &ModelSource, base: &Path, saved: bool) -> Result<ModelSource, RpcError> {
    let abs = of_assembly::resolve_file(base, &m.path);
    let ext = abs
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if !matches!(ext.as_deref(), Some("ofp" | "ofa")) {
        return Err(RpcError::params(
            "a view shows an .ofp part or .ofa assembly",
        ));
    }
    if !abs.exists() {
        return Err(fail(format!("{} does not exist", abs.display())));
    }
    let path = if saved {
        of_assembly::relative_to(&abs, base)
    } else {
        of_assembly::cache::canonical(&abs)
            .to_string_lossy()
            .replace('\\', "/")
    };
    Ok(ModelSource {
        path,
        hash: None,
        configuration: m.configuration,
    })
}

fn default_name(kind: &ViewKind, id: u32) -> String {
    match kind {
        ViewKind::Model { .. } => format!("Drawing View{id}"),
        ViewKind::Projected { .. } => format!("Projected View{id}"),
        ViewKind::Auxiliary { .. } => format!("Auxiliary View{id}"),
        ViewKind::Section { .. } => format!("Section View{id}"),
        ViewKind::Detail { .. } => format!("Detail View{id}"),
    }
}

/// Delete views with the views derived from them. Leaders to deleted views
/// are removed; their notes stay. Returns every deleted ID.
fn delete_views(doc: &mut DrawingDocument, ids: &[u32]) -> Vec<u32> {
    let mut gone: HashSet<u32> = ids.iter().copied().collect();
    for s in &mut doc.sheets {
        loop {
            let more: Vec<u32> = s
                .views
                .iter()
                .filter(|v| !gone.contains(&v.id) && v.parent().is_some_and(|p| gone.contains(&p)))
                .map(|v| v.id)
                .collect();
            if more.is_empty() {
                break;
            }
            gone.extend(more);
        }
        s.views.retain(|v| !gone.contains(&v.id));
        if s.properties_view.is_some_and(|v| gone.contains(&v)) {
            s.properties_view = None;
        }
        let gone_list: Vec<u32> = gone.iter().copied().collect();
        for a in &mut s.annotations {
            a.kind.drop_leaders_to(&gone_list);
        }
        // Annotations that move with, or measure, a deleted view go too.
        s.annotations
            .retain(|a| !a.views().iter().any(|v| gone.contains(v)));
    }
    let mut out: Vec<u32> = gone.into_iter().collect();
    out.sort();
    out
}

/// Annotations may refer only to views on their own sheet (a BOM may list
/// an assembly view on any sheet).
fn check_anchors(sheet: &Sheet, a: &Annotation) -> Result<(), RpcError> {
    let bom_view = match &a.kind {
        AnnotationKind::Table(t) => match &t.kind {
            of_drawing::tables::TableKind::Bom { view, .. } => Some(*view),
            _ => None,
        },
        _ => None,
    };
    for v in a.views() {
        if sheet.view(v).is_none() && Some(v) != bom_view {
            return Err(unknown("view on this sheet", v));
        }
    }
    Ok(())
}

impl Default for DrawSession {
    fn default() -> Self {
        DrawSession::new(&NewDrawing {
            standard: None,
            size: None,
            projection: None,
            model: None,
            configuration: None,
        })
        .expect("a blank drawing needs no model")
    }
}

/// Whether a path names a drawing.
pub fn is_drawing(path: &Path) -> bool {
    of_drawing::file::is_drawing(path)
}

//! Standard view layouts: the standard 3 view (front with a top and a side
//! view projected from it) and an optional isometric, scaled to fit the
//! sheet's drawing area and placed around its middle.

use of_model::geom::{DVec2, DVec3};

use crate::document::*;
use crate::drawing::Drawing;
use crate::views::{self, Frame};

/// Preferred drawing scales (ISO 5455), largest first.
pub const SCALES: [(f64, f64); 13] = [
    (10.0, 1.0),
    (5.0, 1.0),
    (2.0, 1.0),
    (1.0, 1.0),
    (1.0, 2.0),
    (1.0, 5.0),
    (1.0, 10.0),
    (1.0, 20.0),
    (1.0, 50.0),
    (1.0, 100.0),
    (1.0, 200.0),
    (1.0, 500.0),
    (1.0, 1000.0),
];

/// Paper extent of a box seen in a frame (unscaled, relative to the model
/// origin): low and high corners.
fn extent(frame: &Frame, lo: DVec3, hi: DVec3) -> (DVec2, DVec2) {
    let mut a = DVec2::splat(f64::MAX);
    let mut b = DVec2::splat(f64::MIN);
    for i in 0..8 {
        let c = DVec3::new(
            if i & 1 == 0 { lo.x } else { hi.x },
            if i & 2 == 0 { lo.y } else { hi.y },
            if i & 4 == 0 { lo.z } else { hi.z },
        );
        let p = frame.project(c);
        a = a.min(p);
        b = b.max(p);
    }
    (a, b)
}

/// IDs of the views a standard layout added.
#[derive(Debug, Clone, PartialEq)]
pub struct Added {
    pub front: u32,
    pub top: u32,
    pub side: u32,
    pub isometric: Option<u32>,
    pub scale: Scale,
}

impl Drawing {
    /// Add a standard 3 view (and an isometric) of a model to a sheet. With
    /// `scale` unset the largest preferred scale that fits is chosen and set
    /// as the sheet scale.
    pub fn add_standard_views(
        &mut self,
        sheet_id: u32,
        model: ModelSource,
        isometric: bool,
        scale: Option<Scale>,
    ) -> Result<Added, String> {
        let sheet = self
            .doc
            .sheet(sheet_id)
            .cloned()
            .ok_or_else(|| format!("no sheet {sheet_id}"))?;
        let path = self.model_path(&model);
        let scene = self.sources.scene(
            &path,
            model.configuration,
            model.flat.is_some(),
            false,
            &[],
            model.bodies.as_deref(),
        )?;
        let bb = scene
            .shape
            .bbox()
            .ok_or("the model has no geometry to draw")?;
        let front = views::orientation(&Orientation::Front);
        let (side_place, top_place) = match sheet.projection {
            Projection::Third => (Placement::Right, Placement::Above),
            Projection::First => (Placement::Right, Placement::Below),
        };
        let side = views::projected(&front, side_place, sheet.projection);
        let top = views::projected(&front, top_place, sheet.projection);
        let iso = views::orientation(&Orientation::Isometric);
        let (f0, f1) = extent(&front, bb.min, bb.max);
        let (s0, s1) = extent(&side, bb.min, bb.max);
        let (t0, t1) = extent(&top, bb.min, bb.max);
        let (i0, i1) = extent(&iso, bb.min, bb.max);
        let (fw, fh) = (f1.x - f0.x, f1.y - f0.y);
        let sw = s1.x - s0.x;
        let th = t1.y - t0.y;
        let (iw, ih) = (i1.x - i0.x, i1.y - i0.y);

        // Usable area: inside the frame, above the title block.
        let format = self.format(&sheet)?;
        let [flo, fhi] = format.frame;
        let tb = format.title_block;
        let area_lo = DVec2::new(flo[0], (tb[1][1]).max(flo[1]) + 5.0);
        let area_hi = DVec2::new(fhi[0], fhi[1]);
        let avail = (area_hi - area_lo) * 0.9;
        // A 2 × 2 grid: front, top above or below it, side to its right
        // and the isometric in the free corner. Gaps are paper mm.
        let gap = 20.0;
        let cells = |k: f64| {
            let left_w = fw * k;
            let right_w = sw.max(if isometric { iw } else { 0.0 }) * k;
            let width = left_w + gap + right_w;
            let front_h = fh * k;
            let other_h = th.max(if isometric { ih } else { 0.0 }) * k;
            (width, front_h + gap + other_h)
        };
        let fits = |k: f64| {
            let (w, h) = cells(k);
            w <= avail.x && h <= avail.y
        };
        let scale = match scale {
            Some(s) => s,
            None => {
                let (num, den) = SCALES
                    .iter()
                    .copied()
                    .find(|(n, d)| fits(n / d))
                    .unwrap_or(SCALES[SCALES.len() - 1]);
                Scale { num, den }
            }
        };
        let k = scale.factor();
        let (width, height) = cells(k);
        let mid = (area_lo + area_hi) / 2.0;
        let left = mid.x - width / 2.0;
        let bottom = mid.y - height / 2.0;
        let other_h = th.max(if isometric { ih } else { 0.0 }) * k;
        // Front view box: bottom-left corner on the sheet.
        let front_y = match top_place {
            Placement::Above => bottom,
            _ => bottom + other_h + gap,
        };
        let front_x = left;
        let front_origin = DVec2::new(front_x, front_y) - f0 * k;
        let right_x = front_x + fw * k + gap;
        let side_origin = DVec2::new(right_x - s0.x * k, front_origin.y);
        let other_y = match top_place {
            Placement::Above => front_y + fh * k + gap,
            _ => bottom,
        };
        let top_origin = DVec2::new(
            front_origin.x,
            other_y + (other_h - th * k) / 2.0 - t0.y * k,
        );
        let iso_origin = DVec2::new(right_x, other_y + (other_h - ih * k) / 2.0) - i0 * k;

        let doc = &mut self.doc;
        let ids = [doc.alloc_id(), doc.alloc_id(), doc.alloc_id()];
        let iso_id = isometric.then(|| doc.alloc_id());
        let sheet = doc.sheet_mut(sheet_id).expect("checked");
        sheet.scale = scale;
        let mut v = View::new(
            ids[0],
            "Front".into(),
            ViewKind::Model {
                model: model.clone(),
                orientation: Orientation::Front,
                exploded: false,
            },
            [front_origin.x, front_origin.y],
        );
        v.display = Some(DisplayStyle::HiddenRemoved);
        sheet.views.push(v);
        sheet.views.push(View::new(
            ids[1],
            "Top".into(),
            ViewKind::Projected {
                parent: ids[0],
                placement: top_place,
            },
            [top_origin.x, top_origin.y],
        ));
        sheet.views.push(View::new(
            ids[2],
            if sheet.projection == Projection::Third {
                "Right".into()
            } else {
                "Left".into()
            },
            ViewKind::Projected {
                parent: ids[0],
                placement: side_place,
            },
            [side_origin.x, side_origin.y],
        ));
        if let Some(id) = iso_id {
            sheet.views.push(View::new(
                id,
                "Isometric".into(),
                ViewKind::Model {
                    model,
                    orientation: Orientation::Isometric,
                    exploded: false,
                },
                [iso_origin.x, iso_origin.y],
            ));
        }
        if sheet.properties_view.is_none() {
            sheet.properties_view = Some(ids[0]);
        }
        Ok(Added {
            front: ids[0],
            top: ids[1],
            side: ids[2],
            isometric: iso_id,
            scale,
        })
    }
}

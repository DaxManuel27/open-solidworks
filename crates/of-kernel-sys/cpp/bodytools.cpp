// Body tools: scaling (uniform or per axis), capping planar openings, and
// deforming a shape by moving the control points of its B-spline geometry
// (flex and deform).

#include "of-kernel-sys/cpp/ofk.h"
#include "of-kernel-sys/src/lib.rs.h"

#include <BRepBuilderAPI_GTransform.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_NurbsConvert.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepLib.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GeomConvert.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <ShapeAnalysis_FreeBounds.hxx>
#include <ShapeBuild_ReShape.hxx>
#include <ShapeFix_Shape.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_HSequenceOfShape.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <gp_GTrsf.hxx>
#include <gp_Trsf.hxx>

#include <stdexcept>
#include <vector>

namespace ofk {
namespace {

const TopAbs_ShapeEnum SUB[3] = {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX};

std::unique_ptr<Shape> wrap(const TopoDS_Shape &s) { return std::make_unique<Shape>(s); }

template <class Op>
void record(Op &op, const TopoDS_Shape &input, const TopoDS_Shape &out, rust::Vec<HistoryEntry> &hist) {
  TopTools_IndexedMapOfShape outm[3];
  for (int k = 0; k < 3; ++k) TopExp::MapShapes(out, SUB[k], outm[k]);
  for (uint8_t k = 0; k < 3; ++k) {
    TopTools_IndexedMapOfShape in;
    TopExp::MapShapes(input, SUB[k], in);
    for (int i = 1; i <= in.Extent(); ++i) {
      bool found = false;
      for (const TopoDS_Shape &m : op.Modified(in(i))) {
        int idx = outm[k].FindIndex(m);
        if (idx > 0) {
          hist.push_back(HistoryEntry{0, k, static_cast<uint32_t>(i - 1), 1, k, static_cast<uint32_t>(idx - 1)});
          found = true;
        }
      }
      if (!found) {
        int idx = outm[k].FindIndex(in(i));
        if (idx > 0)
          hist.push_back(HistoryEntry{0, k, static_cast<uint32_t>(i - 1), 0, k, static_cast<uint32_t>(idx - 1)});
      }
    }
  }
}

} // namespace

std::unique_ptr<Shape> scale_shape(const Shape &s, const Vec3 &center, const Vec3 &factors,
                                   rust::Vec<HistoryEntry> &hist) {
  if (factors.x <= 0 || factors.y <= 0 || factors.z <= 0)
    throw std::invalid_argument("scale factors must be more than zero");
  const gp_Pnt c(center.x, center.y, center.z);
  if (std::abs(factors.x - factors.y) < 1e-12 && std::abs(factors.y - factors.z) < 1e-12) {
    gp_Trsf t;
    t.SetScale(c, factors.x);
    BRepBuilderAPI_Transform op(s.shape, t, true);
    TopoDS_Shape out = op.Shape();
    record(op, s.shape, out, hist);
    return wrap(out);
  }
  gp_GTrsf g;
  g.SetValue(1, 1, factors.x);
  g.SetValue(2, 2, factors.y);
  g.SetValue(3, 3, factors.z);
  g.SetTranslationPart(gp_XYZ(c.X() * (1 - factors.x), c.Y() * (1 - factors.y), c.Z() * (1 - factors.z)));
  BRepBuilderAPI_GTransform op(s.shape, g, true);
  if (!op.IsDone()) throw std::runtime_error("the shape can't be scaled by axis");
  TopoDS_Shape out = op.Shape();
  if (!BRepCheck_Analyzer(out).IsValid()) throw std::runtime_error("scaling produced invalid geometry");
  record(op, s.shape, out, hist);
  return wrap(out);
}

std::unique_ptr<Shape> cap_loops(const Shape &s) {
  TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
  TopExp::MapShapesAndAncestors(s.shape, TopAbs_EDGE, TopAbs_FACE, edge_faces);
  Handle(TopTools_HSequenceOfShape) free = new TopTools_HSequenceOfShape;
  for (int i = 1; i <= edge_faces.Extent(); ++i) {
    const TopoDS_Edge &e = TopoDS::Edge(edge_faces.FindKey(i));
    if (!BRep_Tool::Degenerated(e) && edge_faces(i).Extent() == 1) free->Append(e);
  }
  BRep_Builder b;
  TopoDS_Compound out;
  b.MakeCompound(out);
  if (free->IsEmpty()) return wrap(out);
  Handle(TopTools_HSequenceOfShape) wires;
  ShapeAnalysis_FreeBounds::ConnectEdgesToWires(free, 1e-6, false, wires);
  for (int i = 1; i <= wires->Length(); ++i) {
    const TopoDS_Wire &w = TopoDS::Wire(wires->Value(i));
    if (!BRep_Tool::IsClosed(w)) continue;
    BRepBuilderAPI_MakeFace make(w, true);
    if (make.IsDone()) b.Add(out, make.Face());
  }
  return wrap(out);
}

// A copy of a shape whose surfaces and curves are B-splines, with their
// control points in one list: every face's surface poles, then every edge's
// curve poles, then every vertex.
struct Deformable::Impl {
  TopoDS_Shape shape;
  TopoDS_Shape input;
  BRepBuilderAPI_NurbsConvert convert;
  std::vector<Handle(Geom_BSplineSurface)> surfaces;
  std::vector<TopLoc_Location> surface_locs;
  std::vector<Handle(Geom_BSplineCurve)> curves;
  std::vector<TopLoc_Location> curve_locs;
  std::vector<TopoDS_Vertex> vertices;
};

Deformable::Deformable() : impl(std::make_unique<Impl>()) {}
Deformable::~Deformable() = default;

std::unique_ptr<Deformable> deformable_new(const Shape &s, uint32_t refine) {
  auto d = std::make_unique<Deformable>();
  d->impl->input = s.shape;
  d->impl->convert.Perform(s.shape, true);
  if (!d->impl->convert.IsDone()) throw std::runtime_error("the shape can't be converted to control points");
  d->impl->shape = d->impl->convert.Shape();
  TopTools_IndexedMapOfShape faces, edges, verts;
  TopExp::MapShapes(d->impl->shape, TopAbs_FACE, faces);
  TopExp::MapShapes(d->impl->shape, TopAbs_EDGE, edges);
  TopExp::MapShapes(d->impl->shape, TopAbs_VERTEX, verts);
  auto refine_surface = [&](Handle(Geom_BSplineSurface) bs) {
    for (bool in_u : {true, false}) {
      double a, b, c, e;
      bs->Bounds(a, b, c, e);
      const double lo = in_u ? a : c, hi = in_u ? b : e;
      for (uint32_t k = 1; k < refine; ++k) {
        const double t = lo + (hi - lo) * k / refine;
        if (in_u) bs->InsertUKnot(t, 1, 1e-9, true);
        else bs->InsertVKnot(t, 1, 1e-9, true);
      }
    }
  };
  for (int i = 1; i <= faces.Extent(); ++i) {
    const TopoDS_Face &f = TopoDS::Face(faces(i));
    TopLoc_Location loc;
    Handle(Geom_Surface) surf = BRep_Tool::Surface(f, loc);
    // The converted copy owns its geometry, so it can be refined in place;
    // the edges' curves on the surface stay valid since knots don't move it.
    Handle(Geom_BSplineSurface) bs = Handle(Geom_BSplineSurface)::DownCast(surf);
    if (bs.IsNull()) throw std::runtime_error("a face didn't convert to control points");
    refine_surface(bs);
    d->impl->surfaces.push_back(bs);
    d->impl->surface_locs.push_back(loc);
  }
  for (int i = 1; i <= edges.Extent(); ++i) {
    const TopoDS_Edge &e = TopoDS::Edge(edges(i));
    if (BRep_Tool::Degenerated(e)) {
      d->impl->curves.push_back(Handle(Geom_BSplineCurve)());
      d->impl->curve_locs.push_back(TopLoc_Location());
      continue;
    }
    TopLoc_Location loc;
    double f0, f1;
    Handle(Geom_Curve) c = BRep_Tool::Curve(e, loc, f0, f1);
    Handle(Geom_BSplineCurve) bc = Handle(Geom_BSplineCurve)::DownCast(c);
    if (!bc.IsNull()) {
      const double a = bc->FirstParameter(), b = bc->LastParameter();
      for (uint32_t k = 1; k < refine; ++k) bc->InsertKnot(a + (b - a) * k / refine, 1, 1e-9, true);
    }
    d->impl->curves.push_back(bc);
    d->impl->curve_locs.push_back(loc);
  }
  for (int i = 1; i <= verts.Extent(); ++i) d->impl->vertices.push_back(TopoDS::Vertex(verts(i)));
  return d;
}

rust::Vec<double> deformable_points(const Deformable &d) {
  rust::Vec<double> out;
  auto push = [&](const gp_Pnt &p) {
    out.push_back(p.X());
    out.push_back(p.Y());
    out.push_back(p.Z());
  };
  for (size_t i = 0; i < d.impl->surfaces.size(); ++i) {
    const auto &bs = d.impl->surfaces[i];
    const gp_Trsf t = d.impl->surface_locs[i].Transformation();
    for (int u = 1; u <= bs->NbUPoles(); ++u)
      for (int v = 1; v <= bs->NbVPoles(); ++v) push(bs->Pole(u, v).Transformed(t));
  }
  for (size_t i = 0; i < d.impl->curves.size(); ++i) {
    const auto &bc = d.impl->curves[i];
    if (bc.IsNull()) continue;
    const gp_Trsf t = d.impl->curve_locs[i].Transformation();
    for (int k = 1; k <= bc->NbPoles(); ++k) push(bc->Pole(k).Transformed(t));
  }
  for (const auto &v : d.impl->vertices) push(BRep_Tool::Pnt(v));
  return out;
}

std::unique_ptr<Shape> deformable_apply(Deformable &d, rust::Slice<const double> points,
                                        rust::Vec<HistoryEntry> &hist) {
  size_t at = 0;
  auto next = [&]() {
    if (at + 3 > points.size()) throw std::invalid_argument("too few control points");
    gp_Pnt p(points[at], points[at + 1], points[at + 2]);
    at += 3;
    return p;
  };
  for (size_t i = 0; i < d.impl->surfaces.size(); ++i) {
    auto &bs = d.impl->surfaces[i];
    const gp_Trsf inv = d.impl->surface_locs[i].Transformation().Inverted();
    for (int u = 1; u <= bs->NbUPoles(); ++u)
      for (int v = 1; v <= bs->NbVPoles(); ++v) bs->SetPole(u, v, next().Transformed(inv));
  }
  for (size_t i = 0; i < d.impl->curves.size(); ++i) {
    auto &bc = d.impl->curves[i];
    if (bc.IsNull()) continue;
    const gp_Trsf inv = d.impl->curve_locs[i].Transformation().Inverted();
    for (int k = 1; k <= bc->NbPoles(); ++k) bc->SetPole(k, next().Transformed(inv));
  }
  BRep_Builder builder;
  for (const auto &v : d.impl->vertices) builder.UpdateVertex(v, next(), BRep_Tool::Tolerance(v));
  // Edge curves and face surfaces moved separately: settle their tolerances.
  BRepLib::SameParameter(d.impl->shape, 1e-5, true);
  ShapeFix_Shape fix(d.impl->shape);
  fix.Perform();
  TopoDS_Shape out = fix.Shape();
  if (!BRepCheck_Analyzer(out).IsValid()) throw std::runtime_error("the deformed shape is invalid; deform it less");
  TopTools_IndexedMapOfShape outm[3];
  for (int k = 0; k < 3; ++k) TopExp::MapShapes(out, SUB[k], outm[k]);
  for (uint8_t k = 0; k < 3; ++k) {
    TopTools_IndexedMapOfShape in;
    TopExp::MapShapes(d.impl->input, SUB[k], in);
    for (int i = 1; i <= in.Extent(); ++i) {
      for (const TopoDS_Shape &m : d.impl->convert.Modified(in(i))) {
        TopoDS_Shape now = fix.Context().IsNull() ? m : fix.Context()->Apply(m);
        int idx = outm[k].FindIndex(now);
        if (idx > 0) {
          hist.push_back(HistoryEntry{0, k, static_cast<uint32_t>(i - 1), 1, k, static_cast<uint32_t>(idx - 1)});
          break;
        }
      }
    }
  }
  return wrap(out);
}

} // namespace ofk

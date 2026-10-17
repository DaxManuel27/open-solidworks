// Face editing support: faces extended past their boundaries, the closed
// cells a set of faces encloses, point classification, merging faces that
// lie on one surface, and holes removed from faces. Move, replace and offset
// face are built in Rust from these.

#include "of-kernel-sys/cpp/ofk.h"
#include "of-kernel-sys/src/lib.rs.h"

#include <BOPAlgo_MakerVolume.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepGProp.hxx>
#include <BRepGProp_Face.hxx>
#include <BRepTools.hxx>
#include <BRepTools_ReShape.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <GeomLib.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_BezierSurface.hxx>
#include <Geom_BoundedSurface.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <Geom_Surface.hxx>
#include <Precision.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>

#include <algorithm>
#include <cmath>
#include <stdexcept>
#include <vector>

namespace ofk {
namespace {

const TopAbs_ShapeEnum SUB[3] = {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX};

std::unique_ptr<Shape> wrap(const TopoDS_Shape &s) { return std::make_unique<Shape>(s); }

void push(rust::Vec<HistoryEntry> &hist, uint8_t from_kind, int from, uint8_t rel, uint8_t to_kind,
          int to) {
  hist.push_back(HistoryEntry{0, from_kind, static_cast<uint32_t>(from - 1), rel, to_kind,
                              static_cast<uint32_t>(to - 1)});
}

TopoDS_Face face_at(const TopoDS_Shape &s, uint32_t index) {
  TopTools_IndexedMapOfShape m;
  TopExp::MapShapes(s, TopAbs_FACE, m);
  if (index >= static_cast<uint32_t>(m.Extent())) throw std::out_of_range("no such face");
  const TopoDS_Shape &f = m(static_cast<int>(index) + 1);
  for (TopExp_Explorer ex(s, TopAbs_FACE); ex.More(); ex.Next())
    if (ex.Current().IsSame(f)) return TopoDS::Face(ex.Current());
  return TopoDS::Face(f);
}

// A point on a face, near its centre of area.
gp_Pnt point_on(const TopoDS_Face &face) {
  GProp_GProps props;
  BRepGProp::SurfaceProperties(face, props);
  BRepExtrema_DistShapeShape d(BRepBuilderAPI_MakeVertex(props.CentreOfMass()).Vertex(), face);
  if (!d.IsDone() || d.NbSolution() == 0) throw std::runtime_error("can't find a point on a face");
  return d.PointOnShape2(1);
}

} // namespace

std::unique_ptr<Shape> extend_face(const Shape &face_in, double length) {
  TopExp_Explorer ex(face_in.shape, TopAbs_FACE);
  if (!ex.More()) throw std::invalid_argument("extend needs a face");
  const TopoDS_Face face = TopoDS::Face(ex.Current());
  Handle(Geom_Surface) surface = BRep_Tool::Surface(face);
  while (surface->IsKind(STANDARD_TYPE(Geom_RectangularTrimmedSurface)))
    surface = Handle(Geom_RectangularTrimmedSurface)::DownCast(surface)->BasisSurface();
  double u0, u1, v0, v1;
  BRepTools::UVBounds(face, u0, u1, v0, v1);
  if (surface->IsKind(STANDARD_TYPE(Geom_BSplineSurface)) ||
      surface->IsKind(STANDARD_TYPE(Geom_BezierSurface))) {
    Handle(Geom_BoundedSurface) bounded = Handle(Geom_BoundedSurface)::DownCast(surface->Copy());
    for (bool in_u : {true, false})
      for (bool after : {true, false}) GeomLib::ExtendSurfByLength(bounded, length, 1, in_u, after);
    surface = bounded;
    surface->Bounds(u0, u1, v0, v1);
  } else {
    double nu0, nu1, nv0, nv1;
    surface->Bounds(nu0, nu1, nv0, nv1);
    // Parameter units per millimetre, from the surface's derivatives.
    gp_Pnt p;
    gp_Vec du, dv;
    surface->D1(0.5 * (u0 + u1), 0.5 * (v0 + v1), p, du, dv);
    const double eu = du.Magnitude() > 1e-12 ? length / du.Magnitude() : 0.0;
    const double ev = dv.Magnitude() > 1e-12 ? length / dv.Magnitude() : 0.0;
    if (surface->IsUPeriodic()) {
      u0 = nu0;
      u1 = nu1;
    } else {
      u0 = std::max(nu0, u0 - eu);
      u1 = std::min(nu1, u1 + eu);
    }
    if (surface->IsVPeriodic()) {
      v0 = nv0;
      v1 = nv1;
    } else {
      v0 = std::max(nv0, v0 - ev);
      v1 = std::min(nv1, v1 + ev);
    }
  }
  BRepBuilderAPI_MakeFace make(surface, u0, u1, v0, v1, Precision::Confusion());
  if (!make.IsDone()) throw std::runtime_error("can't extend the face");
  TopoDS_Face out = make.Face();
  if (face.Orientation() == TopAbs_REVERSED) out.Reverse();
  return wrap(out);
}

std::unique_ptr<Shape> cells(const Shape &faces) {
  BOPAlgo_MakerVolume mv;
  TopTools_ListOfShape args;
  for (TopExp_Explorer ex(faces.shape, TopAbs_FACE); ex.More(); ex.Next()) args.Append(ex.Current());
  if (args.IsEmpty()) throw std::invalid_argument("no faces to build cells from");
  mv.SetArguments(args);
  mv.SetIntersect(true);
  mv.SetAvoidInternalShapes(true);
  mv.SetRunParallel(true);
  mv.Perform();
  if (mv.HasErrors()) throw std::runtime_error("the faces don't enclose any region");
  return wrap(mv.Shape());
}

int8_t classify(const Shape &solid, const Vec3 &point) {
  BRepClass3d_SolidClassifier c(solid.shape, gp_Pnt(point.x, point.y, point.z), 1e-7);
  switch (c.State()) {
  case TopAbs_IN: return 1;
  case TopAbs_OUT: return -1;
  default: return 0;
  }
}

Vec3 inner_point(const Shape &solid) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(solid.shape, props);
  gp_Pnt c = props.CentreOfMass();
  BRepClass3d_SolidClassifier cls(solid.shape, c, 1e-7);
  if (cls.State() == TopAbs_IN) return Vec3{c.X(), c.Y(), c.Z()};
  // Step inside from the middle of each face until a point is inside.
  for (TopExp_Explorer ex(solid.shape, TopAbs_FACE); ex.More(); ex.Next()) {
    const TopoDS_Face &f = TopoDS::Face(ex.Current());
    double u0, u1, v0, v1;
    BRepTools::UVBounds(f, u0, u1, v0, v1);
    BRepGProp_Face prop(f);
    gp_Pnt p;
    gp_Vec n;
    prop.Normal(0.5 * (u0 + u1), 0.5 * (v0 + v1), p, n);
    if (n.Magnitude() < 1e-12) continue;
    n.Normalize();
    for (double eps : {1e-1, 1e-2, 1e-3, 1e-4}) {
      gp_Pnt q = p.Translated(-n * eps);
      BRepClass3d_SolidClassifier cq(solid.shape, q, 1e-7);
      if (cq.State() == TopAbs_IN) return Vec3{q.X(), q.Y(), q.Z()};
    }
  }
  throw std::runtime_error("can't find a point inside the region");
}

// Points strictly inside a face, on a grid over its parameter range.
static std::vector<gp_Pnt> interior_points(const TopoDS_Face &face) {
  std::vector<gp_Pnt> out;
  double u0, u1, v0, v1;
  BRepTools::UVBounds(face, u0, u1, v0, v1);
  BRepAdaptor_Surface surf(face);
  BRepClass_FaceClassifier cls;
  const int n = 5;
  for (int i = 1; i < n; ++i)
    for (int j = 1; j < n; ++j) {
      gp_Pnt2d uv(u0 + (u1 - u0) * i / n, v0 + (v1 - v0) * j / n);
      cls.Perform(face, uv, 1e-9);
      if (cls.State() == TopAbs_IN) out.push_back(surf.Value(uv.X(), uv.Y()));
    }
  if (out.empty()) out.push_back(point_on(face));
  return out;
}

rust::Vec<uint32_t> faces_on(const Shape &s, const Shape &target, double tolerance) {
  rust::Vec<uint32_t> out;
  TopTools_IndexedMapOfShape m;
  TopExp::MapShapes(s.shape, TopAbs_FACE, m);
  for (int i = 1; i <= m.Extent(); ++i) {
    std::vector<gp_Pnt> pts;
    try {
      pts = interior_points(TopoDS::Face(m(i)));
    } catch (...) {
      continue;
    }
    bool on = true;
    for (const gp_Pnt &p : pts) {
      BRepExtrema_DistShapeShape d(BRepBuilderAPI_MakeVertex(p).Vertex(), target.shape);
      if (!d.IsDone() || d.NbSolution() == 0 || d.Value() > tolerance) {
        on = false;
        break;
      }
    }
    if (on) out.push_back(static_cast<uint32_t>(i - 1));
  }
  return out;
}

std::unique_ptr<Shape> unify(const Shape &s, rust::Vec<HistoryEntry> &hist) {
  ShapeUpgrade_UnifySameDomain op(s.shape, true, true, false);
  op.Build();
  TopoDS_Shape out = op.Shape();
  if (out.IsNull()) throw std::runtime_error("merging faces produced no geometry");
  Handle(BRepTools_History) h = op.History();
  TopTools_IndexedMapOfShape outm[3];
  for (int k = 0; k < 3; ++k) TopExp::MapShapes(out, SUB[k], outm[k]);
  for (uint8_t k = 0; k < 3; ++k) {
    TopTools_IndexedMapOfShape in;
    TopExp::MapShapes(s.shape, SUB[k], in);
    for (int i = 1; i <= in.Extent(); ++i) {
      const TopoDS_Shape &x = in(i);
      bool modified = false;
      for (const TopoDS_Shape &m : h->Modified(x)) {
        int idx = outm[k].FindIndex(m);
        if (idx > 0) {
          push(hist, k, i, 1, k, idx);
          modified = true;
        }
      }
      if (!modified && !h->IsRemoved(x)) {
        int idx = outm[k].FindIndex(x);
        if (idx > 0) push(hist, k, i, 0, k, idx);
      }
    }
  }
  return wrap(out);
}

std::unique_ptr<Shape> remove_holes(const Shape &s, rust::Slice<const uint32_t> faces,
                                    rust::Vec<HistoryEntry> &hist) {
  BRepTools_ReShape reshape;
  for (uint32_t index : faces) {
    TopoDS_Face face = face_at(s.shape, index);
    TopoDS_Wire outer = BRepTools::OuterWire(face);
    int wires = 0;
    for (TopExp_Explorer ex(face, TopAbs_WIRE); ex.More(); ex.Next()) ++wires;
    if (wires < 2) throw std::invalid_argument("the face has no holes");
    TopoDS_Face bare = TopoDS::Face(face.EmptyCopied());
    BRep_Builder().Add(bare, outer);
    reshape.Replace(face, bare);
  }
  TopoDS_Shape out = reshape.Apply(s.shape);
  TopTools_IndexedMapOfShape outm[3];
  for (int k = 0; k < 3; ++k) TopExp::MapShapes(out, SUB[k], outm[k]);
  for (uint8_t k = 0; k < 3; ++k) {
    TopTools_IndexedMapOfShape in;
    TopExp::MapShapes(s.shape, SUB[k], in);
    for (int i = 1; i <= in.Extent(); ++i) {
      TopoDS_Shape now = reshape.Value(in(i));
      if (now.IsNull()) continue;
      int idx = outm[k].FindIndex(now);
      if (idx > 0) push(hist, k, i, now.IsSame(in(i)) ? 0 : 1, k, idx);
    }
  }
  return wrap(out);
}

std::unique_ptr<Shape> make_vertex(const Vec3 &p) {
  return wrap(BRepBuilderAPI_MakeVertex(gp_Pnt(p.x, p.y, p.z)).Vertex());
}

} // namespace ofk

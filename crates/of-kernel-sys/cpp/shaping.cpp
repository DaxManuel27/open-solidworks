// Surface shaping: ruled strips from edges (ruled and radiate surfaces,
// linear extension), surfaces continued past an edge, untrimming, and
// freeform edits of a face's control points.

#include "of-kernel-sys/cpp/ofk.h"
#include "of-kernel-sys/src/lib.rs.h"

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <ShapeFix_Edge.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <BRepLProp_SLProps.hxx>
#include <BRepTools.hxx>
#include <BRepTools_ReShape.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GCPnts_AbscissaPoint.hxx>
#include <GCPnts_UniformAbscissa.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <GeomAPI_PointsToBSpline.hxx>
#include <GeomAPI_PointsToBSplineSurface.hxx>
#include <GeomConvert.hxx>
#include <GeomFill.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <Precision.hxx>
#include <ShapeAnalysis_Surface.hxx>
#include <TColStd_Array1OfReal.hxx>
#include <TColgp_Array1OfPnt.hxx>
#include <TColgp_Array2OfPnt.hxx>
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

std::unique_ptr<Shape> wrap(const TopoDS_Shape &s) { return std::make_unique<Shape>(s); }

const TopAbs_ShapeEnum SUB[3] = {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX};

TopoDS_Face face_in(const TopoDS_Shape &s, uint32_t index) {
  TopTools_IndexedMapOfShape m;
  TopExp::MapShapes(s, TopAbs_FACE, m);
  if (index >= static_cast<uint32_t>(m.Extent())) throw std::out_of_range("no such face");
  const TopoDS_Shape &f = m(static_cast<int>(index) + 1);
  for (TopExp_Explorer ex(s, TopAbs_FACE); ex.More(); ex.Next())
    if (ex.Current().IsSame(f)) return TopoDS::Face(ex.Current());
  return TopoDS::Face(f);
}

TopoDS_Edge edge_in(const TopoDS_Shape &s, uint32_t index) {
  TopTools_IndexedMapOfShape m;
  TopExp::MapShapes(s, TopAbs_EDGE, m);
  if (index >= static_cast<uint32_t>(m.Extent())) throw std::out_of_range("no such edge");
  return TopoDS::Edge(m(static_cast<int>(index) + 1));
}

gp_Vec v(const Vec3 &x) { return gp_Vec(x.x, x.y, x.z); }

// The face's outward normal and the uv of the point on it nearest `p`.
gp_Vec normal_at(const TopoDS_Face &face, const gp_Pnt &p, gp_Pnt2d &uv) {
  Handle(Geom_Surface) s = BRep_Tool::Surface(face);
  ShapeAnalysis_Surface sas(s);
  uv = sas.ValueOfUV(p, 1e-7);
  BRepAdaptor_Surface a(face);
  BRepLProp_SLProps props(a, uv.X(), uv.Y(), 1, 1e-9);
  if (!props.IsNormalDefined()) throw std::runtime_error("the face has no normal at an edge point");
  gp_Vec n(props.Normal());
  if (face.Orientation() == TopAbs_REVERSED) n.Reverse();
  return n;
}

// Points along an edge, evenly by length, with their tangents.
void sample_edge(const TopoDS_Edge &edge, int n, std::vector<gp_Pnt> &pts, std::vector<gp_Vec> &tans) {
  BRepAdaptor_Curve c(edge);
  GCPnts_UniformAbscissa ab(c, n, 1e-7);
  if (!ab.IsDone()) throw std::runtime_error("can't sample the edge");
  for (int i = 1; i <= ab.NbPoints(); ++i) {
    gp_Pnt p;
    gp_Vec t;
    c.D1(ab.Parameter(i), p, t);
    if (edge.Orientation() == TopAbs_REVERSED) t.Reverse();
    pts.push_back(p);
    tans.push_back(t.Normalized());
  }
}

// Whether a step from the edge in direction d leaves the face.
bool leaves(const TopoDS_Face &face, const gp_Pnt &p, const gp_Vec &d, double step) {
  Handle(Geom_Surface) s = BRep_Tool::Surface(face);
  ShapeAnalysis_Surface sas(s);
  gp_Pnt2d uv = sas.ValueOfUV(p.Translated(d * step), 1e-7);
  BRepClass_FaceClassifier cls(face, uv, 1e-9);
  return cls.State() == TopAbs_OUT;
}

Handle(Geom_BSplineCurve) fit(const std::vector<gp_Pnt> &pts) {
  TColgp_Array1OfPnt arr(1, static_cast<int>(pts.size()));
  for (size_t i = 0; i < pts.size(); ++i) arr.SetValue(static_cast<int>(i) + 1, pts[i]);
  TColStd_Array1OfReal params(1, static_cast<int>(pts.size()));
  for (size_t i = 0; i < pts.size(); ++i)
    params.SetValue(static_cast<int>(i) + 1, static_cast<double>(i) / (pts.size() - 1));
  GeomAPI_PointsToBSpline make(arr, params, 3, 8, GeomAbs_C2, 1e-6);
  if (!make.IsDone()) throw std::runtime_error("can't fit a curve through the strip");
  return make.Curve();
}

} // namespace

// Mode: 0 tangent to the surface, 1 normal to it, 2 tapered to `vector` at
// `angle`, 3 perpendicular to `vector`, 4 swept (`vector` made square to the
// edge). Strips go away from the face, or the other way with `alternate`.
std::unique_ptr<Shape> ruled_strip(const Shape &s, uint32_t edge_index, int32_t face_index,
                                   uint8_t mode, const Vec3 &vector, rust::Slice<const double> size,
                                   bool alternate) {
  if (size.size() < 2) throw std::invalid_argument("a ruled strip needs a length and an angle");
  const double distance = size[0], angle = size[1];
  const TopoDS_Edge edge = edge_in(s.shape, edge_index);
  const bool has_face = face_index >= 0;
  TopoDS_Face face;
  if (has_face) face = face_in(s.shape, static_cast<uint32_t>(face_index));
  if (!has_face && mode < 2) throw std::invalid_argument("this ruled surface type needs the face beside the edge");
  BRepAdaptor_Curve c(edge);
  const double length = GCPnts_AbscissaPoint::Length(c);
  int n = std::clamp(static_cast<int>(length / (distance > 0 ? distance : 1.0) * 4) + 9, 9, 60);
  if (c.GetType() == GeomAbs_Line) n = 2;
  std::vector<gp_Pnt> pts;
  std::vector<gp_Vec> tans;
  sample_edge(edge, n, pts, tans);
  gp_Vec ref = v(vector);
  if (mode >= 2) {
    if (ref.Magnitude() < 1e-12) throw std::invalid_argument("select a direction for the ruled surface");
    ref.Normalize();
  }
  std::vector<gp_Pnt> far;
  for (size_t i = 0; i < pts.size(); ++i) {
    const gp_Vec &t = tans[i];
    gp_Vec nrm;
    gp_Pnt2d uv;
    if (has_face) nrm = normal_at(face, pts[i], uv);
    gp_Vec d;
    switch (mode) {
    case 0: d = nrm.Crossed(t); break;
    case 1: d = nrm; break;
    case 2: {
      gp_Vec side = ref.Crossed(t);
      if (side.Magnitude() < 1e-12) throw std::runtime_error("the direction runs along the edge");
      side.Normalize();
      if (has_face && !leaves(face, pts[i], side, 1e-3 * std::max(1.0, distance))) side.Reverse();
      d = ref * std::cos(angle) + side * std::sin(angle);
      break;
    }
    case 3: d = ref.Crossed(t); break;
    case 4: d = ref - t * ref.Dot(t); break;
    default: throw std::invalid_argument("unknown ruled surface type");
    }
    if (d.Magnitude() < 1e-12) throw std::runtime_error("the ruled direction vanishes along the edge");
    d.Normalize();
    if ((mode == 0 || mode == 3 || mode == 4) && has_face &&
        !leaves(face, pts[i], d, 1e-3 * std::max(1.0, distance)))
      d.Reverse();
    if (alternate) d.Reverse();
    far.push_back(pts[i].Translated(d * distance));
  }
  TopoDS_Face out;
  const gp_Vec d0(pts.front(), far.front()), d1(pts.back(), far.back());
  if (c.GetType() == GeomAbs_Line && d0.IsParallel(d1, 1e-9) && std::abs(d0.Magnitude() - d1.Magnitude()) < 1e-9) {
    // A straight edge moved along one direction: an exact flat strip.
    BRepPrimAPI_MakePrism prism(edge, d0);
    TopExp_Explorer fx(prism.Shape(), TopAbs_FACE);
    if (!fx.More()) throw std::runtime_error("the ruled surface is empty");
    out = TopoDS::Face(fx.Current());
  } else {
    Handle(Geom_BSplineCurve) a = fit(pts), b = fit(far);
    Handle(Geom_Surface) surf = GeomFill::Surface(a, b);
    out = BRepBuilderAPI_MakeFace(surf, Precision::Confusion()).Face();
  }
  if (!BRepCheck_Analyzer(out).IsValid()) throw std::runtime_error("the ruled surface is invalid");
  return wrap(out);
}

// The face beside `edge` continued past it by `distance`, following the
// face's own surface: a strip fitted through points marched across the edge.
std::unique_ptr<Shape> extend_strip(const Shape &s, uint32_t edge_index, uint32_t face_index,
                                    double distance) {
  const TopoDS_Edge edge = edge_in(s.shape, edge_index);
  const TopoDS_Face face = face_in(s.shape, face_index);
  Handle(Geom_Surface) surf = BRep_Tool::Surface(face);
  while (surf->IsKind(STANDARD_TYPE(Geom_RectangularTrimmedSurface)))
    surf = Handle(Geom_RectangularTrimmedSurface)::DownCast(surf)->BasisSurface();
  BRepAdaptor_Curve c(edge);
  const double length = GCPnts_AbscissaPoint::Length(c);
  const int n = std::clamp(static_cast<int>(length / std::max(distance, 1e-3) * 4) + 9, 9, 40);
  const int m = 6;
  std::vector<gp_Pnt> pts;
  std::vector<gp_Vec> tans;
  sample_edge(edge, n, pts, tans);
  ShapeAnalysis_Surface sas(surf);
  TColgp_Array2OfPnt grid(1, n, 1, m);
  for (int i = 0; i < n; ++i) {
    gp_Pnt2d uv;
    gp_Vec nrm = normal_at(face, pts[i], uv);
    gp_Vec d = nrm.Crossed(tans[i]).Normalized();
    if (!leaves(face, pts[i], d, 1e-3 * std::max(1.0, distance))) d.Reverse();
    // March across the edge on the surface, a step at a time.
    gp_Pnt2d at = uv;
    grid.SetValue(i + 1, 1, pts[i]);
    gp_Vec dir = d;
    for (int j = 1; j < m; ++j) {
      gp_Pnt p;
      gp_Vec du, dv;
      surf->D1(at.X(), at.Y(), p, du, dv);
      // Solve dir = a du + b dv in the least-squares sense.
      const double guu = du.Dot(du), guv = du.Dot(dv), gvv = dv.Dot(dv);
      const double det = guu * gvv - guv * guv;
      if (std::abs(det) < 1e-18) throw std::runtime_error("the surface is degenerate at the edge");
      const double ru = dir.Dot(du), rv = dir.Dot(dv);
      double a = (gvv * ru - guv * rv) / det, b = (guu * rv - guv * ru) / det;
      const double step = distance / (m - 1);
      const double len = (du * a + dv * b).Magnitude();
      a *= step / len;
      b *= step / len;
      at = gp_Pnt2d(at.X() + a, at.Y() + b);
      gp_Pnt q;
      gp_Vec qu, qv;
      surf->D1(at.X(), at.Y(), q, qu, qv);
      grid.SetValue(i + 1, j + 1, q);
      gp_Vec next(grid.Value(i + 1, j), q);
      if (next.Magnitude() > 1e-12) dir = next.Normalized();
    }
  }
  GeomAPI_PointsToBSplineSurface fit_surface(grid, 3, 8, GeomAbs_C2, 1e-6);
  if (!fit_surface.IsDone()) throw std::runtime_error("can't extend the surface there");
  TopoDS_Face out = BRepBuilderAPI_MakeFace(fit_surface.Surface(), Precision::Confusion()).Face();
  if (!BRepCheck_Analyzer(out).IsValid()) throw std::runtime_error("the extension is invalid");
  return wrap(out);
}

// A face rebuilt on its surface's natural bounds, or (for unbounded
// surfaces) on the box of its own parameter range.
std::unique_ptr<Shape> untrim_face(const Shape &s, uint32_t face_index) {
  const TopoDS_Face face = face_in(s.shape, face_index);
  Handle(Geom_Surface) surf = BRep_Tool::Surface(face);
  double u0, u1, v0, v1;
  BRepTools::UVBounds(face, u0, u1, v0, v1);
  Handle(Geom_Surface) basis = surf;
  while (basis->IsKind(STANDARD_TYPE(Geom_RectangularTrimmedSurface)))
    basis = Handle(Geom_RectangularTrimmedSurface)::DownCast(basis)->BasisSurface();
  double nu0, nu1, nv0, nv1;
  basis->Bounds(nu0, nu1, nv0, nv1);
  if (!Precision::IsInfinite(nu0) && !Precision::IsInfinite(nu1)) { u0 = nu0; u1 = nu1; }
  if (!Precision::IsInfinite(nv0) && !Precision::IsInfinite(nv1)) { v0 = nv0; v1 = nv1; }
  BRepBuilderAPI_MakeFace make(basis, u0, u1, v0, v1, Precision::Confusion());
  if (!make.IsDone()) throw std::runtime_error("can't untrim the face");
  TopoDS_Face out = make.Face();
  if (face.Orientation() == TopAbs_REVERSED) out.Reverse();
  return wrap(out);
}

// Move control points of a face. The face's surface becomes a B-spline with
// at least `grid` x `grid` poles; each move (u, v in 0..1 and a
// displacement, five values) shifts the nearest pole. `fixed` rows of poles
// at each boundary stay put: 1 keeps the boundary, 2 its tangency, 3 its
// curvature. The face's edges stay where they are.
std::unique_ptr<Shape> freeform_face(const Shape &s, uint32_t face_index, uint32_t grid,
                                     rust::Slice<const double> moves, uint32_t fixed,
                                     rust::Vec<HistoryEntry> &hist) {
  // Work on a copy, so edges shared with earlier states are never changed.
  BRepBuilderAPI_Copy copy(s.shape, true, false);
  const TopoDS_Shape work = copy.Shape();
  const TopoDS_Face original = face_in(s.shape, face_index);
  const TopoDS_Face face = TopoDS::Face(copy.Modified(original).First().Oriented(original.Orientation()));
  double u0, u1, v0, v1;
  BRepTools::UVBounds(face, u0, u1, v0, v1);
  Handle(Geom_Surface) surface = BRep_Tool::Surface(face);
  Handle(Geom_BSplineSurface) bs = GeomConvert::SurfaceToBSplineSurface(
      new Geom_RectangularTrimmedSurface(surface, u0, u1, v0, v1));
  bs = Handle(Geom_BSplineSurface)::DownCast(bs->Copy());
  // Refine to the requested grid by inserting knots evenly.
  const int want = static_cast<int>(std::max<uint32_t>(grid, 4));
  for (bool in_u : {true, false}) {
    double a, b, c, d;
    bs->Bounds(a, b, c, d);
    const double lo = in_u ? a : c, hi = in_u ? b : d;
    for (int k = 1; (in_u ? bs->NbUPoles() : bs->NbVPoles()) < want && k < want; ++k) {
      const double t = lo + (hi - lo) * k / want;
      if (in_u) bs->InsertUKnot(t, 1, 1e-9, true);
      else bs->InsertVKnot(t, 1, 1e-9, true);
    }
  }
  const int nu = bs->NbUPoles(), nv = bs->NbVPoles();
  const int keep = static_cast<int>(fixed);
  if (2 * keep >= nu || 2 * keep >= nv) throw std::invalid_argument("the face has too few control points to move");
  if (moves.size() % 5 != 0) throw std::invalid_argument("each move needs u, v and a displacement");
  for (size_t i = 0; i < moves.size(); i += 5) {
    const double u = std::clamp(moves[i], 0.0, 1.0), w = std::clamp(moves[i + 1], 0.0, 1.0);
    int pi = 1 + static_cast<int>(std::lround(u * (nu - 1)));
    int pj = 1 + static_cast<int>(std::lround(w * (nv - 1)));
    pi = std::clamp(pi, 1 + keep, nu - keep);
    pj = std::clamp(pj, 1 + keep, nv - keep);
    gp_Pnt p = bs->Pole(pi, pj);
    p.Translate(gp_Vec(moves[i + 2], moves[i + 3], moves[i + 4]));
    bs->SetPole(pi, pj, p);
  }
  // The same edges on the new surface, with their curves on it rebuilt.
  TopoDS_Face edited = TopoDS::Face(face.EmptyCopied());
  BRep_Builder builder;
  builder.UpdateFace(edited, bs, TopLoc_Location(), BRep_Tool::Tolerance(face));
  for (TopExp_Explorer wx(face.Oriented(TopAbs_FORWARD), TopAbs_WIRE); wx.More(); wx.Next())
    builder.Add(edited, wx.Current());
  ShapeFix_Edge fix_edge;
  for (TopExp_Explorer ex(edited, TopAbs_EDGE); ex.More(); ex.Next()) {
    const TopoDS_Edge &e = TopoDS::Edge(ex.Current());
    fix_edge.FixAddPCurve(e, edited, BRep_Tool::IsClosed(e, edited), 1e-6);
  }
  BRepTools_ReShape reshape;
  reshape.Replace(face, edited.Oriented(face.Orientation()));
  TopoDS_Shape out = reshape.Apply(work);
  if (!BRepCheck_Analyzer(out).IsValid())
    throw std::runtime_error("the edited face no longer meets its edges; move points further from them");
  TopTools_IndexedMapOfShape outm[3];
  for (int k = 0; k < 3; ++k) TopExp::MapShapes(out, SUB[k], outm[k]);
  for (uint8_t k = 0; k < 3; ++k) {
    TopTools_IndexedMapOfShape in;
    TopExp::MapShapes(s.shape, SUB[k], in);
    for (int i = 1; i <= in.Extent(); ++i) {
      const TopTools_ListOfShape &copies = copy.Modified(in(i));
      if (copies.IsEmpty()) continue;
      TopoDS_Shape now = reshape.Value(copies.First());
      int idx = now.IsNull() ? 0 : outm[k].FindIndex(now);
      if (idx > 0)
        hist.push_back(HistoryEntry{0, k, static_cast<uint32_t>(i - 1),
                                    static_cast<uint8_t>(now.IsSame(copies.First()) ? 0 : 1), k,
                                    static_cast<uint32_t>(idx - 1)});
    }
  }
  return wrap(out);
}

} // namespace ofk

// Boundary: a solid bounded by a network of closed sections (direction 1) and
// direction-2 curves crossing every section. The network is split at its
// crossings into four-sided cells; each cell is filled with a Coons patch that
// interpolates all four of its boundary curves exactly, so both curve families
// constrain the surface. Planar end sections are capped and the patches sewn.
#include "ofk.h"
#include "of-kernel-sys/src/lib.rs.h"

#include <BOPAlgo_ArgumentAnalyzer.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepGProp.hxx>
#include <BRepLib.hxx>
#include <BRepLib_FindSurface.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <GProp_GProps.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <GeomConvert.hxx>
#include <GeomConvert_CompCurveToBSplineCurve.hxx>
#include <GeomFill.hxx>
#include <GeomFill_BSplineCurves.hxx>
#include <gp_Lin.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Iterator.hxx>
#include <TopoDS_Solid.hxx>
#include <TopoDS_Wire.hxx>

#include <algorithm>
#include <cmath>
#include <stdexcept>
#include <string>
#include <vector>

namespace ofk {
namespace {
enum : uint8_t { GENERATED = 2, FIRST_CAP = 3, LAST_CAP = 4 };

Handle(Geom_BSplineCurve) wire_curve(const TopoDS_Wire &wire, double tol) {
  GeomConvert_CompCurveToBSplineCurve join;
  int count = 0;
  for (BRepTools_WireExplorer e(wire); e.More(); e.Next()) {
    double first, last;
    Handle(Geom_Curve) curve = BRep_Tool::Curve(e.Current(), first, last);
    if (curve.IsNull()) throw std::invalid_argument("boundary curve has an edge without geometry");
    Handle(Geom_BSplineCurve) piece =
        GeomConvert::CurveToBSplineCurve(new Geom_TrimmedCurve(curve, first, last));
    if (e.Current().Orientation() == TopAbs_REVERSED) piece->Reverse();
    if (!join.Add(piece, tol)) throw std::invalid_argument("boundary curve segments are not connected");
    ++count;
  }
  if (!count) throw std::invalid_argument("boundary curve is empty");
  return join.BSplineCurve();
}

Handle(Geom_BSplineCurve) segment(const Handle(Geom_BSplineCurve) &curve, double a, double b) {
  Handle(Geom_BSplineCurve) copy = Handle(Geom_BSplineCurve)::DownCast(curve->Copy());
  copy->Segment(a, b);
  return copy;
}

// The part of closed `curve` running forward from `a` to `b`, across its end if needed.
Handle(Geom_BSplineCurve) forward(const Handle(Geom_BSplineCurve) &curve, double a, double b,
                                  double tol) {
  const double first = curve->FirstParameter(), last = curve->LastParameter();
  const double eps = 1e-12 * std::max(1.0, last - first);
  if (b > a + eps) return segment(curve, a, b);
  GeomConvert_CompCurveToBSplineCurve join;
  if (last - a > eps) join.Add(segment(curve, a, last), tol);
  if (b - first > eps && !join.Add(segment(curve, first, b), tol))
    throw std::runtime_error("could not split a boundary section");
  return join.BSplineCurve();
}

double project(const Handle(Geom_BSplineCurve) &curve, const gp_Pnt &point) {
  GeomAPI_ProjectPointOnCurve projection(point, curve);
  if (!projection.NbPoints()) {
    // Endpoints are not always reported as extrema.
    return point.Distance(curve->StartPoint()) <= point.Distance(curve->EndPoint())
               ? curve->FirstParameter()
               : curve->LastParameter();
  }
  double best = projection.LowerDistanceParameter();
  if (point.Distance(curve->StartPoint()) < projection.LowerDistance()) best = curve->FirstParameter();
  if (point.Distance(curve->EndPoint()) < projection.LowerDistance()) best = curve->LastParameter();
  return best;
}

// Whether every pole lies on the chord, so the curve is a straight segment.
bool straight(const Handle(Geom_BSplineCurve) &curve, double tol) {
  const gp_Pnt a = curve->StartPoint(), b = curve->EndPoint();
  if (a.Distance(b) <= tol) return false;
  const gp_Lin chord(a, gp_Dir(gp_Vec(a, b)));
  for (int i = 1; i <= curve->NbPoles(); ++i)
    if (chord.Distance(curve->Pole(i)) > tol) return false;
  return true;
}

std::vector<TopoDS_Wire> wires_of(const TopoDS_Shape &shape) {
  std::vector<TopoDS_Wire> out;
  for (TopoDS_Iterator it(shape); it.More(); it.Next()) {
    if (it.Value().ShapeType() != TopAbs_WIRE) throw std::invalid_argument("boundary curves must be wires");
    out.push_back(TopoDS::Wire(it.Value()));
  }
  return out;
}
} // namespace

std::unique_ptr<Shape> boundary(const Shape &sections_in, const Shape &guides_in,
                                rust::Vec<HistoryEntry> &hist) {
  const auto section_wires = wires_of(sections_in.shape);
  const auto guide_wires = wires_of(guides_in.shape);
  const int n = static_cast<int>(section_wires.size()), k = static_cast<int>(guide_wires.size());
  if (n < 2) throw std::invalid_argument("select at least two direction 1 sections");
  if (k < 2) throw std::invalid_argument("select at least two direction 2 curves");

  Bnd_Box box;
  BRepBndLib::Add(sections_in.shape, box);
  BRepBndLib::Add(guides_in.shape, box);
  const double scale = std::max(1.0, std::sqrt(box.SquareExtent()));
  const double tol = 1e-6 * scale;

  std::vector<Handle(Geom_BSplineCurve)> S, G;
  for (int i = 0; i < n; ++i) {
    const auto &w = section_wires[i];
    if (!BRep_Tool::IsClosed(w))
      throw std::invalid_argument("direction 1 section " + std::to_string(i + 1) + " is not closed");
    if (i == 0 || i == n - 1) {
      BRepLib_FindSurface plane(w, tol, true);
      if (!plane.Found())
        throw std::invalid_argument("the first and last boundary sections must be planar to cap them");
    }
    S.push_back(wire_curve(w, tol));
  }
  for (const auto &w : guide_wires) G.push_back(wire_curve(w, tol));

  // Where each direction 2 curve crosses each section, in both curves' parameters.
  std::vector<std::vector<double>> s(n, std::vector<double>(k)), t(n, std::vector<double>(k));
  for (int i = 0; i < n; ++i)
    for (int j = 0; j < k; ++j) {
      BRepExtrema_DistShapeShape dist(section_wires[i], guide_wires[j]);
      if (!dist.IsDone() || dist.Value() > tol)
        throw std::invalid_argument("direction 2 curve " + std::to_string(j + 1) +
                                    " does not touch section " + std::to_string(i + 1));
      const gp_Pnt p = dist.PointOnShape1(1);
      s[i][j] = project(S[i], p);
      t[i][j] = project(G[j], p);
    }
  for (int j = 0; j < k; ++j) {
    if (t[n - 1][j] < t[0][j]) {
      const double sum = G[j]->FirstParameter() + G[j]->LastParameter();
      G[j]->Reverse();
      for (int i = 0; i < n; ++i) t[i][j] = sum - t[i][j];
    }
    for (int i = 1; i < n; ++i)
      if (!(t[i][j] > t[i - 1][j]))
        throw std::invalid_argument("direction 2 curve " + std::to_string(j + 1) +
                                    " must cross the sections in their order");
  }
  // Order the direction 2 curves around the first section; every section must
  // see the same cyclic order, or the network is twisted.
  for (int i = 0; i < n; ++i) {
    const double first = S[i]->FirstParameter(), period = S[i]->LastParameter() - first;
    for (int j = 0; j < k; ++j) {
      s[i][j] = first + std::fmod(s[i][j] - first, period);
      if (s[i][j] < first) s[i][j] += period;
    }
  }
  std::vector<int> order(k);
  for (int j = 0; j < k; ++j) order[j] = j;
  std::sort(order.begin(), order.end(), [&](int a, int b) { return s[0][a] < s[0][b]; });
  for (int i = 0; i < n; ++i) {
    int wraps = 0;
    for (int m = 0; m < k; ++m) {
      const double a = s[i][order[m]], b = s[i][order[(m + 1) % k]];
      if (std::abs(b - a) < 1e-12) throw std::invalid_argument("two direction 2 curves meet a section at one point");
      if (b < a) ++wraps;
    }
    if (wraps != 1)
      throw std::invalid_argument("direction 2 curves must cross every section in the same order");
  }

  BRepBuilderAPI_Sewing sewing(10 * tol);
  std::vector<TopoDS_Face> patches;
  for (int i = 0; i + 1 < n; ++i)
    for (int m = 0; m < k; ++m) {
      const int a = order[m], b = order[(m + 1) % k];
      Handle(Geom_BSplineCurve) c1 = forward(S[i], s[i][a], s[i][b], tol);
      Handle(Geom_BSplineCurve) c2 = segment(G[b], t[i][b], t[i + 1][b]);
      Handle(Geom_BSplineCurve) c3 = forward(S[i + 1], s[i + 1][a], s[i + 1][b], tol);
      Handle(Geom_BSplineCurve) c4 = segment(G[a], t[i][a], t[i + 1][a]);
      c3->Reverse();
      c4->Reverse();
      Handle(Geom_Surface) surface;
      if (straight(c2, tol) && straight(c4, tol)) {
        // With straight sides a Coons patch is the ruled surface between its
        // sections; building that directly keeps rational sections exact.
        Handle(Geom_BSplineCurve) a = c1, b = Handle(Geom_BSplineCurve)::DownCast(c3->Reversed());
        surface = GeomFill::Surface(a, b);
      } else {
        // Coons filling needs at least four poles on each boundary.
        for (const auto &c : {c1, c2, c3, c4})
          if (c->Degree() < 3) c->IncreaseDegree(3);
        GeomFill_BSplineCurves fill(c1, c2, c3, c4, GeomFill_CoonsStyle);
        surface = fill.Surface();
      }
      TopoDS_Face face = BRepBuilderAPI_MakeFace(surface, Precision::Confusion()).Face();
      if (!BRepCheck_Analyzer(face).IsValid())
        throw std::runtime_error("a boundary patch is invalid; check the curve network");
      patches.push_back(face);
      sewing.Add(face);
    }
  std::vector<TopoDS_Face> caps;
  for (int i : {0, n - 1}) {
    BRepBuilderAPI_MakeFace cap(section_wires[i], true);
    if (!cap.IsDone()) throw std::runtime_error("could not cap a boundary end section");
    caps.push_back(cap.Face());
    sewing.Add(cap.Face());
  }
  sewing.Perform();
  TopExp_Explorer shells(sewing.SewedShape(), TopAbs_SHELL);
  if (!shells.More() || sewing.NbFreeEdges() > 0)
    throw std::runtime_error("the boundary patches do not close; check that the curves meet");
  BRepBuilderAPI_MakeSolid make(TopoDS::Shell(shells.Current()));
  shells.Next();
  if (shells.More() || !make.IsDone()) throw std::runtime_error("the boundary does not form one solid");
  TopoDS_Solid solid = make.Solid();
  if (!BRepLib::OrientClosedSolid(solid)) throw std::runtime_error("the boundary solid is not closed");
  if (!BRepCheck_Analyzer(solid).IsValid()) throw std::runtime_error("the boundary solid is invalid");
  BOPAlgo_ArgumentAnalyzer self_check;
  self_check.SetShape1(solid);
  self_check.SelfInterMode() = true;
  self_check.Perform();
  if (self_check.HasFaulty())
    throw std::runtime_error("the boundary surface intersects itself; adjust the curves");
  GProp_GProps props;
  BRepGProp::VolumeProperties(solid, props);
  if (!(props.Mass() > 0)) throw std::runtime_error("the boundary encloses no volume");

  TopTools_IndexedMapOfShape faces;
  TopExp::MapShapes(solid, TopAbs_FACE, faces);
  auto record = [&](const TopoDS_Face &face, uint8_t rel, uint32_t from) {
    TopoDS_Shape sewn = sewing.IsModified(face) ? sewing.Modified(face) : face;
    const int index = faces.FindIndex(sewn);
    if (index > 0) hist.push_back(HistoryEntry{0, 0, from, rel, 0, static_cast<uint32_t>(index - 1)});
  };
  // Patch (span i, gap m) is reported as generated from index i * k + m.
  for (size_t p = 0; p < patches.size(); ++p) record(patches[p], GENERATED, static_cast<uint32_t>(p));
  record(caps[0], FIRST_CAP, 0);
  record(caps[1], LAST_CAP, 0);
  return std::make_unique<Shape>(solid);
}
} // namespace ofk

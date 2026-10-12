// Lofts that ThruSections can't build: skinning through closed sections with
// start/end tangent conditions, and lofts whose sections follow a centerline.
//
// Skinning makes each section edge a B-spline compatible with the matching
// edges of the other sections, then interpolates every pole row across the
// sections. A tangent condition is the derivative of every pole row at that
// end, so the surface leaves the section along exactly that vector.
#include "of-kernel-sys/cpp/ofk.h"
#include "of-kernel-sys/src/lib.rs.h"

#include <BOPAlgo_ArgumentAnalyzer.hxx>
#include <gp_Mat2d.hxx>
#include <gp_GTrsf.hxx>
#include <TopoDS_Compound.hxx>
#include <BRep_Builder.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepBuilderAPI_GTransform.hxx>
#include <BRepAdaptor_CompCurve.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepFill_CompatibleWires.hxx>
#include <BRepGProp.hxx>
#include <BRepLib.hxx>
#include <BRepLib_FindSurface.hxx>
#include <BRepOffsetAPI_MakePipeShell.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <GProp_GProps.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <GeomConvert.hxx>
#include <GeomConvert_ApproxCurve.hxx>
#include <GeomConvert_CompCurveToBSplineCurve.hxx>
#include <GeomFill_Profiler.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_Plane.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <TColStd_HArray1OfBoolean.hxx>
#include <TColStd_HArray1OfReal.hxx>
#include <TColgp_Array1OfVec.hxx>
#include <TColStd_Array2OfReal.hxx>
#include <TColgp_Array2OfPnt.hxx>
#include <TColgp_HArray1OfPnt.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopTools_SequenceOfShape.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Iterator.hxx>
#include <TopoDS_Solid.hxx>
#include <TopoDS_Wire.hxx>
#include <gp_Pln.hxx>

#include <cmath>
#include <stdexcept>
#include <string>
#include <memory>
#include <vector>

namespace ofk {
namespace {
enum : uint8_t { GENERATED = 2, FIRST_CAP = 3, LAST_CAP = 4 };
enum : uint8_t { FREE = 0, NORMAL = 1, DIRECTION = 2 };

std::vector<TopoDS_Wire> wires_in(const TopoDS_Shape &shape) {
  std::vector<TopoDS_Wire> out;
  for (TopoDS_Iterator it(shape); it.More(); it.Next()) {
    if (it.Value().ShapeType() != TopAbs_WIRE) throw std::invalid_argument("loft sections must be wires");
    out.push_back(TopoDS::Wire(it.Value()));
  }
  return out;
}

double scale_of(const TopoDS_Shape &shape) {
  Bnd_Box box;
  BRepBndLib::Add(shape, box);
  return std::max(1.0, std::sqrt(box.SquareExtent()));
}

gp_Pnt centroid(const TopoDS_Wire &wire) {
  GProp_GProps props;
  BRepGProp::LinearProperties(wire, props);
  return props.CentreOfMass();
}

gp_Pln plane_of(const TopoDS_Wire &wire, double tol, const char *what) {
  BRepLib_FindSurface find(wire, tol, true);
  if (!find.Found()) throw std::invalid_argument(what);
  return Handle(Geom_Plane)::DownCast(find.Surface())->Pln();
}

// A B-spline for one oriented wire edge; with `approximate`, a non-rational one
// within `tol` of it.
Handle(Geom_BSplineCurve) edge_spline(const TopoDS_Edge &edge, double tol, bool approximate) {
  double first, last;
  Handle(Geom_Curve) curve = BRep_Tool::Curve(edge, first, last);
  if (curve.IsNull()) throw std::invalid_argument("a loft section has an edge without geometry");
  Handle(Geom_TrimmedCurve) trimmed = new Geom_TrimmedCurve(curve, first, last);
  Handle(Geom_BSplineCurve) spline = GeomConvert::CurveToBSplineCurve(trimmed);
  if (spline->IsPeriodic()) spline->SetNotPeriodic();
  if (approximate && spline->IsRational()) {
    GeomConvert_ApproxCurve approx(spline, tol, GeomAbs_C2, 200, 8);
    if (!approx.HasResult() || approx.MaxError() > tol)
      throw std::runtime_error("could not represent a loft section curve accurately");
    spline = approx.Curve();
  }
  if (edge.Orientation() == TopAbs_REVERSED) spline->Reverse();
  return spline;
}

std::vector<TopoDS_Edge> ordered_edges(const TopoDS_Wire &wire) {
  std::vector<TopoDS_Edge> out;
  for (BRepTools_WireExplorer e(wire); e.More(); e.Next()) out.push_back(e.Current());
  return out;
}

TopoDS_Solid close_solid(BRepBuilderAPI_Sewing &sewing, const char *what) {
  sewing.Perform();
  TopExp_Explorer shells(sewing.SewedShape(), TopAbs_SHELL);
  if (!shells.More() || sewing.NbFreeEdges() > 0)
    throw std::runtime_error(std::string(what) + " faces do not close");
  BRepBuilderAPI_MakeSolid make(TopoDS::Shell(shells.Current()));
  shells.Next();
  if (shells.More() || !make.IsDone()) throw std::runtime_error(std::string(what) + " does not form one solid");
  TopoDS_Solid solid = make.Solid();
  if (!BRepLib::OrientClosedSolid(solid)) throw std::runtime_error(std::string(what) + " is not closed");
  if (!BRepCheck_Analyzer(solid).IsValid()) throw std::runtime_error(std::string(what) + " is invalid");
  BOPAlgo_ArgumentAnalyzer self_check;
  self_check.SetShape1(solid);
  self_check.SelfInterMode() = true;
  self_check.Perform();
  if (self_check.HasFaulty())
    throw std::runtime_error(std::string(what) + " intersects itself; change the sections or tangent lengths");
  GProp_GProps props;
  BRepGProp::VolumeProperties(solid, props);
  if (!(props.Mass() > 0)) throw std::runtime_error(std::string(what) + " encloses no volume");
  return solid;
}

// One B-spline through a wire's edges in order.
Handle(Geom_BSplineCurve) joined(const TopoDS_Wire &wire, double tol, const char *what) {
  GeomConvert_CompCurveToBSplineCurve join;
  int count = 0;
  for (BRepTools_WireExplorer e(wire); e.More(); e.Next()) {
    double first, last;
    Handle(Geom_Curve) curve = BRep_Tool::Curve(e.Current(), first, last);
    if (curve.IsNull()) throw std::invalid_argument(std::string(what) + " has an edge without geometry");
    Handle(Geom_BSplineCurve) piece = GeomConvert::CurveToBSplineCurve(new Geom_TrimmedCurve(curve, first, last));
    if (e.Current().Orientation() == TopAbs_REVERSED) piece->Reverse();
    if (!join.Add(piece, tol)) throw std::invalid_argument(std::string(what) + " segments are not connected");
    ++count;
  }
  if (!count) throw std::invalid_argument(std::string(what) + " is empty");
  return join.BSplineCurve();
}

TopoDS_Wire first_wire(const TopoDS_Shape &shape, const char *what) {
  if (shape.ShapeType() == TopAbs_WIRE) return TopoDS::Wire(shape);
  TopExp_Explorer ex(shape, TopAbs_WIRE);
  if (ex.More()) return TopoDS::Wire(ex.Current());
  BRepBuilderAPI_MakeWire make;
  for (TopExp_Explorer e(shape, TopAbs_EDGE); e.More(); e.Next()) make.Add(TopoDS::Edge(e.Current()));
  if (!make.IsDone()) throw std::invalid_argument(std::string(what) + " must be one connected curve");
  return make.Wire();
}

struct EndCondition {
  uint8_t mode = FREE;
  gp_Vec dir;
  double weight = 1;
};

struct Skin {
  TopoDS_Solid solid;
  std::vector<TopoDS_Edge> first;   // the first section's edges, in wire order
  std::vector<TopoDS_Shape> sides;  // the side face from each of those edges
  TopoDS_Shape caps[2];
};

// Skin through closed wires; see the file comment.
Skin skin_wires(std::vector<TopoDS_Wire> wires, bool preserve, const EndCondition &start_condition,
                const EndCondition &end_condition, double tol) {
  const int n = static_cast<int>(wires.size());
  if (n < 2) throw std::invalid_argument("a loft needs at least two sections");
  for (const auto &w : wires)
    if (!BRep_Tool::IsClosed(w)) throw std::invalid_argument("loft sections must be closed");
  for (double w : {start_condition.weight, end_condition.weight})
    if (!std::isfinite(w) || w <= 0) throw std::invalid_argument("tangent lengths must be positive");

  if (!preserve) {
    TopTools_SequenceOfShape seq;
    for (const auto &w : wires) seq.Append(w);
    BRepFill_CompatibleWires compatible(seq);
    compatible.Perform();
    if (!compatible.IsDone()) throw std::runtime_error("could not match the loft sections' edges");
    for (int i = 0; i < n; ++i) wires[i] = TopoDS::Wire(compatible.Shape().Value(i + 1));
  }
  std::vector<std::vector<TopoDS_Edge>> edges;
  for (const auto &w : wires) edges.push_back(ordered_edges(w));
  const size_t m = edges[0].size();
  for (const auto &e : edges)
    if (e.size() != m) throw std::invalid_argument("loft sections need the same number of edges");

  // Interpolation parameters: chord length between section centroids.
  std::vector<gp_Pnt> centers;
  for (const auto &w : wires) centers.push_back(centroid(w));
  Handle(TColStd_HArray1OfReal) params = new TColStd_HArray1OfReal(1, n);
  params->SetValue(1, 0);
  for (int i = 1; i < n; ++i) {
    const double step = centers[i].Distance(centers[i - 1]);
    if (step < tol) throw std::invalid_argument("two loft sections share a position");
    params->SetValue(i + 1, params->Value(i) + step);
  }

  // The end tangents, in units of the chord-length parameter.
  auto tangent = [&](const EndCondition &c, int at, int toward) -> gp_Vec {
    gp_Vec forward(centers[std::min(at, toward)], centers[std::max(at, toward)]);
    gp_Vec t;
    if (c.mode == NORMAL) {
      const gp_Pln plane = plane_of(wires[at], tol, "tangency normal to a profile needs a planar end section");
      t = gp_Vec(plane.Axis().Direction());
      if (t.Dot(forward) < 0) t.Reverse();
    } else if (c.mode == DIRECTION) {
      t = c.dir;
      if (t.Magnitude() < 1e-12) throw std::invalid_argument("the tangent direction is zero");
      t.Normalize();
    } else {
      throw std::invalid_argument("unknown loft end condition");
    }
    return t * c.weight;
  };
  const bool start = start_condition.mode != FREE, end = end_condition.mode != FREE;
  gp_Vec t0, t1;
  if (start) t0 = tangent(start_condition, 0, 1);
  if (end) t1 = tangent(end_condition, n - 1, n - 2);

  BRepBuilderAPI_Sewing sewing(10 * tol);
  std::vector<TopoDS_Face> sides;
  for (size_t j = 0; j < m; ++j) {
    // Rational sections stay exact when their weights agree pole for pole (arcs
    // and circles of any size do): the weights then stay constant across the
    // sections. Otherwise use close non-rational approximations.
    auto compatible = [&](bool approximate) {
      auto profiler = std::make_unique<GeomFill_Profiler>();
      for (int i = 0; i < n; ++i) profiler->AddCurve(edge_spline(edges[i][j], tol, approximate));
      profiler->Perform(1e-9);
      return profiler;
    };
    auto profiler = compatible(false);
    int poles = profiler->NbPoles();
    TColStd_Array1OfReal weights(1, poles);
    profiler->Weights(1, weights);
    for (int i = 2; i <= n && profiler; ++i) {
      TColStd_Array1OfReal other(1, poles);
      profiler->Weights(i, other);
      for (int k = 1; k <= poles; ++k)
        if (std::abs(other(k) - weights(k)) > 1e-12 * std::abs(weights(k))) {
          profiler = compatible(true);
          poles = profiler->NbPoles();
          weights = TColStd_Array1OfReal(1, poles);
          weights.Init(1.0);
          break;
        }
    }
    const int knots = profiler->NbKnots();
    TColStd_Array1OfReal uknots(1, knots);
    TColStd_Array1OfInteger umults(1, knots);
    profiler->KnotsAndMults(uknots, umults);
    std::vector<TColgp_Array1OfPnt> rows;
    for (int i = 0; i < n; ++i) {
      rows.emplace_back(1, poles);
      profiler->Poles(i + 1, rows.back());
    }
    std::vector<Handle(Geom_BSplineCurve)> columns;
    for (int k = 1; k <= poles; ++k) {
      Handle(TColgp_HArray1OfPnt) points = new TColgp_HArray1OfPnt(1, n);
      for (int i = 0; i < n; ++i) points->SetValue(i + 1, rows[i](k));
      GeomAPI_Interpolate interpolate(points, params, false, tol);
      if (start || end) {
        // A free end takes the tangent of the parabola through its nearest
        // three conditions, so both ends are always loaded and every pole row
        // gets the same knot vector.
        auto P = [&](int i) { return points->Value(i + 1).XYZ(); };
        auto v = [&](int i) { return params->Value(i + 1); };
        auto free_end = [&](int i, int j, int k, const gp_Vec *known) -> gp_Vec {
          // Tangent at i of the quadratic through i, j (and k, or the known tangent at j).
          const double h = v(j) - v(i);
          if (known) return gp_Vec(2 * (P(j) - P(i)) / h) - *known;
          const double h2 = v(k) - v(i);
          const gp_XYZ d1 = (P(j) - P(i)) / h, d2 = (P(k) - P(i)) / h2;
          return gp_Vec((d1 * h2 - d2 * h) / (h2 - h));
        };
        gp_Vec a = t0, b = t1;
        if (!start) a = n > 2 ? free_end(0, 1, 2, nullptr) : free_end(0, 1, 1, &b);
        if (!end) b = n > 2 ? free_end(n - 1, n - 2, n - 3, nullptr) : free_end(1, 0, 0, &a);
        interpolate.Load(a, b, false);
      }
      interpolate.Perform();
      if (!interpolate.IsDone()) throw std::runtime_error("could not interpolate the loft sections");
      columns.push_back(interpolate.Curve());
    }
    const Handle(Geom_BSplineCurve) &c = columns[0];
    TColgp_Array2OfPnt net(1, poles, 1, c->NbPoles());
    TColStd_Array2OfReal net_weights(1, poles, 1, c->NbPoles());
    for (int k = 1; k <= poles; ++k) {
      if (columns[k - 1]->NbPoles() != c->NbPoles())
        throw std::runtime_error("loft pole rows are incompatible");
      for (int l = 1; l <= c->NbPoles(); ++l) {
        net(k, l) = columns[k - 1]->Pole(l);
        net_weights(k, l) = weights(k);
      }
    }
    Handle(Geom_BSplineSurface) surface =
        new Geom_BSplineSurface(net, net_weights, uknots, c->Knots(), umults, c->Multiplicities(),
                                profiler->Degree(), c->Degree());
    TopoDS_Face face = BRepBuilderAPI_MakeFace(surface, Precision::Confusion()).Face();
    sewing.Add(face);
    sides.push_back(face);
  }
  std::vector<TopoDS_Face> caps;
  for (int i : {0, n - 1}) {
    plane_of(wires[i], tol, "the first and last loft sections must be planar to cap them");
    BRepBuilderAPI_MakeFace cap(wires[i], true);
    if (!cap.IsDone()) throw std::runtime_error("could not cap a loft end section");
    caps.push_back(cap.Face());
    sewing.Add(cap.Face());
  }
  Skin out;
  out.solid = close_solid(sewing, "the loft");
  auto sewn = [&](const TopoDS_Face &face) -> TopoDS_Shape {
    return sewing.IsModified(face) ? sewing.Modified(face) : face;
  };
  out.first = edges[0];
  for (const auto &face : sides) out.sides.push_back(sewn(face));
  out.caps[0] = sewn(caps[0]);
  out.caps[1] = sewn(caps[1]);
  return out;
}

// Record side faces as generated from `edges` (indices into `input`) and the caps.
void record_skin(const Skin &skin, const TopoDS_Shape &input, const std::vector<TopoDS_Edge> &edges,
                 rust::Vec<HistoryEntry> &hist) {
  TopTools_IndexedMapOfShape faces, input_edges;
  TopExp::MapShapes(skin.solid, TopAbs_FACE, faces);
  TopExp::MapShapes(input, TopAbs_EDGE, input_edges);
  auto record = [&](const TopoDS_Shape &face, uint8_t kind, uint8_t rel, int from) {
    const int index = faces.FindIndex(face);
    if (index > 0 && from > 0)
      hist.push_back(HistoryEntry{0, kind, static_cast<uint32_t>(from - 1), rel, 0, static_cast<uint32_t>(index - 1)});
  };
  for (size_t j = 0; j < skin.sides.size() && j < edges.size(); ++j)
    record(skin.sides[j], 1, GENERATED, input_edges.FindIndex(edges[j]));
  record(skin.caps[0], 0, FIRST_CAP, 1);
  record(skin.caps[1], 0, LAST_CAP, 1);
}
} // namespace

std::unique_ptr<Shape> skin(const Shape &sections, bool preserve, uint8_t start_mode, const Vec3 &start_dir,
                            double start_weight, uint8_t end_mode, const Vec3 &end_dir, double end_weight,
                            rust::Vec<HistoryEntry> &hist) {
  const double tol = 1e-7 * scale_of(sections.shape);
  if (start_mode > DIRECTION || end_mode > DIRECTION) throw std::invalid_argument("unknown loft end condition");
  const EndCondition a{start_mode, gp_Vec(start_dir.x, start_dir.y, start_dir.z), start_weight};
  const EndCondition b{end_mode, gp_Vec(end_dir.x, end_dir.y, end_dir.z), end_weight};
  const Skin out = skin_wires(wires_in(sections.shape), preserve, a, b, tol);
  record_skin(out, sections.shape, out.first, hist);
  return std::make_unique<Shape>(out.solid);
}

std::unique_ptr<Shape> loft_centerline(const Shape &sections, const Shape &centerline, rust::Vec<HistoryEntry> &hist) {
  const std::vector<TopoDS_Wire> wires = wires_in(sections.shape);
  const int n = static_cast<int>(wires.size());
  if (n < 2) throw std::invalid_argument("a loft needs at least two sections");
  TopExp_Explorer spine_wire(centerline.shape, TopAbs_WIRE);
  if (!spine_wire.More()) throw std::invalid_argument("the centerline must be a connected curve");
  const double scale = scale_of(sections.shape);
  const double tol = 1e-6 * scale;

  // One spline for the whole centerline, so it can be trimmed to the sections.
  Handle(Geom_BSplineCurve) spine = joined(TopoDS::Wire(spine_wire.Current()), tol, "the centerline");

  // Where the centerline crosses each section's plane.
  std::vector<double> at;
  const double a = spine->FirstParameter(), b = spine->LastParameter();
  const int samples = 400;
  for (int i = 0; i < n; ++i) {
    const gp_Pln plane = plane_of(wires[i], tol, "centerline loft sections must be planar");
    auto side = [&](double t) { return gp_Vec(plane.Location(), spine->Value(t)).Dot(gp_Vec(plane.Axis().Direction())); };
    const gp_Pnt c = centroid(wires[i]);
    double best = 0, dist = 1e300;
    bool found = false;
    for (int s = 0; s < samples; ++s) {
      double lo = a + (b - a) * s / samples, hi = a + (b - a) * (s + 1) / samples;
      double flo = side(lo), fhi = side(hi);
      if (std::abs(flo) <= tol && s == 0) fhi = flo, hi = lo;
      if (flo * fhi > 0 && std::abs(fhi) > tol) continue;
      for (int it = 0; it < 100 && std::abs(hi - lo) > 1e-14 * (b - a); ++it) {
        const double mid = 0.5 * (lo + hi);
        if (side(lo) * side(mid) <= 0) hi = mid;
        else lo = mid;
      }
      const double t = 0.5 * (lo + hi);
      const double d = spine->Value(t).Distance(c);
      if (d < dist) dist = d, best = t, found = true;
    }
    if (!found)
      throw std::invalid_argument("the centerline must cross the plane of section " + std::to_string(i + 1));
    at.push_back(best);
  }
  for (int i = 1; i < n; ++i)
    if (!(at[i] > at[i - 1] + 1e-9 * (b - a)))
      throw std::invalid_argument("the sections must lie in order along the centerline");
  spine->Segment(at.front(), at.back());
  TopoDS_Wire path = BRepBuilderAPI_MakeWire(BRepBuilderAPI_MakeEdge(spine)).Wire();

  BRepOffsetAPI_MakePipeShell op(path);
  op.SetMode(false);
  op.SetTransitionMode(BRepBuilderAPI_RightCorner);
  for (int i = 0; i < n; ++i) {
    TopoDS_Vertex where = BRepBuilderAPI_MakeVertex(spine->Value(at[i]));
    op.Add(wires[i], where, false, false);
  }
  if (!op.IsReady()) throw std::invalid_argument("the centerline loft sections are incompatible");
  op.Build();
  if (!op.IsDone() || !op.MakeSolid()) throw std::runtime_error("centerline loft failed; check the sections and centerline");
  TopoDS_Shape out = op.Shape();
  TopExp_Explorer solids(out, TopAbs_SOLID);
  if (!solids.More()) throw std::runtime_error("the centerline loft does not form a solid");
  TopoDS_Solid solid = TopoDS::Solid(solids.Current());
  if (!BRepCheck_Analyzer(solid).IsValid()) throw std::runtime_error("the centerline loft is invalid");
  BOPAlgo_ArgumentAnalyzer self_check;
  self_check.SetShape1(solid);
  self_check.SelfInterMode() = true;
  self_check.Perform();
  if (self_check.HasFaulty())
    throw std::runtime_error("the centerline loft intersects itself; reduce the sections or the centerline's bend");

  TopTools_IndexedMapOfShape faces, input_edges;
  TopExp::MapShapes(solid, TopAbs_FACE, faces);
  TopExp::MapShapes(sections.shape, TopAbs_EDGE, input_edges);
  for (int i = 1; i <= input_edges.Extent(); ++i)
    for (const TopoDS_Shape &g : op.Generated(input_edges(i))) {
      const int index = faces.FindIndex(g);
      if (index > 0) hist.push_back(HistoryEntry{0, 1, static_cast<uint32_t>(i - 1), GENERATED, 0, static_cast<uint32_t>(index - 1)});
    }
  auto cap = [&](const TopoDS_Shape &end, uint8_t rel) {
    for (TopExp_Explorer f(end, TopAbs_FACE); f.More(); f.Next()) {
      const int index = faces.FindIndex(f.Current());
      if (index > 0) hist.push_back(HistoryEntry{0, 0, 0, rel, 0, static_cast<uint32_t>(index - 1)});
    }
  };
  cap(op.FirstShape(), FIRST_CAP);
  cap(op.LastShape(), LAST_CAP);
  return std::make_unique<Shape>(solid);
}

// A sweep whose section follows one or two guide curves exactly at stations
// along a tangent-continuous path. Each station's section is the profile
// carried by a rotation-minimizing frame and transformed in its plane so the
// points where the guides cross the plane keep their positions relative to the
// section: one guide rotates and (with `scale`) uniformly scales it, two guides
// stretch it. The sections are skinned, and stations double until sampled
// profile points between stations lie within tolerance of the surface.
std::unique_ptr<Shape> sweep_scaled(const Shape &profile, const Shape &path, const Shape &guide1,
                                    const Shape &guide2, bool scale_section, rust::Vec<HistoryEntry> &hist) {
  const TopoDS_Wire section = first_wire(profile.shape, "the sweep profile");
  if (!BRep_Tool::IsClosed(section)) throw std::invalid_argument("sweep requires a closed section");
  if (guide1.shape.IsNull()) throw std::invalid_argument("select a guide curve");
  Bnd_Box box;
  BRepBndLib::Add(section, box);
  BRepBndLib::Add(path.shape, box);
  BRepBndLib::Add(guide1.shape, box);
  if (!guide2.shape.IsNull()) BRepBndLib::Add(guide2.shape, box);
  const double size = std::max(1.0, std::sqrt(box.SquareExtent()));
  const double tol = 1e-7 * size;

  Handle(Geom_BSplineCurve) spine = joined(first_wire(path.shape, "the sweep path"), tol, "the sweep path");
  if (spine->StartPoint().Distance(spine->EndPoint()) < 10 * tol)
    throw std::invalid_argument("guide curves that scale the section need an open path");
  for (int i = 2; i < spine->NbKnots(); ++i) {
    if (spine->Multiplicity(i) < spine->Degree()) continue;
    gp_Pnt p;
    gp_Vec left, right;
    spine->LocalD1(spine->Knot(i), i - 1, i, p, left);
    spine->LocalD1(spine->Knot(i), i, i + 1, p, right);
    if (left.Magnitude() > 0 && right.Magnitude() > 0 && left.Angle(right) > 1e-4)
      throw std::invalid_argument("guide curves that scale the section need a tangent-continuous path");
  }
  std::vector<Handle(Geom_BSplineCurve)> guides{
      joined(first_wire(guide1.shape, "the guide curve"), tol, "the guide curve")};
  if (!guide2.shape.IsNull())
    guides.push_back(joined(first_wire(guide2.shape, "the second guide curve"), tol, "the second guide curve"));

  // Path samples with a rotation-minimizing frame (double reflection).
  const int K = 1024;
  const double a = spine->FirstParameter(), b = spine->LastParameter();
  std::vector<gp_Pnt> S(K + 1);
  std::vector<gp_Vec> T(K + 1), X(K + 1);
  for (int i = 0; i <= K; ++i) {
    gp_Vec d;
    spine->D1(a + (b - a) * i / K, S[i], d);
    if (d.Magnitude() < 1e-12) throw std::invalid_argument("the sweep path has a point with no direction");
    T[i] = d.Normalized();
  }

  // Where guide g crosses the plane normal to the path at sample i, from guide
  // parameter u (Newton's method on the signed distance to the plane).
  auto crossing = [&](const Handle(Geom_BSplineCurve) &g, int i, double &u) -> bool {
    const double lo = g->FirstParameter(), hi = g->LastParameter();
    for (int it = 0; it < 60; ++it) {
      gp_Pnt p;
      gp_Vec d;
      g->D1(u, p, d);
      const double f = gp_Vec(S[i], p).Dot(T[i]), df = d.Dot(T[i]);
      if (std::abs(f) < 1e-12 * size) return true;
      if (std::abs(df) < 1e-14) return false;
      u = std::min(hi, std::max(lo, u - f / df));
    }
    gp_Pnt p = g->Value(u);
    return std::abs(gp_Vec(S[i], p).Dot(T[i])) < 1e-9 * size;
  };
  std::vector<std::vector<gp_Pnt>> G(guides.size(), std::vector<gp_Pnt>(K + 1));
  for (size_t g = 0; g < guides.size(); ++g) {
    const auto &curve = guides[g];
    // The first crossing: the sign change of the plane distance nearest the path start.
    const int n = 512;
    const double lo = curve->FirstParameter(), hi = curve->LastParameter();
    double best = 0, dist = 1e300;
    bool found = false;
    auto f = [&](double u) { return gp_Vec(S[0], curve->Value(u)).Dot(T[0]); };
    for (int k = 0; k < n; ++k) {
      double u0 = lo + (hi - lo) * k / n, u1 = lo + (hi - lo) * (k + 1) / n;
      if (f(u0) * f(u1) > 0) continue;
      for (int it = 0; it < 80; ++it) {
        const double mid = 0.5 * (u0 + u1);
        if (f(u0) * f(mid) <= 0) u1 = mid;
        else u0 = mid;
      }
      const double d = curve->Value(u0).Distance(S[0]);
      if (d < dist) dist = d, best = u0, found = true;
    }
    if (!found) throw std::invalid_argument("a guide curve must cross the profile's plane at the path start");
    double u = best;
    for (int i = 0; i <= K; ++i) {
      const gp_Pnt last = i ? G[g][i - 1] : curve->Value(u);
      if (!crossing(curve, i, u))
        throw std::invalid_argument("a guide curve must cross every plane normal to the path along its whole length");
      G[g][i] = curve->Value(u);
      if (i && G[g][i].Distance(last) > 0.2 * size)
        throw std::invalid_argument("a guide curve crosses a plane normal to the path more than once");
    }
  }

  // The frame's x axis starts toward the first guide and is carried along.
  gp_Vec x0 = gp_Vec(S[0], G[0][0]);
  x0 -= T[0] * x0.Dot(T[0]);
  if (x0.Magnitude() < 1e3 * tol) throw std::invalid_argument("the guide curve must start away from the path");
  X[0] = x0.Normalized();
  for (int i = 0; i < K; ++i) {
    const gp_Vec v1(S[i], S[i + 1]);
    const double c1 = v1.Dot(v1);
    gp_Vec r = X[i], t = T[i];
    if (c1 > 1e-30) {
      r -= v1 * (2 / c1 * v1.Dot(r));
      t -= v1 * (2 / c1 * v1.Dot(t));
    }
    const gp_Vec v2 = T[i + 1] - t;
    const double c2 = v2.Dot(v2);
    if (c2 > 1e-30) r -= v2 * (2 / c2 * v2.Dot(r));
    r -= T[i + 1] * r.Dot(T[i + 1]);
    X[i + 1] = r.Normalized();
  }

  // The in-plane map at sample i, in frame coordinates.
  auto local = [&](size_t g, int i) {
    const gp_Vec d(S[i], G[g][i]);
    const gp_Vec y = T[i].Crossed(X[i]);
    return gp_XY(d.Dot(X[i]), d.Dot(y));
  };
  auto map2d = [&](int i) {
    gp_Mat2d m;
    const gp_XY g0 = local(0, 0), gi = local(0, i);
    if (guides.size() == 1) {
      const double angle = std::atan2(gi.Y(), gi.X()) - std::atan2(g0.Y(), g0.X());
      const double s = scale_section ? gi.Modulus() / g0.Modulus() : 1.0;
      m.SetRotation(angle);
      m.Multiply(s);
      return m;
    }
    const gp_XY h0 = local(1, 0), hi = local(1, i);
    const double det0 = g0.X() * h0.Y() - g0.Y() * h0.X();
    if (std::abs(det0) < 1e-6 * g0.Modulus() * h0.Modulus())
      throw std::invalid_argument("the two guide curves must not line up with the path");
    gp_Mat2d before(gp_XY(g0.X(), g0.Y()), gp_XY(h0.X(), h0.Y()));  // columns
    gp_Mat2d after(gp_XY(gi.X(), gi.Y()), gp_XY(hi.X(), hi.Y()));
    m = after * before.Inverted();
    if (m.Determinant() <= 1e-9) throw std::invalid_argument("the guide curves fold the section; check their positions");
    return m;
  };
  auto transform = [&](int i) {
    const gp_Vec y0 = T[0].Crossed(X[0]), yi = T[i].Crossed(X[i]);
    const gp_Mat2d m = map2d(i);
    // world = S_i + F_i * L * F_0^T * (p - S_0), F = [x y t].
    gp_Mat F0(X[0].XYZ(), y0.XYZ(), T[0].XYZ()), Fi(X[i].XYZ(), yi.XYZ(), T[i].XYZ());
    gp_Mat L(gp_XYZ(m.Value(1, 1), m.Value(2, 1), 0), gp_XYZ(m.Value(1, 2), m.Value(2, 2), 0), gp_XYZ(0, 0, 1));
    gp_Mat F0t = F0.Transposed();
    gp_Mat M = Fi * L * F0t;
    gp_GTrsf g;
    g.SetVectorialPart(M);
    g.SetTranslationPart(S[i].XYZ() - M * S[0].XYZ());
    return g;
  };
  auto section_at = [&](int i) {
    BRepBuilderAPI_GTransform copy(section, transform(i), true);
    return TopoDS::Wire(copy.Shape());
  };

  // Profile samples whose transformed positions check the surface between stations.
  std::vector<gp_Pnt> samples;
  for (BRepTools_WireExplorer e(section); e.More(); e.Next()) {
    double f, l;
    Handle(Geom_Curve) c = BRep_Tool::Curve(e.Current(), f, l);
    for (int k = 0; k < 3; ++k) samples.push_back(c->Value(f + (l - f) * (k + 0.5) / 3));
  }
  const double check = 1e-5 * size;
  for (int stations = 8; stations <= 256; stations *= 2) {
    std::vector<TopoDS_Wire> wires;
    for (int k = 0; k <= stations; ++k) wires.push_back(section_at(k * K / stations));
    const Skin out = skin_wires(wires, true, EndCondition{}, EndCondition{}, tol);
    TopoDS_Compound sides;
    BRep_Builder builder;
    builder.MakeCompound(sides);
    for (const auto &f : out.sides) builder.Add(sides, f);
    double worst = 0;
    for (int k = 0; k < stations && worst <= check; ++k) {
      const gp_GTrsf g = transform((2 * k + 1) * K / (2 * stations));
      for (const auto &p : samples) {
        gp_XYZ q = p.XYZ();
        g.Transforms(q);
        BRepExtrema_DistShapeShape dist(BRepBuilderAPI_MakeVertex(gp_Pnt(q)), sides);
        if (!dist.IsDone()) throw std::runtime_error("could not measure the guided sweep");
        worst = std::max(worst, dist.Value());
        if (worst > check) break;
      }
    }
    if (worst <= check) {
      record_skin(out, profile.shape, ordered_edges(section), hist);
      return std::make_unique<Shape>(out.solid);
    }
  }
  throw std::runtime_error("the guided sweep needs too many sections; smooth the path or guide curves");
}
} // namespace ofk

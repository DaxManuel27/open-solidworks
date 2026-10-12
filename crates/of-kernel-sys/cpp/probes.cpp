// Bounded R0 experiments. No feature dispatcher or document can call these.
#include "of-kernel-sys/cpp/probes.h"
#include "of-kernel-sys/src/probes.rs.h"
#include "of-kernel-sys/src/lib.rs.h"

#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepGProp.hxx>
#include <BRepLib.hxx>
#include <BRep_Tool.hxx>
#include <BRep_Builder.hxx>
#include <BRepOffsetAPI_MakePipeShell.hxx>
#include <BRepOffsetAPI_ThruSections.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakeSphere.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <GeomConvert.hxx>
#include <GeomFill_BSplineCurves.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_BezierCurve.hxx>
#include <GProp_GProps.hxx>
#include <Standard_Version.hxx>
#include <TColgp_Array1OfPnt.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopoDS.hxx>
#include <gp_Circ.hxx>
#include <gp_Pln.hxx>

#include <algorithm>
#include <array>
#include <chrono>
#include <cmath>
#include <stdexcept>
#include <vector>

namespace ofk::probes {
namespace {
constexpr double PI = 3.14159265358979323846;

void require(bool condition, const char *message) {
  if (!condition) throw std::runtime_error(message);
}

double volume(const TopoDS_Shape &shape) {
  GProp_GProps props;
  BRepGProp::VolumeProperties(shape, props);
  return props.Mass();
}

uint32_t count_shapes(const TopoDS_Shape &shape, TopAbs_ShapeEnum kind) {
  TopTools_IndexedMapOfShape map;
  TopExp::MapShapes(shape, kind, map);
  return map.Extent();
}

double gap(const gp_Pnt &point, const TopoDS_Shape &shape) {
  // Distance to a solid is zero for an interior point. Measure the skin so an
  // inward guide-fitting error cannot pass just because the point is inside.
  TopoDS_Compound skin;
  BRep_Builder builder;
  builder.MakeCompound(skin);
  for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next())
    builder.Add(skin, it.Current());
  BRepExtrema_DistShapeShape distance(BRepBuilderAPI_MakeVertex(point).Shape(), skin);
  require(distance.IsDone() && distance.NbSolution() > 0, "distance measurement failed");
  return distance.Value();
}

gp_Dir cylinder_normal(const TopoDS_Shape &shape, const gp_Pnt &point, double tolerance) {
  for (TopExp_Explorer it(shape, TopAbs_FACE); it.More(); it.Next()) {
    const auto face = TopoDS::Face(it.Current());
    if (BRepAdaptor_Surface(face).GetType() != GeomAbs_Cylinder) continue;
    auto surface = BRep_Tool::Surface(face);
    GeomAPI_ProjectPointOnSurf projection(point, surface);
    if (projection.NbPoints() == 0 || projection.LowerDistance() > tolerance) continue;
    double u, v;
    projection.LowerDistanceParameters(u, v);
    gp_Pnt p;
    gp_Vec du, dv;
    surface->D1(u, v, p, du, dv);
    return gp_Dir(du.Crossed(dv));
  }
  throw std::runtime_error("blend contact point has no cylindrical surface");
}

Handle(Geom_BSplineCurve) curve(const gp_Pnt &a, const gp_Pnt &b, double lift) {
  TColgp_Array1OfPnt poles(1, 4);
  poles(1) = a;
  poles(2) = gp_Pnt(a.XYZ() * (2.0 / 3.0) + b.XYZ() / 3.0 + gp_XYZ(0, 0, lift));
  poles(3) = gp_Pnt(a.XYZ() / 3.0 + b.XYZ() * (2.0 / 3.0) + gp_XYZ(0, 0, lift));
  poles(4) = b;
  return GeomConvert::CurveToBSplineCurve(new Geom_BezierCurve(poles));
}

TopoDS_Wire rectangle(double x, double y, double z, double half_x, double half_y) {
  BRepBuilderAPI_MakePolygon wire;
  wire.Add(gp_Pnt(x - half_x, y - half_y, z));
  wire.Add(gp_Pnt(x + half_x, y - half_y, z));
  wire.Add(gp_Pnt(x + half_x, y + half_y, z));
  wire.Add(gp_Pnt(x - half_x, y + half_y, z));
  wire.Close();
  return wire.Wire();
}

struct Trial {
  TopoDS_Shape shape;
  // Source faces are construction faces, not a claim of persistent model naming.
  std::vector<TopoDS_Shape> source_faces;
  double expected = 0;
  bool analytic = false;
  double max_gap = 0;
  double normal_error = 0;
  bool normal_checked = false;
  gp_Pnt cut_center;
};

Trial boundary(uint8_t variant, double s) {
  Trial t;
  const std::array<gp_Pnt, 4> top = {gp_Pnt(0, 0, 4*s), gp_Pnt(10*s, 0, 4*s),
                                  gp_Pnt(10*s, 8*s, 4*s), gp_Pnt(0, 8*s, 4*s)};
  std::array<Handle(Geom_BSplineCurve), 4> edges;
  for (int i = 0; i < 4; ++i)
    edges[i] = curve(top[i], top[(i+1)%4],
                     (variant == 1 && i%2 == 0) || (variant == 2 && i%2 == 1) ? s : 0);
  if (variant == 3) edges[0] = curve(gp_Pnt(-s, 0, 4*s), top[1], 0);
  for (int i = 0; i < 4; ++i) {
    const auto &next = edges[(i+1)%4];
    require(edges[i]->Value(edges[i]->LastParameter()).Distance(next->Value(next->FirstParameter())) <= 1e-6*s,
            "boundary curves must meet at every corner");
  }
  GeomFill_BSplineCurves fill(edges[0], edges[1], edges[2], edges[3], GeomFill_CoonsStyle);
  auto surface = fill.Surface();
  require(!surface.IsNull(), "four-boundary fill failed");
  auto top_face = BRepBuilderAPI_MakeFace(surface, 1e-7*s).Face();
  // Measure the original boundary curves against the computed surface, including
  // the independently perturbed U and V curve families.
  for (const auto &edge : edges)
    for (int j = 0; j <= 20; ++j) {
      gp_Pnt p = edge->Value(edge->FirstParameter() +
                            (edge->LastParameter()-edge->FirstParameter())*j/20.0);
      GeomAPI_ProjectPointOnSurf projection(p, surface);
      require(projection.NbPoints() > 0, "boundary projection measurement failed");
      t.max_gap = std::max(t.max_gap, projection.LowerDistance());
    }
  BRepBuilderAPI_Sewing sewing(1e-6*s);
  sewing.Add(top_face);
  t.source_faces.push_back(top_face);
  for (int i = 0; i < 4; ++i) {
    const int next = (i+1)%4;
    gp_Pnt a(top[i].X(), top[i].Y(), 0), b(top[next].X(), top[next].Y(), 0);
    BRepBuilderAPI_MakeWire wire;
    wire.Add(BRepBuilderAPI_MakeEdge(edges[i]).Edge());
    wire.Add(BRepBuilderAPI_MakeEdge(top[next], b).Edge());
    wire.Add(BRepBuilderAPI_MakeEdge(b, a).Edge());
    wire.Add(BRepBuilderAPI_MakeEdge(a, top[i]).Edge());
    auto face = BRepBuilderAPI_MakeFace(wire.Wire(), true).Face();
    sewing.Add(face);
    t.source_faces.push_back(face);
  }
  auto bottom = BRepBuilderAPI_MakeFace(rectangle(5*s, 4*s, 0, 5*s, 4*s)).Face();
  sewing.Add(bottom);
  t.source_faces.push_back(bottom);
  sewing.Perform();
  auto sewn = sewing.SewedShape();
  require(sewn.ShapeType() == TopAbs_SHELL, "boundary sewing did not produce one shell");
  auto solid = BRepBuilderAPI_MakeSolid(TopoDS::Shell(sewn)).Solid();
  require(BRepLib::OrientClosedSolid(solid), "boundary shell was not closed");
  t.shape = solid;
  for (auto &face : t.source_faces)
    if (sewing.IsModifiedSubShape(face)) face = sewing.ModifiedSubShape(face);
  // Each raised pair contributes height 3*s*u*(1-u), whose mean is s/2.
  t.expected = (variant == 0 ? 320 : 360)*s*s*s;
  t.analytic = true;
  t.cut_center = gp_Pnt(5*s, 4*s, 0);
  return t;
}

Trial planar_blend(uint8_t variant, double s) {
  Trial t;
  // Analytic rolling-circle solution between the support planes x=0 and y=0.
  // Trim against both planes and fuse into the remaining prism. This does not
  // call the edge-fillet builder and does not solve arbitrary face-set blends.
  const double r = (variant == 3 ? 12 : 2 + variant)*s, height = 8*s;
  require(r < 10*s, "rolling circle does not fit within the support faces");
  auto block = BRepPrimAPI_MakeBox(10*s, 10*s, height).Shape();
  auto corner = BRepPrimAPI_MakeBox(r, r, height).Shape();
  auto roll = BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(r, r, 0), gp::DZ()), r, height).Shape();
  auto trim = BRepAlgoAPI_Common(corner, roll).Shape();
  auto remainder = BRepAlgoAPI_Cut(block, corner).Shape();
  t.shape = BRepAlgoAPI_Fuse(remainder, trim).Shape();
  for (TopExp_Explorer it(block, TopAbs_FACE); it.More(); it.Next())
    t.source_faces.push_back(it.Current());
  for (double z : {0.25*height, 0.5*height, 0.75*height}) {
    t.max_gap = std::max(t.max_gap, gap(gp_Pnt(0, r, z), t.shape));
    t.max_gap = std::max(t.max_gap, gap(gp_Pnt(r, 0, z), t.shape));
  }
  // Cylinder and support-plane normals agree at both contact generators.
  gp_Dir nx = cylinder_normal(t.shape, gp_Pnt(0, r, height/2), 1e-6*s);
  gp_Dir ny = cylinder_normal(t.shape, gp_Pnt(r, 0, height/2), 1e-6*s);
  t.normal_error = std::max(std::acos(std::clamp(std::abs(nx.Dot(gp::DX())), 0.0, 1.0)),
                            std::acos(std::clamp(std::abs(ny.Dot(gp::DY())), 0.0, 1.0)));
  t.normal_checked = true;
  t.expected = (100*s*s - r*r*(1-PI/4))*height;
  t.analytic = true;
  t.cut_center = gp_Pnt(5*s, 5*s, 0);
  return t;
}

Trial dome(uint8_t variant, double s) {
  Trial t;
  const double radius = 10*s, h = (variant == 3 ? 0 : 3+variant)*s;
  require(h > 0 && h <= radius, "spherical-cap height must be positive and at most the base radius");
  const double sphere_radius = (radius*radius+h*h)/(2*h);
  auto sphere = BRepPrimAPI_MakeSphere(gp_Pnt(0, 0, h-sphere_radius), sphere_radius).Shape();
  auto half = BRepPrimAPI_MakeBox(gp_Pnt(-11*s, -11*s, 0), 22*s, 22*s, h+s).Shape();
  auto cap = BRepAlgoAPI_Common(sphere, half).Shape();
  auto base = BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, -2*s), gp::DZ()), radius, 2*s).Shape();
  t.shape = BRepAlgoAPI_Fuse(base, cap).Shape();
  for (TopExp_Explorer it(base, TopAbs_FACE); it.More(); it.Next())
    t.source_faces.push_back(it.Current());
  t.max_gap = gap(gp_Pnt(0, 0, h), t.shape);
  for (int i = 0; i < 32; ++i) {
    double a = 2*PI*i/32;
    t.max_gap = std::max(t.max_gap, gap(gp_Pnt(radius*std::cos(a), radius*std::sin(a), 0), t.shape));
  }
  t.expected = PI*radius*radius*2*s + PI*h*(3*radius*radius+h*h)/6;
  t.analytic = true;
  t.cut_center = gp_Pnt(0, 0, 0);
  return t;
}

Trial wrap(uint8_t variant, double s) {
  Trial t;
  const double r = 10*s, depth = s, angle = variant == 3 ? 2*PI+0.1 : 0.8;
  require(angle > 0 && angle < 2*PI, "wrapped interval must not cover the target more than once");
  const double start = variant == 1 ? 2*PI-0.2 : (variant == 2 ? -0.4 : 0.2);
  auto base = BRepPrimAPI_MakeCylinder(r, 8*s).Shape();
  auto outer = BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, 2*s), gp::DZ()), r+depth, 4*s, angle).Shape();
  auto inner = BRepPrimAPI_MakeCylinder(r, 8*s).Shape();
  auto band = BRepAlgoAPI_Cut(outer, inner).Shape();
  gp_Trsf rotate;
  rotate.SetRotation(gp_Ax1(gp::Origin(), gp::DZ()), start);
  band = BRepBuilderAPI_Transform(band, rotate, true).Shape();
  t.shape = BRepAlgoAPI_Fuse(base, band).Shape();
  for (TopExp_Explorer it(base, TopAbs_FACE); it.More(); it.Next())
    t.source_faces.push_back(it.Current());
  // Exact analytic rectangular map: x/r is the angular coordinate, z is axial.
  for (int i = 0; i <= 20; ++i) {
    double a = start + angle*i/20;
    t.max_gap = std::max(t.max_gap, gap(gp_Pnt((r+depth)*std::cos(a), (r+depth)*std::sin(a), 4*s), t.shape));
  }
  t.expected = PI*r*r*8*s + angle*((r+depth)*(r+depth)-r*r)*4*s/2;
  t.analytic = true;
  t.cut_center = gp_Pnt(0, 0, 0);
  return t;
}

Trial guided_sweep(uint8_t variant, double s) {
  Trial t;
  double angle = variant == 3 ? PI : variant*PI/6;
  require(angle < PI-1e-8, "auxiliary guide crosses the spine and leaves the frame undefined");
  auto spine = BRepBuilderAPI_MakeWire(BRepBuilderAPI_MakeEdge(gp_Pnt(0, 0, 0), gp_Pnt(0, 0, 10*s)).Edge()).Wire();
  auto guide = BRepBuilderAPI_MakeWire(BRepBuilderAPI_MakeEdge(gp_Pnt(3*s, 0, 0), gp_Pnt(3*s*std::cos(angle), 3*s*std::sin(angle), 10*s)).Edge()).Wire();
  auto profile = rectangle(0, 0, 0, 2*s, s);
  BRepOffsetAPI_MakePipeShell pipe(spine);
  pipe.SetTolerance(1e-6*s, 1e-6*s, 1e-7);
  pipe.SetMode(guide, false, BRepFill_NoContact);
  pipe.Add(profile);
  pipe.Build();
  require(pipe.IsDone() && pipe.MakeSolid(), "auxiliary-guide sweep failed");
  t.shape = pipe.Shape();
  // At each z, the auxiliary guide determines the section X axis. Check its
  // four corners against the actual skin, rather than only trusting volume.
  for (int j = 0; j <= 20; ++j) {
    double u = j/20.0;
    double a = std::atan2(u*std::sin(angle), 1-u+u*std::cos(angle));
    for (double x : {-2*s, 2*s}) for (double y : {-s, s})
      t.max_gap = std::max(t.max_gap, gap(gp_Pnt(x*std::cos(a)-y*std::sin(a), x*std::sin(a)+y*std::cos(a), 10*s*u), t.shape));
  }
  t.source_faces = {pipe.FirstShape(), pipe.LastShape()};
  t.expected = 80*s*s*s;
  t.analytic = true;
  t.cut_center = gp_Pnt(0, 0, 0);
  return t;
}

Trial guided_sections(uint8_t variant, double s) {
  Trial t;
  // Candidate for guides: interpolate sections sampled along a quadratic guide.
  // Measure the unsampled points to expose approximation error explicitly.
  BRepOffsetAPI_ThruSections loft(true, false, 1e-6*s);
  const int intervals = variant == 2 ? 8 : 4;
  const double bend = variant == 0 ? 0 : 4*s;
  double previous = -1;
  for (int i = 0; i <= intervals; ++i) {
    double u = double(i)/intervals;
    if (variant == 3 && i == 2) u = previous;
    require(u > previous, "guide sections must have distinct, increasing path parameters");
    previous = u;
    loft.AddWire(rectangle(bend*u*u, 0, 10*s*u, 2*s, s));
  }
  loft.CheckCompatibility(true);
  loft.Build();
  require(loft.IsDone(), "guide-sampled section loft failed");
  t.shape = loft.Shape();
  t.source_faces = {loft.FirstShape(), loft.LastShape()};
  for (int i = 0; i <= 40; ++i) {
    double u = i/40.0;
    for (double x : {-2*s, 2*s}) for (double y : {-s, s})
      t.max_gap = std::max(t.max_gap, gap(gp_Pnt(bend*u*u+x, y, 10*s*u), t.shape));
  }
  t.expected = 80*s*s*s;
  t.analytic = true;
  t.cut_center = gp_Pnt(0, 0, 0);
  return t;
}
} // namespace

rust::String kernel_version() { return OCC_VERSION_COMPLETE; }

Measurement measure(uint8_t which, uint8_t variant, double scale) {
  if (which > 5 || variant > 3) throw std::invalid_argument("unknown bounded probe case or variant");
  if (!std::isfinite(scale) || scale < 0.01 || scale > 100)
    throw std::invalid_argument("probe scale must be finite and between 0.01 and 100");
  auto start = std::chrono::steady_clock::now();
  Trial t;
  switch (which) {
  case 0: t = boundary(variant, scale); break;
  case 1: t = planar_blend(variant, scale); break;
  case 2: t = dome(variant, scale); break;
  case 3: t = wrap(variant, scale); break;
  case 4: t = guided_sweep(variant, scale); break;
  case 5: t = guided_sections(variant, scale); break;
  }
  Measurement m{};
  m.valid = !t.shape.IsNull() && BRepCheck_Analyzer(t.shape).IsValid();
  m.solids = count_shapes(t.shape, TopAbs_SOLID);
  m.volume = volume(t.shape);
  m.expected_volume = t.expected;
  m.has_expected_volume = t.analytic;
  m.max_gap = t.max_gap;
  // Fixture length is about 10*scale mm: one part per million of that length.
  m.fit_tolerance = 1e-5*scale;
  m.fit_passed = t.max_gap <= m.fit_tolerance;
  m.normal_error = t.normal_error;
  m.has_normal_check = t.normal_checked;
  m.source_faces = t.source_faces.size();
  TopTools_IndexedMapOfShape faces;
  TopExp::MapShapes(t.shape, TopAbs_FACE, faces);
  for (const auto &face : t.source_faces)
    if (faces.Contains(face)) ++m.retained_source_faces;

  // Apply a real downstream through cut and retain its kernel history.
  auto cylinder = BRepPrimAPI_MakeCylinder(
      gp_Ax2(gp_Pnt(t.cut_center.X(), t.cut_center.Y(), -20*scale), gp::DZ()), 0.25*scale, 60*scale).Shape();
  BRepAlgoAPI_Cut cut(t.shape, cylinder);
  if (cut.IsDone() && !cut.Shape().IsNull()) {
    m.cut_valid = BRepCheck_Analyzer(cut.Shape()).IsValid();
    m.cut_volume = volume(cut.Shape());
    TopTools_IndexedMapOfShape output_faces;
    TopExp::MapShapes(cut.Shape(), TopAbs_FACE, output_faces);
    for (int i = 1; i <= faces.Extent(); ++i) {
      bool tracked = output_faces.Contains(faces(i));
      for (const auto &f : cut.Modified(faces(i))) tracked |= output_faces.Contains(f);
      if (tracked) ++m.cut_history_faces;
    }
  }
  // A downstream edge fillet is a consumer check, never the face-blend prototype.
  Shape shape(t.shape);
  const uint32_t edges = count_shapes(t.shape, TopAbs_EDGE);
  for (uint32_t i = 0; i < std::min(edges, uint32_t(16)); ++i) {
    try {
      rust::Vec<HistoryEntry> history;
      const uint32_t edge[] = {i};
      auto rounded = ofk::fillet(shape, rust::Slice<const uint32_t>(edge, 1), 0.1*scale, history);
      if (ofk::is_valid(*rounded) && ofk::count(*rounded, 3) == 1) {
        m.fillet_valid = true;
        break;
      }
    } catch (const Standard_Failure &) {
    } catch (const std::exception &) {
    }
  }
  m.elapsed_ms = std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now()-start).count();
  return m;
}
} // namespace ofk::probes

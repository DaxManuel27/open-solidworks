// Domes. On a circular disk the dome is exact: a spherical cap meets the disk at
// an angle; an elliptical cap (half an ellipsoid of revolution) meets it parallel
// to the disk normal, so it is tangent to a cylinder that the disk closes.
//
// On other planar faces without holes the dome is a smooth filling surface
// through the face's boundary and a point `height` above the face's centroid.
// The tangent variant also keeps the dome tangent to the faces beside the
// boundary, which needs a boundary without corners. The filling's measured
// boundary, apex and tangency errors must be within tolerance.
#include "ofk.h"
#include "of-kernel-sys/src/lib.rs.h"

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepGProp.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <BRepPrimAPI_MakeSphere.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepLib.hxx>
#include <BRepOffsetAPI_MakeFilling.hxx>
#include <BRepTools.hxx>
#include <BRepTools_ReShape.hxx>
#include <BOPAlgo_ArgumentAnalyzer.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRep_Tool.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <TopExp.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <TopoDS_Solid.hxx>
#include <TopoDS_Shell.hxx>
#include <BRep_Builder.hxx>
#include <ShapeFix_Face.hxx>
#include <Geom_Ellipse.hxx>
#include <GProp_GProps.hxx>
#include <Precision.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <BRepBndLib.hxx>
#include <Bnd_Box.hxx>
#include <ElCLib.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <gp_Circ.hxx>
#include <gp_Elips.hxx>
#include <gp_Pln.hxx>

#include <algorithm>
#include <cmath>
#include <stdexcept>
#include <vector>

namespace ofk {
namespace {
constexpr double PI = 3.14159265358979323846;

double volume(const TopoDS_Shape &shape) {
  GProp_GProps properties;
  BRepGProp::VolumeProperties(shape, properties);
  return properties.Mass();
}

// A complete planar circular disk face, with its outward (or inward) normal.
struct Disk {
  gp_Circ circle;
  double radius;
  double tolerance;
  gp_Dir normal;
};

Disk disk_of(const Shape &body, uint32_t face_index, double height, bool inward) {
  if (!std::isfinite(height) || height <= 0)
    throw std::invalid_argument("dome height must be positive and finite");
  const auto selected = sub_shape(body, 0, face_index);
  const TopoDS_Face face = TopoDS::Face(selected->shape);
  const BRepAdaptor_Surface surface(face);
  if (surface.GetType() != GeomAbs_Plane)
    throw std::invalid_argument("dome requires a planar circular disk face");
  unsigned wires = 0;
  for (TopExp_Explorer it(face, TopAbs_WIRE); it.More(); it.Next()) ++wires;
  if (wires != 1)
    throw std::invalid_argument("dome requires one circular boundary without holes");

  bool first = true;
  gp_Circ circle;
  double length = 0;
  for (TopExp_Explorer it(face, TopAbs_EDGE); it.More(); it.Next()) {
    BRepAdaptor_Curve curve(TopoDS::Edge(it.Current()));
    if (curve.GetType() != GeomAbs_Circle)
      throw std::invalid_argument("dome supports circular faces only; select a complete disk");
    const auto candidate = curve.Circle();
    if (first) { circle = candidate; first = false; }
    const double tolerance = std::max(10*Precision::Confusion(), circle.Radius()*1e-8);
    if (candidate.Location().Distance(circle.Location()) > tolerance ||
        std::abs(candidate.Radius()-circle.Radius()) > tolerance ||
        !candidate.Axis().Direction().IsParallel(circle.Axis().Direction(), 1e-8))
      throw std::invalid_argument("dome boundary arcs must belong to one circle");
    GProp_GProps properties;
    BRepGProp::LinearProperties(it.Current(), properties);
    length += properties.Mass();
  }
  if (first) throw std::invalid_argument("dome face has no circular boundary");
  const double radius = circle.Radius();
  const double tolerance = std::max(10*Precision::Confusion(), radius*1e-8);
  GProp_GProps properties;
  BRepGProp::SurfaceProperties(face, properties);
  if (std::abs(length - 2*PI*radius) > 10*tolerance ||
      std::abs(properties.Mass() - PI*radius*radius) > 10*tolerance*radius)
    throw std::invalid_argument("dome requires a complete circular disk without holes");
  if (height <= tolerance)
    throw std::invalid_argument("dome height must exceed the face tolerance");
  const auto info = face_info(body, face_index);
  gp_Dir normal(info.normal.x, info.normal.y, info.normal.z);
  if (inward) normal.Reverse();
  return Disk{circle, radius, tolerance, normal};
}

// Fuse or cut `cap` and verify that exactly its analytic volume changed.
std::unique_ptr<Shape> attach(const Shape &body, uint32_t face_index, const Disk &disk,
                              const TopoDS_Shape &cap_shape, double expected, bool inward,
                              rust::Vec<HistoryEntry> &history) {
  Shape cap(cap_shape);
  if (!is_valid(cap) || count(cap, 3) != 1)
    throw std::runtime_error("dome cap construction failed; change the height");
  const double volume_tolerance =
      std::max(expected*1e-6, disk.tolerance*disk.radius*disk.radius*10);
  if (std::abs(volume(cap.shape)-expected) > volume_tolerance)
    throw std::runtime_error("dome cap could not meet its analytic volume tolerance");
  rust::Vec<HistoryEntry> result_history;
  auto result = boolean(inward ? 1 : 0, body, cap, result_history);
  if (!is_valid(*result) || count(*result, 3) != count(body, 3))
    throw std::runtime_error("dome would split or join bodies; reduce height or change the face");
  const double delta = (volume(result->shape)-volume(body.shape)) * (inward ? -1 : 1);
  if (std::abs(delta-expected) > volume_tolerance)
    throw std::runtime_error(inward
      ? "inward dome exceeds the supporting material; reduce its height"
      : "outward dome intersects other material; reduce height or change the face");
  for (auto entry : result_history) {
    if (entry.input == 1) {
      if (entry.to_kind != 0) continue;
      // Newly introduced cap faces inherit authored ancestry from the selected
      // disk, not an internal cap face index that users cannot reference.
      entry.input = 0;
      entry.from_kind = 0;
      entry.from = face_index;
      entry.rel = 2; // generated
    }
    history.push_back(entry);
  }
  return result;
}

// Whether the face is a complete planar disk bounded by arcs of one circle.
bool circular_disk(const TopoDS_Face &face) {
  if (BRepAdaptor_Surface(face).GetType() != GeomAbs_Plane) return false;
  unsigned wires = 0;
  for (TopExp_Explorer it(face, TopAbs_WIRE); it.More(); it.Next()) ++wires;
  if (wires != 1) return false;
  for (TopExp_Explorer it(face, TopAbs_EDGE); it.More(); it.Next())
    if (BRepAdaptor_Curve(TopoDS::Edge(it.Current())).GetType() != GeomAbs_Circle) return false;
  return true;
}

std::unique_ptr<Shape> dome_filled(const Shape &body, uint32_t face_index, double height, bool inward,
                                   bool tangent, rust::Vec<HistoryEntry> &history) {
  if (!std::isfinite(height) || height <= 0)
    throw std::invalid_argument("dome height must be positive and finite");
  const auto selected = sub_shape(body, 0, face_index);
  const TopoDS_Face face = TopoDS::Face(selected->shape);
  if (BRepAdaptor_Surface(face).GetType() != GeomAbs_Plane)
    throw std::invalid_argument("dome requires a planar face");
  unsigned wires = 0;
  for (TopExp_Explorer it(face, TopAbs_WIRE); it.More(); it.Next()) ++wires;
  if (wires != 1) throw std::invalid_argument("dome requires a face without holes");
  const TopoDS_Wire outer = BRepTools::OuterWire(face);

  GProp_GProps area;
  BRepGProp::SurfaceProperties(face, area);
  const gp_Pnt centroid = area.CentreOfMass();
  Bnd_Box box;
  BRepBndLib::Add(face, box);
  const double size = std::sqrt(box.SquareExtent());
  const double tolerance = std::max(10 * Precision::Confusion(), size * 1e-5);
  if (height <= tolerance) throw std::invalid_argument("dome height must exceed the face tolerance");
  {
    GeomAPI_ProjectPointOnSurf foot(centroid, BRep_Tool::Surface(face));
    double u, v;
    foot.LowerDistanceParameters(u, v);
    if (BRepClass_FaceClassifier(face, gp_Pnt2d(u, v), tolerance).State() != TopAbs_IN)
      throw std::invalid_argument("dome needs a face whose centre lies inside it");
  }
  const auto info = face_info(body, face_index);
  gp_Dir normal(info.normal.x, info.normal.y, info.normal.z);
  if (inward) normal.Reverse();
  const gp_Pnt apex = centroid.Translated(gp_Vec(normal) * height);

  // The faces beside each boundary edge, for tangency.
  TopTools_IndexedDataMapOfShapeListOfShape sides;
  TopExp::MapShapesAndUniqueAncestors(body.shape, TopAbs_EDGE, TopAbs_FACE, sides);
  if (tangent) {
    // Tangency to each side face is only consistent where the boundary is smooth.
    std::vector<TopoDS_Edge> edges;
    for (BRepTools_WireExplorer e(outer); e.More(); e.Next()) edges.push_back(e.Current());
    for (size_t i = 0; i < edges.size(); ++i) {
      const TopoDS_Edge &a = edges[i], &b = edges[(i + 1) % edges.size()];
      BRepAdaptor_Curve ca(a), cb(b);
      gp_Pnt p;
      gp_Vec ta, tb;
      ca.D1(a.Orientation() == TopAbs_REVERSED ? ca.FirstParameter() : ca.LastParameter(), p, ta);
      cb.D1(b.Orientation() == TopAbs_REVERSED ? cb.LastParameter() : cb.FirstParameter(), p, tb);
      if (a.Orientation() == TopAbs_REVERSED) ta.Reverse();
      if (b.Orientation() == TopAbs_REVERSED) tb.Reverse();
      if (edges.size() > 1 && ta.Angle(tb) > 1e-3)
        throw std::invalid_argument("a tangent dome needs a face whose boundary has no corners");
    }
  }
  BRepOffsetAPI_MakeFilling fill(3, 40, 4, false, 1e-6, 0.1 * tolerance, 1e-4, 0.1, 10, 30);
  for (BRepTools_WireExplorer e(outer); e.More(); e.Next()) {
    const TopoDS_Edge &edge = e.Current();
    if (!tangent) {
      fill.Add(edge, GeomAbs_C0);
      continue;
    }
    TopoDS_Face side;
    if (const auto *faces = sides.Seek(edge))
      for (const TopoDS_Shape &f : *faces)
        if (!f.IsSame(face)) side = TopoDS::Face(f);
    if (side.IsNull()) throw std::invalid_argument("a tangent dome needs a face beside each boundary edge");
    fill.Add(edge, side, GeomAbs_G1);
  }
  fill.Add(apex);
  fill.Build();
  if (!fill.IsDone()) throw std::runtime_error("could not build the dome surface; change the height");
  if (fill.G0Error() > tolerance)
    throw std::runtime_error("the dome surface could not meet the face boundary within tolerance");
  if (tangent && fill.G1Error() > 1e-3)
    throw std::runtime_error("the dome surface could not stay tangent to the side faces");
  TopoDS_Face cap;
  for (TopExp_Explorer f(fill.Shape(), TopAbs_FACE); f.More(); f.Next()) cap = TopoDS::Face(f.Current());
  if (cap.IsNull()) throw std::runtime_error("the dome surface is empty");
  {
    BRepExtrema_DistShapeShape top(BRepBuilderAPI_MakeVertex(apex), cap);
    if (!top.IsDone() || top.Value() > tolerance)
      throw std::runtime_error("the dome surface misses its apex; change the height");
  }
  // Rebuild the dome on the filling surface with the face's own boundary
  // edges, so the cap shares them with the body exactly.
  auto bounded = [&](const TopoDS_Wire &wire) {
    BRepBuilderAPI_MakeFace rebuilt(BRep_Tool::Surface(cap), wire, true);
    if (!rebuilt.IsDone()) throw std::runtime_error("could not bound the dome surface by the face");
    ShapeFix_Face fix(rebuilt.Face());
    fix.SetPrecision(std::max(tolerance, 2 * fill.G0Error()));
    fix.Perform();
    TopoDS_Face out = fix.Face();
    BRepLib::SameParameter(out, std::max(tolerance, 2 * fill.G0Error()));
    return out;
  };
  // The wire must run counter-clockwise about the filling surface's normal.
  TopoDS_Face dome_face = bounded(outer);
  {
    GProp_GProps a;
    BRepGProp::SurfaceProperties(dome_face, a);
    if (a.Mass() < 0) dome_face = bounded(TopoDS::Wire(outer.Reversed()));
  }
  if (!BRepCheck_Analyzer(dome_face).IsValid()) throw std::runtime_error("the dome surface is invalid");
  // Pair the dome with the face so that each shared edge runs opposite ways in
  // the two faces, then orient the closed result outward.
  TopoDS_Solid solid;
  for (bool flip_dome : {false, true}) {
    for (bool flip_base : {true, false}) {
      TopoDS_Shell shell;
      BRep_Builder builder;
      builder.MakeShell(shell);
      builder.Add(shell, flip_dome ? dome_face.Reversed() : TopoDS_Shape(dome_face));
      builder.Add(shell, flip_base ? face.Reversed() : TopoDS_Shape(face));
      shell.Closed(true);
      BRepBuilderAPI_MakeSolid make(shell);
      if (!make.IsDone()) continue;
      TopoDS_Solid candidate = make.Solid();
      if (!BRepLib::OrientClosedSolid(candidate) || !BRepCheck_Analyzer(candidate).IsValid()) continue;
      solid = candidate;
      break;
    }
    if (!solid.IsNull()) break;
  }
  if (solid.IsNull()) throw std::runtime_error("the dome does not form a valid solid");
  const double cap_volume = volume(solid);
  if (!(cap_volume > 0)) throw std::runtime_error("the dome encloses no volume");
  // A dome lies between the face and the apex height.
  if (cap_volume > area.Mass() * height * (1 + 1e-6))
    throw std::runtime_error("the dome surface bulges past its height; reduce the height");
  // The dome shares the face's edges, so it replaces the face in place; the
  // volume must change by exactly the cap's, or the dome hit other material.
  Handle(BRepTools_ReShape) reshape = new BRepTools_ReShape();
  TopoDS_Shape replacement = dome_face;
  // Keep the body's outward side: the dome's normal must point the way the face's did.
  {
    BRepAdaptor_Surface along(dome_face);
    double u0 = along.FirstUParameter(), u1 = along.LastUParameter(), v0 = along.FirstVParameter(),
           v1 = along.LastVParameter();
    GeomAPI_ProjectPointOnSurf foot(apex, BRep_Tool::Surface(dome_face));
    double u = 0.5 * (u0 + u1), v = 0.5 * (v0 + v1);
    if (foot.NbPoints()) foot.LowerDistanceParameters(u, v);
    gp_Pnt p;
    gp_Vec du, dv;
    BRep_Tool::Surface(dome_face)->D1(u, v, p, du, dv);
    gp_Vec n = du.Crossed(dv);
    if (dome_face.Orientation() == TopAbs_REVERSED) n.Reverse();
    gp_Dir original(info.normal.x, info.normal.y, info.normal.z);
    if (n.Dot(gp_Vec(original)) < 0) replacement.Reverse();
  }
  reshape->Replace(face, replacement);
  const TopoDS_Shape out = reshape->Apply(body.shape);
  if (!BRepCheck_Analyzer(out).IsValid()) throw std::runtime_error("the dome makes the body invalid; change the height");
  BOPAlgo_ArgumentAnalyzer self_check;
  self_check.SetShape1(out);
  self_check.SelfInterMode() = true;
  self_check.Perform();
  const double delta = (volume(out) - volume(body.shape)) * (inward ? -1 : 1);
  if (self_check.HasFaulty() || std::abs(delta - cap_volume) > std::max(cap_volume * 1e-6, tolerance * size * size))
    throw std::runtime_error(inward ? "inward dome exceeds the supporting material; reduce its height"
                                    : "outward dome intersects other material; reduce height or change the face");
  // Everything but the face is unchanged; the dome face comes from it.
  for (uint8_t kind = 0; kind < 3; ++kind) {
    TopTools_IndexedMapOfShape before, after;
    const TopAbs_ShapeEnum type = kind == 0 ? TopAbs_FACE : kind == 1 ? TopAbs_EDGE : TopAbs_VERTEX;
    TopExp::MapShapes(body.shape, type, before);
    TopExp::MapShapes(out, type, after);
    for (int i = 1; i <= before.Extent(); ++i) {
      if (kind == 0 && before(i).IsSame(face)) {
        const int to = after.FindIndex(replacement);
        if (to > 0) history.push_back(HistoryEntry{0, 0, static_cast<uint32_t>(i - 1), 2, 0, static_cast<uint32_t>(to - 1)});
        continue;
      }
      const int to = after.FindIndex(before(i));
      if (to > 0) history.push_back(HistoryEntry{0, kind, static_cast<uint32_t>(i - 1), 0, kind, static_cast<uint32_t>(to - 1)});
    }
  }
  return std::make_unique<Shape>(out);
}
} // namespace

std::unique_ptr<Shape> dome(const Shape &body, uint32_t face_index, double height,
                            bool inward, rust::Vec<HistoryEntry> &history) {
  if (!circular_disk(TopoDS::Face(sub_shape(body, 0, face_index)->shape)))
    return dome_filled(body, face_index, height, inward, false, history);
  const Disk disk = disk_of(body, face_index, height, inward);
  if (height > disk.radius)
    throw std::invalid_argument("dome height must be at most the face's circular radius");
  const double radius = disk.radius;
  const double sphere_radius = (radius*radius + height*height)/(2*height);
  const auto center = disk.circle.Location().Translated(gp_Vec(disk.normal)*(height-sphere_radius));
  // Direct spherical segment construction yields the same exact cap as the
  // measured sphere/half-space prototype without an arbitrary clipping box.
  const double latitude = std::asin(std::clamp(1-height/sphere_radius, -1.0, 1.0));
  BRepPrimAPI_MakeSphere make_cap(gp_Ax2(center, disk.normal), sphere_radius, latitude, PI/2);
  const double expected = PI*height*(3*radius*radius + height*height)/6;
  return attach(body, face_index, disk, make_cap.Shape(), expected, inward, history);
}

std::unique_ptr<Shape> dome_elliptical(const Shape &body, uint32_t face_index, double height,
                                       bool inward, rust::Vec<HistoryEntry> &history) {
  if (!circular_disk(TopoDS::Face(sub_shape(body, 0, face_index)->shape)))
    return dome_filled(body, face_index, height, inward, true, history);
  const Disk disk = disk_of(body, face_index, height, inward);
  const double radius = disk.radius;
  // Revolve the quarter ellipse from the rim to the apex, closed through the centre.
  const gp_Pnt center = disk.circle.Location();
  const gp_Dir x(gp_Vec(center, ElCLib::Value(0, disk.circle)));
  const gp_Pnt rim = center.Translated(gp_Vec(x)*radius);
  const gp_Pnt apex = center.Translated(gp_Vec(disk.normal)*height);
  // Put the major axis first so that parameters 0 to pi/2 span exactly the
  // quarter between the rim and the apex.
  const bool wide = radius >= height;
  const gp_Elips ellipse = wide
      ? gp_Elips(gp_Ax2(center, x.Crossed(disk.normal), x), radius, height)
      : gp_Elips(gp_Ax2(center, disk.normal.Crossed(x), disk.normal), height, radius);
  Handle(Geom_TrimmedCurve) arc = new Geom_TrimmedCurve(new Geom_Ellipse(ellipse), 0, PI/2);
  BRepBuilderAPI_MakeWire wire;
  wire.Add(BRepBuilderAPI_MakeEdge(center, rim).Edge());
  wire.Add(BRepBuilderAPI_MakeEdge(arc).Edge());
  wire.Add(BRepBuilderAPI_MakeEdge(apex, center).Edge());
  if (!wire.IsDone()) throw std::runtime_error("could not outline the elliptical dome");
  const TopoDS_Face section = BRepBuilderAPI_MakeFace(wire.Wire(), true).Face();
  BRepPrimAPI_MakeRevol revolve(section, gp_Ax1(center, disk.normal), 2*PI);
  if (!revolve.IsDone()) throw std::runtime_error("could not revolve the elliptical dome");
  TopExp_Explorer solid(revolve.Shape(), TopAbs_SOLID);
  if (!solid.More()) throw std::runtime_error("the elliptical dome is not a solid");
  const double expected = 2.0/3.0*PI*radius*radius*height;
  return attach(body, face_index, disk, solid.Current(), expected, inward, history);
}
} // namespace ofk

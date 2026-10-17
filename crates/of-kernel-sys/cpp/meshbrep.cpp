// Meshes as B-rep: triangles as planar faces sharing their edges, and faces
// fitted to scan data (analytic surfaces in a parameter box, and B-spline
// surfaces through grids of points).

#include "of-kernel-sys/cpp/ofk.h"
#include "of-kernel-sys/src/lib.rs.h"

#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GeomAPI_PointsToBSplineSurface.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Plane.hxx>
#include <Geom_SphericalSurface.hxx>
#include <Geom_ToroidalSurface.hxx>
#include <Precision.hxx>
#include <TColgp_Array2OfPnt.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Shell.hxx>
#include <gp_Ax3.hxx>
#include <gp_Pln.hxx>

#include <map>
#include <stdexcept>
#include <vector>

namespace ofk {

std::unique_ptr<Shape> mesh_brep(rust::Slice<const double> points, rust::Slice<const uint32_t> triangles) {
  const size_t n = points.size() / 3;
  if (triangles.size() % 3 != 0 || triangles.empty()) throw std::invalid_argument("a mesh needs triangles");
  std::vector<TopoDS_Vertex> verts(n);
  std::vector<bool> made(n, false);
  auto vertex = [&](uint32_t i) {
    if (i >= n) throw std::out_of_range("a triangle refers to a missing vertex");
    if (!made[i]) {
      verts[i] = BRepBuilderAPI_MakeVertex(gp_Pnt(points[3 * i], points[3 * i + 1], points[3 * i + 2])).Vertex();
      made[i] = true;
    }
    return verts[i];
  };
  std::map<std::pair<uint32_t, uint32_t>, TopoDS_Edge> edges;
  auto edge = [&](uint32_t a, uint32_t b) {
    auto key = std::make_pair(std::min(a, b), std::max(a, b));
    auto it = edges.find(key);
    if (it == edges.end()) {
      BRepBuilderAPI_MakeEdge make(vertex(key.first), vertex(key.second));
      if (!make.IsDone()) throw std::runtime_error("a mesh edge has zero length");
      it = edges.emplace(key, make.Edge()).first;
    }
    TopoDS_Edge e = it->second;
    return a == key.first ? e : TopoDS::Edge(e.Reversed());
  };
  BRep_Builder b;
  TopoDS_Shell shell;
  b.MakeShell(shell);
  for (size_t t = 0; t < triangles.size(); t += 3) {
    const uint32_t i = triangles[t], j = triangles[t + 1], k = triangles[t + 2];
    const gp_Pnt pi = BRep_Tool::Pnt(vertex(i)), pj = BRep_Tool::Pnt(vertex(j)), pk = BRep_Tool::Pnt(vertex(k));
    gp_Vec normal = gp_Vec(pi, pj).Crossed(gp_Vec(pi, pk));
    if (normal.Magnitude() < 1e-14) continue;
    BRepBuilderAPI_MakeWire wire;
    wire.Add(edge(i, j));
    wire.Add(edge(j, k));
    wire.Add(edge(k, i));
    if (!wire.IsDone()) throw std::runtime_error("a mesh triangle doesn't close");
    gp_Pln plane(pi, gp_Dir(normal));
    BRepBuilderAPI_MakeFace face(plane, wire.Wire(), true);
    if (!face.IsDone()) throw std::runtime_error("a mesh triangle can't be a face");
    b.Add(shell, face.Face());
  }
  return std::make_unique<Shape>(shell);
}

// Kinds: 1 plane, 2 cylinder, 3 cone (r1 radius at the origin, r2 half
// angle), 4 sphere, 5 torus (r1 major, r2 minor). The frame is the
// surface's position: origin, axis (z) and x direction.
std::unique_ptr<Shape> primitive_face(uint8_t kind, const Frame &frame, double r1, double r2,
                                      rust::Slice<const double> bounds) {
  if (bounds.size() != 4) throw std::invalid_argument("a face needs its parameter range");
  const gp_Ax3 ax(gp_Pnt(frame.origin.x, frame.origin.y, frame.origin.z),
                  gp_Dir(frame.normal.x, frame.normal.y, frame.normal.z),
                  gp_Dir(frame.x_dir.x, frame.x_dir.y, frame.x_dir.z));
  Handle(Geom_Surface) s;
  switch (kind) {
  case 1: s = new Geom_Plane(ax); break;
  case 2: s = new Geom_CylindricalSurface(ax, r1); break;
  case 3: s = new Geom_ConicalSurface(ax, r2, r1); break;
  case 4: s = new Geom_SphericalSurface(ax, r1); break;
  case 5: s = new Geom_ToroidalSurface(ax, r1, r2); break;
  default: throw std::invalid_argument("unknown surface kind");
  }
  BRepBuilderAPI_MakeFace make(s, bounds[0], bounds[1], bounds[2], bounds[3], Precision::Confusion());
  if (!make.IsDone()) throw std::runtime_error("the fitted face can't be made");
  return std::make_unique<Shape>(make.Face());
}

std::unique_ptr<Shape> bspline_face(rust::Slice<const double> points, uint32_t nu, uint32_t nv) {
  if (nu < 2 || nv < 2 || points.size() != static_cast<size_t>(nu) * nv * 3)
    throw std::invalid_argument("a grid of points is needed");
  TColgp_Array2OfPnt grid(1, static_cast<int>(nu), 1, static_cast<int>(nv));
  for (uint32_t i = 0; i < nu; ++i)
    for (uint32_t j = 0; j < nv; ++j) {
      const size_t at = 3 * (static_cast<size_t>(j) * nu + i);
      grid.SetValue(static_cast<int>(i) + 1, static_cast<int>(j) + 1, gp_Pnt(points[at], points[at + 1], points[at + 2]));
    }
  GeomAPI_PointsToBSplineSurface fit(grid, 3, 8, GeomAbs_C2, 1e-3);
  if (!fit.IsDone()) throw std::runtime_error("no smooth surface fits the points");
  BRepBuilderAPI_MakeFace make(fit.Surface(), Precision::Confusion());
  if (!make.IsDone()) throw std::runtime_error("the fitted surface can't be a face");
  return std::make_unique<Shape>(make.Face());
}

} // namespace ofk

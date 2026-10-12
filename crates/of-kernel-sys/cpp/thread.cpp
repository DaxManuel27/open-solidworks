// Modeled threads: the material a thread removes from a cylinder.
//
// The groove uses the ISO 68-1 basic profile (60 degree flanks, fundamental
// triangle height H = sqrt(3)/2 P, thread depth 5H/8). An external thread's
// groove is P/4 wide at the root and 7P/8 wide at the major diameter; an
// internal thread's groove is 3P/4 wide at the minor diameter and P/8 wide at
// the major diameter. The groove lies in a plane through the axis and is swept
// along a helix with the axis as a fixed binormal, so every axial cross-section
// shows the same profile. The sweep starts and ends a pitch beyond the thread
// and is trimmed flat at both ends of the thread length.
#include "of-kernel-sys/cpp/ofk.h"
#include "of-kernel-sys/src/lib.rs.h"

#include <BRepAlgoAPI_Common.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepLib.hxx>
#include <BRepOffsetAPI_MakePipeShell.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <Geom2d_Line.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <gp_Ax3.hxx>

#include <cmath>
#include <stdexcept>

namespace ofk {

std::unique_ptr<Shape> thread_tool(const Frame &frame, double radius, double pitch, double length,
                                   bool left, bool internal) {
  if (!(std::isfinite(radius) && std::isfinite(pitch) && std::isfinite(length)) || radius <= 0 ||
      pitch <= 0 || length <= 0)
    throw std::invalid_argument("thread radius, pitch and length must be positive and finite");
  const double turns = length / pitch + 2;
  if (turns > 202) throw std::invalid_argument("modeled threads support at most 200 turns");
  const double PI = std::acos(-1.0);
  const double depth = 5.0 / 8.0 * std::sqrt(3.0) / 2.0 * pitch;
  const double margin = 0.05 * pitch, flare = 2 * margin * std::tan(PI / 6);
  double r_in, w_in, r_out, w_out;
  if (internal) {
    r_in = radius - margin, w_in = 0.75 * pitch + flare;
    r_out = radius + depth, w_out = pitch / 8;
  } else {
    r_in = radius - depth, w_in = pitch / 4;
    r_out = radius + margin, w_out = 7.0 / 8.0 * pitch + flare;
  }
  if (r_in <= 1e-3 * radius) throw std::invalid_argument("the pitch is too coarse for this diameter");

  const gp_Dir axis(frame.normal.x, frame.normal.y, frame.normal.z);
  const gp_Dir x(frame.x_dir.x, frame.x_dir.y, frame.x_dir.z);
  const gp_Pnt start(frame.origin.x, frame.origin.y, frame.origin.z);
  const gp_Pnt base = start.Translated(gp_Vec(axis) * -pitch);
  // Spine: a helix from one pitch before the thread to one pitch after it.
  Handle(Geom_CylindricalSurface) surface = new Geom_CylindricalSurface(gp_Ax3(base, axis, x), r_in);
  const double angle = turns * 2 * PI * (left ? -1 : 1), height = turns * pitch;
  Handle(Geom2d_Line) line = new Geom2d_Line(gp_Pnt2d(0, 0), gp_Dir2d(angle, height));
  TopoDS_Edge helix = BRepBuilderAPI_MakeEdge(line, surface, 0, std::hypot(angle, height)).Edge();
  if (!BRepLib::BuildCurves3d(helix, 1e-7)) throw std::runtime_error("thread helix approximation failed");
  TopoDS_Wire spine = BRepBuilderAPI_MakeWire(helix).Wire();

  auto at = [&](double r, double z) {
    return base.Translated(gp_Vec(x) * r + gp_Vec(axis) * z);
  };
  BRepBuilderAPI_MakePolygon profile(at(r_in, -w_in / 2), at(r_out, -w_out / 2),
                                     at(r_out, w_out / 2), at(r_in, w_in / 2), true);
  BRepOffsetAPI_MakePipeShell pipe(spine);
  pipe.SetMode(axis);
  pipe.Add(profile.Wire(), false, false);
  pipe.Build();
  if (!pipe.IsDone() || !pipe.MakeSolid()) throw std::runtime_error("thread sweep failed");
  // Trim flat at the thread's ends.
  BRepPrimAPI_MakeCylinder slab(gp_Ax2(start, axis, x), r_out + pitch, length);
  BRepAlgoAPI_Common trim(pipe.Shape(), slab.Shape());
  if (!trim.IsDone() || trim.HasErrors()) throw std::runtime_error("could not trim the thread ends");
  TopExp_Explorer solids(trim.Shape(), TopAbs_SOLID);
  if (!solids.More()) throw std::runtime_error("the thread is empty");
  const TopoDS_Shape solid = solids.Current();
  solids.Next();
  if (solids.More()) throw std::runtime_error("the thread groove is not one solid");
  if (!BRepCheck_Analyzer(solid).IsValid()) throw std::runtime_error("the thread geometry is invalid");
  return std::make_unique<Shape>(solid);
}
} // namespace ofk

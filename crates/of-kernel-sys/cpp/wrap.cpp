// Wrap: map closed sketch regions onto a cylindrical or conical face by true
// arc length.
//
// A sketch plane parallel to the cylinder axis maps affinely onto the
// cylinder's (u, v) parameter space: distance along the plane around the axis
// becomes angle (arc length / radius) and distance along the axis becomes v.
// Every sketch curve is converted to an exact (rational) B-spline in sketch
// coordinates and its poles are mapped through that affine transform, so the
// parameter-space curves are exact and the wrapped edges lie on the face.
//
// A cone unrolls onto the plane tangent to it along one of its lines: a sector
// centred on the apex, where distance from the apex is slant distance and angle
// is turn around the axis times the sine of the half angle. The sketch plane
// must be parallel to such a tangent plane. That map is not affine, so each
// curve's parameter-space image is interpolated through mapped points, doubling
// them until the measured deviation is below tolerance.
#include "ofk.h"
#include "of-kernel-sys/src/lib.rs.h"

#include <BRepAdaptor_Surface.hxx>
#include <string>
#include <Geom_Plane.hxx>
#include <Geom_OffsetSurface.hxx>
#include <GeomLProp_SLProps.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <Bnd_Box.hxx>
#include <BRepBndLib.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <Geom_SphericalSurface.hxx>
#include <ShapeAnalysis_Surface.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRepFill.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepGProp.hxx>
#include <BRepLib.hxx>
#include <BRepTools.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GCE2d_MakeSegment.hxx>
#include <Geom2dConvert.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2d_BezierCurve.hxx>
#include <Geom2d_Circle.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <Geom2dAPI_Interpolate.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <TColStd_HArray1OfReal.hxx>
#include <TColgp_HArray1OfPnt2d.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <ShapeFix_Face.hxx>
#include <ShapeFix_Solid.hxx>
#include <TColStd_Array1OfInteger.hxx>
#include <TColStd_Array1OfReal.hxx>
#include <TColgp_Array1OfPnt2d.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <gp_Ax2d.hxx>
#include <gp_Circ2d.hxx>

#include <cmath>
#include <functional>
#include <stdexcept>
#include <vector>

namespace ofk {
namespace {
constexpr double PI = 3.14159265358979323846;

gp_XYZ xyz(const Vec3 &v) { return gp_XYZ(v.x, v.y, v.z); }

// Sketch coordinates to the cylinder's (u, v) parameters.
struct Map {
  double a, b, c, d, e, f;
  gp_Pnt2d apply(const gp_Pnt2d &p) const {
    return gp_Pnt2d(a * p.X() + b * p.Y() + c, d * p.X() + e * p.Y() + f);
  }
};

Handle(Geom2d_BSplineCurve) to_bspline(const Handle(Geom2d_Curve) &curve) {
  return Geom2dConvert::CurveToBSplineCurve(curve);
}

// A sketch curve (see make_edge in ofk.cpp for the encoding) as an exact 2D B-spline.
Handle(Geom2d_BSplineCurve) sketch_curve(const Curve &c) {
  const auto &d = c.data;
  for (double value : d)
    if (!std::isfinite(value)) throw std::invalid_argument("sketch coordinates must be finite");
  auto need = [&](size_t n) {
    if (d.size() < n) throw std::invalid_argument("sketch curve has too few values");
  };
  switch (c.kind) {
  case 0: {
    need(4);
    gp_Pnt2d a(d[0], d[1]), b(d[2], d[3]);
    if (a.Distance(b) < Precision::Confusion()) throw std::invalid_argument("zero-length line");
    return to_bspline(GCE2d_MakeSegment(a, b).Value());
  }
  case 1: {
    need(6);
    gp_Pnt2d center(d[0], d[1]), s(d[2], d[3]), e(d[4], d[5]);
    const double r = 0.5 * (center.Distance(s) + center.Distance(e));
    double a0 = std::atan2(s.Y() - center.Y(), s.X() - center.X());
    double a1 = std::atan2(e.Y() - center.Y(), e.X() - center.X());
    if (a1 <= a0) a1 += 2 * PI;
    Handle(Geom2d_Circle) circle = new Geom2d_Circle(gp_Circ2d(gp_Ax2d(center, gp_Dir2d(1, 0)), r));
    return to_bspline(new Geom2d_TrimmedCurve(circle, a0, a1));
  }
  case 2: {
    need(3);
    Handle(Geom2d_Circle) circle =
        new Geom2d_Circle(gp_Circ2d(gp_Ax2d(gp_Pnt2d(d[0], d[1]), gp_Dir2d(1, 0)), d[2]));
    // A trimmed full turn converts to a non-periodic spline, which downstream
    // offsets and integration handle far more cheaply than a periodic one.
    return to_bspline(new Geom2d_TrimmedCurve(circle, 0, 2 * PI));
  }
  case 3:
  case 4: {
    const int n = static_cast<int>(d.size() / 2);
    if (n < 2) throw std::invalid_argument("spline needs at least two points");
    const bool closed = c.kind == 4;
    auto get = [&](int i) {
      if (closed) i = (i % n + n) % n;
      if (i < 0) return gp_XY(2 * d[0] - d[2], 2 * d[1] - d[3]);
      if (i >= n) return gp_XY(2 * d[2 * n - 2] - d[2 * n - 4], 2 * d[2 * n - 1] - d[2 * n - 3]);
      return gp_XY(d[2 * i], d[2 * i + 1]);
    };
    const int spans = closed ? n : n - 1;
    TColgp_Array1OfPnt2d poles(1, spans * 3 + 1);
    TColStd_Array1OfReal knots(1, spans + 1);
    TColStd_Array1OfInteger mults(1, spans + 1);
    for (int i = 0; i < spans; ++i) {
      const gp_XY a = get(i), b = get(i + 1);
      poles.SetValue(3 * i + 1, gp_Pnt2d(a));
      poles.SetValue(3 * i + 2, gp_Pnt2d(a + (b - get(i - 1)) / 6.0));
      poles.SetValue(3 * i + 3, gp_Pnt2d(b - (get(i + 2) - a) / 6.0));
    }
    poles.SetValue(spans * 3 + 1, gp_Pnt2d(get(spans)));
    for (int i = 0; i <= spans; ++i) {
      knots.SetValue(i + 1, i);
      mults.SetValue(i + 1, i == 0 || i == spans ? 4 : 3);
    }
    return new Geom2d_BSplineCurve(poles, knots, mults, 3);
  }
  case 5: {
    need(8);
    TColgp_Array1OfPnt2d poles(1, 4);
    for (int i = 0; i < 4; ++i) poles.SetValue(i + 1, gp_Pnt2d(d[2 * i], d[2 * i + 1]));
    return to_bspline(new Geom2d_BezierCurve(poles));
  }
  default:
    throw std::invalid_argument("unknown sketch curve kind");
  }
}

Handle(Geom2d_BSplineCurve) mapped(const Curve &c, const Map &m) {
  Handle(Geom2d_BSplineCurve) curve = sketch_curve(c);
  // Rational B-splines are invariant under affine maps of their poles.
  for (int i = 1; i <= curve->NbPoles(); ++i) curve->SetPole(i, m.apply(curve->Pole(i)));
  return curve;
}

struct Target {
  Handle(Geom_Surface) surface;
  Handle(Geom_CylindricalSurface) cylinder;
  Handle(Geom_ConicalSurface) cone;
  TopoDS_Face face;
  double umin, umax, vmin, vmax;
};

Handle(Geom_Surface) basis(const TopoDS_Face &face) {
  Handle(Geom_Surface) surface = BRep_Tool::Surface(face);
  while (auto trimmed = Handle(Geom_RectangularTrimmedSurface)::DownCast(surface))
    surface = trimmed->BasisSurface();
  return surface;
}

Target target_of(const Shape &body, uint32_t face_index) {
  const auto selected = sub_shape(body, 0, face_index);
  Target t;
  t.face = TopoDS::Face(selected->shape);
  t.surface = basis(t.face);
  t.cylinder = Handle(Geom_CylindricalSurface)::DownCast(t.surface);
  t.cone = Handle(Geom_ConicalSurface)::DownCast(t.surface);
  if (t.surface->IsKind(STANDARD_TYPE(Geom_Plane)))
    throw std::invalid_argument("wrap requires a curved face; sketch directly on a planar face instead");
  BRepTools::UVBounds(t.face, t.umin, t.umax, t.vmin, t.vmax);
  return t;
}

// The coaxial surface `distance` along the normal pointing away from the axis,
// with the same (u, v) parametrization.
Handle(Geom_Surface) offset_surface(const Handle(Geom_Surface) &surface, double distance) {
  // Other surfaces offset along their own normal; the caller picks the sign.
  if (!surface->IsKind(STANDARD_TYPE(Geom_CylindricalSurface)) &&
      !surface->IsKind(STANDARD_TYPE(Geom_ConicalSurface)))
    return new Geom_OffsetSurface(surface, distance);
  if (auto cylinder = Handle(Geom_CylindricalSurface)::DownCast(surface))
    return new Geom_CylindricalSurface(cylinder->Position(), cylinder->Radius() + distance);
  if (auto cone = Handle(Geom_ConicalSurface)::DownCast(surface)) {
    // P(u, v) + d * (cos(a) e_r - sin(a) z) is the cone with the same half angle
    // whose reference circle moves out by d cos(a) and down the axis by d sin(a).
    const double a = cone->SemiAngle();
    gp_Ax3 position = cone->Position();
    position.Translate(gp_Vec(position.Direction()) * (-distance * std::sin(a)));
    const double radius = cone->RefRadius() + distance * std::cos(a);
    if (radius <= 0) throw std::invalid_argument("the wrap depth reaches the cone's axis");
    return new Geom_ConicalSurface(position, a, radius);
  }
  throw std::invalid_argument("wrap requires a cylindrical or conical face");
}

// The geodesic exponential map of a surface from (u0, v0): a tangent vector
// becomes the end of the geodesic leaving in its direction for its length.
// Geodesics follow u'' = -G^u_ij u^i u^j, v'' = -G^v_ij u^i u^j with Christoffel
// symbols from the first and second derivatives, integrated by Runge-Kutta.
struct Exponential {
  Handle(Geom_Surface) surface;
  double u0, v0, size;
  double ulo, uhi, vlo, vhi;  // the surface's domain in non-periodic directions

  void rhs(const double y[4], double out[4]) const {
    gp_Pnt P;
    gp_Vec Su, Sv, Suu, Svv, Suv;
    surface->D2(y[0], y[1], P, Su, Sv, Suu, Svv, Suv);
    const double E = Su.Dot(Su), F = Su.Dot(Sv), G = Sv.Dot(Sv), det = E * G - F * F;
    if (det < 1e-14 * E * G + 1e-300)
      throw std::invalid_argument("the wrap crosses a singular point of the face's surface");
    auto christoffel = [&](const gp_Vec &S, double &gu, double &gv) {
      const double a = S.Dot(Su), b = S.Dot(Sv);
      gu = (G * a - F * b) / det;
      gv = (E * b - F * a) / det;
    };
    double uu_u, uu_v, uv_u, uv_v, vv_u, vv_v;
    christoffel(Suu, uu_u, uu_v);
    christoffel(Suv, uv_u, uv_v);
    christoffel(Svv, vv_u, vv_v);
    out[0] = y[2];
    out[1] = y[3];
    out[2] = -(uu_u * y[2] * y[2] + 2 * uv_u * y[2] * y[3] + vv_u * y[3] * y[3]);
    out[3] = -(uu_v * y[2] * y[2] + 2 * uv_v * y[2] * y[3] + vv_v * y[3] * y[3]);
  }

  gp_Pnt2d operator()(const gp_Vec &w) const {
    const double length = w.Magnitude();
    if (length < 1e-15 * size) return gp_Pnt2d(u0, v0);
    gp_Pnt P;
    gp_Vec Su, Sv;
    surface->D1(u0, v0, P, Su, Sv);
    const double E = Su.Dot(Su), F = Su.Dot(Sv), G = Sv.Dot(Sv), det = E * G - F * F;
    const gp_Vec t = w / length;
    const double a = t.Dot(Su), b = t.Dot(Sv);
    double y[4] = {u0, v0, (G * a - F * b) / det, (E * b - F * a) / det};
    const int steps = std::max(16, static_cast<int>(std::ceil(length / (2e-3 * size))));
    const double h = length / steps;
    for (int i = 0; i < steps; ++i) {
      double k1[4], k2[4], k3[4], k4[4], tmp[4];
      rhs(y, k1);
      for (int j = 0; j < 4; ++j) tmp[j] = y[j] + 0.5 * h * k1[j];
      rhs(tmp, k2);
      for (int j = 0; j < 4; ++j) tmp[j] = y[j] + 0.5 * h * k2[j];
      rhs(tmp, k3);
      for (int j = 0; j < 4; ++j) tmp[j] = y[j] + h * k3[j];
      rhs(tmp, k4);
      for (int j = 0; j < 4; ++j) y[j] += h / 6 * (k1[j] + 2 * k2[j] + 2 * k3[j] + k4[j]);
      if ((!surface->IsUPeriodic() && (y[0] < ulo || y[0] > uhi)) ||
          (!surface->IsVPeriodic() && (y[1] < vlo || y[1] > vhi)))
        throw std::invalid_argument("the wrap runs past the edge of the face's surface");
    }
    return gp_Pnt2d(y[0], y[1]);
  }
};

// The region of `face` swept along the surface normal to `offset`, a coaxial
// surface with the same parametrization. Each boundary edge is copied onto it
// with the same parameter-space curve; the ruled surface between a curve and its
// copy is exactly the normal wall, since corresponding points share (u, v).
TopoDS_Shape radial_solid(const TopoDS_Face &face, const Handle(Geom_Surface) &offset) {
  BRepBuilderAPI_Sewing sewing(1e-6);
  sewing.Add(face);
  const TopoDS_Wire outer = BRepTools::OuterWire(face);
  std::vector<TopoDS_Wire> holes;
  TopoDS_Wire outer_copy;
  for (TopExp_Explorer w(face, TopAbs_WIRE); w.More(); w.Next()) {
    const TopoDS_Wire wire = TopoDS::Wire(w.Current());
    BRepBuilderAPI_MakeWire copy;
    for (BRepTools_WireExplorer e(wire, face); e.More(); e.Next()) {
      const TopoDS_Edge edge = e.Current();
      double first, last;
      Handle(Geom2d_Curve) uv = BRep_Tool::CurveOnSurface(edge, face, first, last);
      if (uv.IsNull()) throw std::runtime_error("wrapped edge has no parameter-space curve");
      TopoDS_Edge moved = BRepBuilderAPI_MakeEdge(uv, offset, first, last).Edge();
      BRepLib::BuildCurves3d(moved, 1e-7);
      // Keep the wire's edge orientation so the ruled wall pairs matching ends.
      moved.Orientation(edge.Orientation());
      copy.Add(moved);
      if (!copy.IsDone()) throw std::runtime_error("could not copy the wrapped boundary");
      sewing.Add(BRepFill::Face(edge, moved));
    }
    if (wire.IsSame(outer)) outer_copy = copy.Wire();
    else holes.push_back(copy.Wire());
  }
  BRepBuilderAPI_MakeFace top(offset, outer_copy, true);
  for (const auto &hole : holes) top.Add(TopoDS::Wire(hole.Reversed()));
  if (!top.IsDone()) throw std::runtime_error("could not build the offset wrapped face");
  ShapeFix_Face fix(top.Face());
  fix.FixOrientation();
  fix.Perform();
  sewing.Add(fix.Face());
  sewing.Perform();
  TopExp_Explorer shells(sewing.SewedShape(), TopAbs_SHELL);
  if (!shells.More()) throw std::runtime_error("wrapped region did not close into a shell");
  BRepBuilderAPI_MakeSolid make(TopoDS::Shell(shells.Current()));
  shells.Next();
  if (shells.More() || !make.IsDone()) throw std::runtime_error("wrapped region did not close into one solid");
  ShapeFix_Solid heal(make.Solid());
  heal.Perform();
  TopExp_Explorer healed(heal.Solid(), TopAbs_SOLID);
  if (!healed.More()) throw std::runtime_error("wrapped region did not close into one solid");
  TopoDS_Solid solid = TopoDS::Solid(healed.Current());
  if (!BRepLib::OrientClosedSolid(solid)) throw std::runtime_error("wrapped solid is not closed");
  return solid;
}
} // namespace

std::unique_ptr<Shape> wrap_faces(const Shape &body, uint32_t face_index, const Frame &frame,
                                  rust::Slice<const Curve> curves,
                                  rust::Slice<const uint32_t> loop_sizes,
                                  rust::Slice<const uint32_t> face_loops) {
  Target target = target_of(body, face_index);
  const gp_XYZ p = xyz(frame.origin), n = xyz(frame.normal), sx = xyz(frame.x_dir);
  const gp_XYZ sy = n.Crossed(sx);
  double radius;
  // The surface the wrap is built on: the face's own, or the same sphere with
  // its poles turned away from the wrap.
  Handle(Geom_Surface) carrier = target.surface;
  gp_Pnt2d anchor;
  gp_XYZ material;  // the face's outward normal at the anchor, for other faces
  // Sketch coordinates to (u, v), when that map is not affine.
  std::function<gp_Pnt2d(const gp_Pnt2d &)> develop;
  Map map{};
  if (!target.cylinder.IsNull()) {
    const gp_Ax3 axes = target.cylinder->Position();
    const gp_XYZ origin = axes.Location().XYZ(), axis = axes.Direction().XYZ();
    const gp_XYZ ux = axes.XDirection().XYZ(), uy = axes.YDirection().XYZ();
    radius = target.cylinder->Radius();
    if (std::abs(n.Dot(axis)) > 1e-6)
      throw std::invalid_argument("the wrap sketch plane must be parallel to the cylinder's axis");
    // The plane touches the wrap at the axis foot point facing it.
    const double side = (p - origin).Dot(n);
    const gp_XYZ toward = n * (side < 0 ? -1.0 : 1.0);
    const double theta0 = std::atan2(toward.Dot(uy), toward.Dot(ux));
    const gp_XYZ tangent = ux * -std::sin(theta0) + uy * std::cos(theta0);
    map = Map{sx.Dot(tangent) / radius, sy.Dot(tangent) / radius,
              theta0 + (p - origin).Dot(tangent) / radius,
              sx.Dot(axis), sy.Dot(axis), (p - origin).Dot(axis)};
  } else if (!target.cone.IsNull()) {
    const gp_Ax3 axes = target.cone->Position();
    const gp_XYZ origin = axes.Location().XYZ(), axis = axes.Direction().XYZ();
    const gp_XYZ ux = axes.XDirection().XYZ(), uy = axes.YDirection().XYZ();
    const double a = target.cone->SemiAngle(), sa = std::sin(a), R = target.cone->RefRadius();
    radius = R;
    // The cone's outward normal along line u is cos(a) e_r(u) - sin(a) z; the
    // sketch normal must be parallel to one of them.
    gp_XYZ m = n;
    if (std::abs(m.Dot(axis) + sa) > 1e-6) m.Reverse();
    if (std::abs(m.Dot(axis) + sa) > 1e-6)
      throw std::invalid_argument(
          "the wrap sketch plane must be parallel to a plane tangent to the cone (tilted by its half angle)");
    const gp_XYZ radial = m - axis * m.Dot(axis);
    const double theta0 = std::atan2(radial.Dot(uy), radial.Dot(ux));
    const gp_XYZ er = ux * std::cos(theta0) + uy * std::sin(theta0);
    const gp_XYZ et = ux * -std::sin(theta0) + uy * std::cos(theta0);
    const double sign = sa > 0 ? 1.0 : -1.0, apex_v = -R / sa;
    const gp_XYZ apex = origin + axis * (apex_v * std::cos(a));
    // The cone's line at theta0, pointing from the apex into the face's nappe.
    const gp_XYZ line = (er * sa + axis * std::cos(a)) * sign;
    develop = [=](const gp_Pnt2d &q) {
      const gp_XYZ point = p + sx * q.X() + sy * q.Y() - apex;
      const double along = point.Dot(line), across = point.Dot(et);
      const double rho = std::hypot(along, across);
      if (rho < 1e-9 * std::max(1.0, R)) throw std::invalid_argument("the wrap sketch touches the cone's apex");
      return gp_Pnt2d(theta0 + std::atan2(across, along) / std::abs(sa), apex_v + sign * rho);
    };
  } else {
    // Other faces: geodesics from the face point nearest the sketch origin,
    // which must have the sketch plane as its tangent plane.
    Bnd_Box box;
    BRepBndLib::Add(target.face, box);
    radius = std::sqrt(box.SquareExtent());
    GeomAPI_ProjectPointOnSurf foot(gp_Pnt(p), target.surface, target.umin, target.umax, target.vmin, target.vmax);
    if (!foot.NbPoints()) throw std::invalid_argument("the wrap sketch origin does not face the selected face");
    double u0, v0;
    foot.LowerDistanceParameters(u0, v0);
    GeomLProp_SLProps props(target.surface, u0, v0, 1, 1e-9);
    if (!props.IsNormalDefined()) throw std::invalid_argument("the face has no normal under the sketch origin");
    const gp_XYZ normal = props.Normal().XYZ();
    if (std::abs(normal.Dot(n)) < std::cos(1e-3))
      throw std::invalid_argument(
          "the wrap sketch plane must be tangent to the face at the point nearest the sketch origin");
    gp_XYZ e1 = sx - normal * sx.Dot(normal);
    e1.Normalize();
    gp_XYZ e2 = normal.Crossed(e1);
    if (e2.Dot(sy) < 0) e2.Reverse();
    material = target.face.Orientation() == TopAbs_REVERSED ? normal.Reversed() : normal;
    if (auto sphere = Handle(Geom_SphericalSurface)::DownCast(target.surface)) {
      // A sphere's parameters are singular at its poles, where a dome's apex
      // sits. Rebuild it with the anchor on the equator, opposite the seam.
      const gp_Pnt center = sphere->Location();
      gp_Dir out(gp_Vec(center, target.surface->Value(u0, v0)));
      carrier = new Geom_SphericalSurface(gp_Ax3(center, gp_Dir(e1), out.Reversed()), sphere->Radius());
      u0 = PI;
      v0 = 0;
    }
    anchor = gp_Pnt2d(u0, v0);
    Exponential exp{carrier, u0, v0, std::max(1.0, radius), 0, 0, 0, 0};
    carrier->Bounds(exp.ulo, exp.uhi, exp.vlo, exp.vhi);
    develop = [=](const gp_Pnt2d &q) { return exp(gp_Vec(e1 * q.X() + e2 * q.Y())); };
  }
  const double size = std::max(1.0, radius);
  double distortion = 0;
  // One curve's image in (u, v): exact for an affine map, otherwise interpolated
  // through mapped points until midpoints deviate by less than the tolerance.
  auto image = [&](const Curve &c) -> Handle(Geom2d_BSplineCurve) {
    if (!develop) return mapped(c, map);
    Handle(Geom2d_BSplineCurve) source = sketch_curve(c);
    const double t0 = source->FirstParameter(), t1 = source->LastParameter();
    for (int count = 8 * (source->NbKnots() + 1); count <= 8192; count *= 2) {
      Handle(TColgp_HArray1OfPnt2d) points = new TColgp_HArray1OfPnt2d(1, count + 1);
      Handle(TColStd_HArray1OfReal) params = new TColStd_HArray1OfReal(1, count + 1);
      for (int i = 0; i <= count; ++i) {
        const double t = t0 + (t1 - t0) * i / count;
        params->SetValue(i + 1, t);
        points->SetValue(i + 1, develop(source->Value(t)));
      }
      Geom2dAPI_Interpolate fit(points, params, false, 1e-12);
      fit.Perform();
      if (!fit.IsDone()) throw std::runtime_error("could not map a sketch curve onto the cone");
      Handle(Geom2d_BSplineCurve) curve = fit.Curve();
      double worst = 0;
      for (int i = 0; i < count; ++i) {
        const double t = t0 + (t1 - t0) * (i + 0.5) / count;
        const gp_Pnt2d want = develop(source->Value(t)), got = curve->Value(t);
        worst = std::max(worst, carrier->Value(want.X(), want.Y()).Distance(carrier->Value(got.X(), got.Y())));
      }
      if (worst < 1e-7 * size) {
        // Length distortion: mapped chords against sketch chords.
        for (int i = 0; i < count; ++i) {
          const double flat = source->Value(params->Value(i + 1)).Distance(source->Value(params->Value(i + 2)));
          const gp_Pnt2d a = points->Value(i + 1), b = points->Value(i + 2);
          const double wrapped = carrier->Value(a.X(), a.Y()).Distance(carrier->Value(b.X(), b.Y()));
          if (flat > 1e-9 * size) distortion = std::max(distortion, std::abs(wrapped / flat - 1));
        }
        return curve;
      }
    }
    throw std::runtime_error("a sketch curve bends too sharply to wrap onto the face accurately");
  };

  // Build every parameter-space curve first to place the wrap inside the face.
  std::vector<Handle(Geom2d_BSplineCurve)> uv;
  double umin = 1e300, umax = -1e300, vmin = 1e300, vmax = -1e300;
  for (const auto &c : curves) {
    uv.push_back(image(c));
    const auto &curve = uv.back();
    for (int i = 0; i <= 32; ++i) {
      const double t = curve->FirstParameter() +
                       (curve->LastParameter() - curve->FirstParameter()) * i / 32.0;
      const gp_Pnt2d q = curve->Value(t);
      umin = std::min(umin, q.X());
      umax = std::max(umax, q.X());
      vmin = std::min(vmin, q.Y());
      vmax = std::max(vmax, q.Y());
    }
  }
  if (uv.empty()) throw std::invalid_argument("the wrap sketch has no closed contours");
  const bool analytic = !target.cylinder.IsNull() || !target.cone.IsNull();
  if (analytic) {
    if (umax - umin >= 2 * PI - 1e-9)
      throw std::invalid_argument("the wrap sketch is wider than the face's circumference");
    // Move the wrap by whole turns so that it sits inside the face's angular range.
    const double mid = 0.5 * (target.umin + target.umax);
    const double shift = 2 * PI * std::round((mid - 0.5 * (umin + umax)) / (2 * PI));
    const bool full_turn = target.umax - target.umin >= 2 * PI - 1e-7;
    const double tol = 1e-7 * std::max(1.0, radius);
    if ((!full_turn && (umin + shift < target.umin - 1e-9 || umax + shift > target.umax + 1e-9)) ||
        vmin < target.vmin - tol || vmax > target.vmax + tol)
      throw std::invalid_argument("the wrap sketch extends past the selected face");
    if (shift != 0)
      for (auto &curve : uv) curve->Translate(gp_Vec2d(shift, 0));
  } else {
    if (distortion > 0.01)
      throw std::invalid_argument("the wrap would stretch lengths by up to " +
                                  std::to_string(std::lround(distortion * 1000) / 10.0).substr(0, 4) +
                                  "% on this face (the limit is 1%); use a smaller sketch or a flatter face");
    // Move by whole periods into the face's parameter range, then check that
    // every sampled point lies on the face.
    auto shift_of = [](bool periodic, double period, double lo, double hi, double a, double b) {
      return periodic ? period * std::round((0.5 * (lo + hi) - 0.5 * (a + b)) / period) : 0.0;
    };
    if (carrier == target.surface) {
      const double du = shift_of(carrier->IsUPeriodic(), carrier->IsUPeriodic() ? carrier->UPeriod() : 0,
                                 target.umin, target.umax, umin, umax);
      const double dv = shift_of(carrier->IsVPeriodic(), carrier->IsVPeriodic() ? carrier->VPeriod() : 0,
                                 target.vmin, target.vmax, vmin, vmax);
      if (du != 0 || dv != 0)
        for (auto &curve : uv) curve->Translate(gp_Vec2d(du, dv));
    }
    const double tol = 1e-6 * std::max(1.0, radius);
    ShapeAnalysis_Surface on_face(target.surface);
    for (const auto &curve : uv)
      for (int i = 0; i <= 64; ++i) {
        const double t = curve->FirstParameter() + (curve->LastParameter() - curve->FirstParameter()) * i / 64.0;
        // Sample points across a periodic seam are checked at their position
        // within the face's own parameter range.
        gp_Pnt2d q = curve->Value(t);
        if (carrier != target.surface) q = on_face.ValueOfUV(carrier->Value(q.X(), q.Y()), tol);
        if (target.surface->IsUPeriodic() && (q.X() < target.umin || q.X() > target.umax))
          q.SetX(target.umin + std::fmod(std::fmod(q.X() - target.umin, target.surface->UPeriod()) +
                                              target.surface->UPeriod(), target.surface->UPeriod()));
        if (target.surface->IsVPeriodic() && (q.Y() < target.vmin || q.Y() > target.vmax))
          q.SetY(target.vmin + std::fmod(std::fmod(q.Y() - target.vmin, target.surface->VPeriod()) +
                                              target.surface->VPeriod(), target.surface->VPeriod()));
        BRepClass_FaceClassifier inside(target.face, q, tol);
        if (inside.State() == TopAbs_OUT)
          throw std::invalid_argument("the wrap sketch extends past the selected face");
      }
  }

  std::vector<TopoDS_Wire> wires;
  size_t start = 0;
  for (uint32_t count : loop_sizes) {
    if (start + count > uv.size()) throw std::invalid_argument("loop sizes exceed curve count");
    BRepBuilderAPI_MakeWire wire;
    for (size_t i = start; i < start + count; ++i) {
      TopoDS_Edge edge = BRepBuilderAPI_MakeEdge(uv[i], carrier).Edge();
      BRepLib::BuildCurves3d(edge, 1e-7);
      wire.Add(edge);
      if (!wire.IsDone()) throw std::runtime_error("wrapped contour is not connected");
    }
    wires.push_back(wire.Wire());
    start += count;
  }
  TopoDS_Compound result;
  BRep_Builder builder;
  builder.MakeCompound(result);
  size_t w = 0;
  for (uint32_t loops : face_loops) {
    if (loops == 0 || w + loops > wires.size()) throw std::invalid_argument("bad face loop counts");
    BRepBuilderAPI_MakeFace make(carrier, wires[w], true);
    for (uint32_t i = 1; i < loops; ++i) make.Add(TopoDS::Wire(wires[w + i].Reversed()));
    if (!make.IsDone()) throw std::runtime_error("could not build the wrapped face");
    ShapeFix_Face fix(make.Face());
    fix.FixOrientation();
    fix.Perform();
    TopoDS_Face face = fix.Face();
    BRepLib::BuildCurves3d(face, 1e-7);
    if (!BRepCheck_Analyzer(face).IsValid()) throw std::runtime_error("wrapped face is invalid");
    // Other faces are oriented with the target face's outward normal, which
    // tells the wrap solid which side the material is on.
    if (!analytic) {
      GeomLProp_SLProps here(carrier, anchor.X(), anchor.Y(), 1, 1e-9);
      if (here.IsNormalDefined() && here.Normal().XYZ().Dot(material) < 0) face.Reverse();
    }
    builder.Add(result, face);
    w += loops;
  }
  return std::make_unique<Shape>(result);
}

std::unique_ptr<Shape> wrap_solid(const Shape &faces, double depth, bool outward) {
  if (!(std::isfinite(depth) && depth > 0)) throw std::invalid_argument("wrap depth must be positive");
  TopoDS_Compound result;
  BRep_Builder builder;
  builder.MakeCompound(result);
  for (TopExp_Explorer it(faces.shape, TopAbs_FACE); it.More(); it.Next()) {
    const TopoDS_Face face = TopoDS::Face(it.Current());
    Handle(Geom_Surface) surface = basis(face);
    if (auto cylinder = Handle(Geom_CylindricalSurface)::DownCast(surface)) {
      if (!outward && depth >= cylinder->Radius())
        throw std::invalid_argument("deboss depth must be less than the cylinder radius");
    } else if (auto cone = Handle(Geom_ConicalSurface)::DownCast(surface)) {
      // The deboss must stay clear of the axis over the whole wrapped region.
      double u0, u1, v0, v1;
      BRepTools::UVBounds(face, u0, u1, v0, v1);
      const double a = cone->SemiAngle();
      const double nearest = std::min(cone->RefRadius() + v0 * std::sin(a), cone->RefRadius() + v1 * std::sin(a));
      if (!outward && depth * std::cos(a) >= nearest)
        throw std::invalid_argument("deboss depth must be less than the cone's radius under the wrap");
    }
    // Away from the axis for cylinders and cones, along the face's outward
    // normal otherwise.
    double distance = outward ? depth : -depth;
    if (!surface->IsKind(STANDARD_TYPE(Geom_CylindricalSurface)) &&
        !surface->IsKind(STANDARD_TYPE(Geom_ConicalSurface)) && face.Orientation() == TopAbs_REVERSED)
      distance = -distance;
    TopoDS_Shape solid = radial_solid(face, offset_surface(surface, distance));
    if (solid.IsNull() || !BRepCheck_Analyzer(solid).IsValid())
      throw std::runtime_error("wrapped solid is invalid");
    builder.Add(result, solid);
  }
  return std::make_unique<Shape>(result);
}
} // namespace ofk

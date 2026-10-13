// Hidden-line removal for drawing views: the visible and hidden edges of a
// shape projected orthographically onto a view plane, as 2D curves.

#include "of-kernel-sys/cpp/ofk.h"
#include "of-kernel-sys/src/lib.rs.h"

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepLProp_SLProps.hxx>
#include <Geom2d_Curve.hxx>
#include <gp_Pnt2d.hxx>
#include <BRepLib.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRep_Tool.hxx>
#include <GCPnts_TangentialDeflection.hxx>
#include <HLRAlgo_Projector.hxx>
#include <HLRBRep_Algo.hxx>
#include <HLRBRep_HLRToShape.hxx>
#include <HLRBRep_PolyAlgo.hxx>
#include <HLRBRep_PolyHLRToShape.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopoDS.hxx>
#include <gp_Ax2.hxx>
#include <gp_Circ.hxx>

#include <cmath>
#include <stdexcept>

namespace ofk {

namespace {

gp_Pnt pnt(const Vec3 &v) { return gp_Pnt(v.x, v.y, v.z); }
gp_Dir dir(const Vec3 &v) { return gp_Dir(v.x, v.y, v.z); }

// Append one projected edge. The HLR output lies in the projector's XY plane
// (z = 0), so only x and y are kept. Lines and circular arcs stay exact;
// everything else (ellipses from inclined circles, splines) becomes a
// polyline within `deflection`.
void push_edge(const TopoDS_Edge &e, bool visible, uint8_t category, double deflection,
               rust::Vec<HlrEdge> &out) {
  if (BRep_Tool::Degenerated(e)) return;
  BRepAdaptor_Curve c(e);
  double u0 = c.FirstParameter(), u1 = c.LastParameter();
  HlrEdge h;
  h.visible = visible;
  h.category = category;
  switch (c.GetType()) {
  case GeomAbs_Line: {
    gp_Pnt a = c.Value(u0), b = c.Value(u1);
    if (a.Distance(b) < 1e-9) return;
    h.kind = 0;
    for (double v : {a.X(), a.Y(), b.X(), b.Y()}) h.data.push_back(v);
    break;
  }
  case GeomAbs_Circle: {
    gp_Circ circ = c.Circle();
    gp_Pnt ctr = circ.Location();
    gp_Pnt a = c.Value(u0), b = c.Value(u1);
    double a0 = std::atan2(a.Y() - ctr.Y(), a.X() - ctr.X());
    double sweep = u1 - u0;
    // A circle whose axis points away from the viewer runs clockwise on paper.
    if (circ.Axis().Direction().Z() < 0) a0 = std::atan2(b.Y() - ctr.Y(), b.X() - ctr.X());
    h.kind = 1;
    for (double v : {ctr.X(), ctr.Y(), circ.Radius(), a0, a0 + sweep}) h.data.push_back(v);
    break;
  }
  default: {
    GCPnts_TangentialDeflection pts(c, 0.1, deflection, 2);
    if (pts.NbPoints() < 2) return;
    h.kind = 2;
    for (int j = 1; j <= pts.NbPoints(); ++j) {
      gp_Pnt p = pts.Value(j);
      h.data.push_back(p.X());
      h.data.push_back(p.Y());
    }
    break;
  }
  }
  out.push_back(std::move(h));
}

void push_all(const TopoDS_Shape &s, bool visible, uint8_t category, double deflection,
              rust::Vec<HlrEdge> &out) {
  if (s.IsNull()) return;
  TopoDS_Shape c = s;
  BRepLib::BuildCurves3d(c);
  for (TopExp_Explorer ex(c, TopAbs_EDGE); ex.More(); ex.Next()) {
    push_edge(TopoDS::Edge(ex.Current()), visible, category, deflection, out);
  }
}

template <typename Out> void collect(Out &o, bool hidden, double deflection, rust::Vec<HlrEdge> &out) {
  push_all(o.VCompound(), true, 0, deflection, out);
  push_all(o.Rg1LineVCompound(), true, 1, deflection, out);
  push_all(o.RgNLineVCompound(), true, 2, deflection, out);
  push_all(o.OutLineVCompound(), true, 3, deflection, out);
  if (!hidden) return;
  push_all(o.HCompound(), false, 0, deflection, out);
  push_all(o.Rg1LineHCompound(), false, 1, deflection, out);
  push_all(o.RgNLineHCompound(), false, 2, deflection, out);
  push_all(o.OutLineHCompound(), false, 3, deflection, out);
}

} // namespace

rust::Vec<HlrEdge> hlr(const Shape &s, const Frame &view, bool exact, bool hidden,
                       double deflection) {
  if (s.shape.IsNull()) throw std::invalid_argument("nothing to project");
  if (!(deflection > 0)) throw std::invalid_argument("deflection must be positive");
  HLRAlgo_Projector projector(gp_Ax2(pnt(view.origin), dir(view.normal), dir(view.x_dir)));
  rust::Vec<HlrEdge> out;
  if (exact) {
    Handle(HLRBRep_Algo) algo = new HLRBRep_Algo();
    algo->Add(s.shape);
    algo->Projector(projector);
    algo->Update();
    algo->Hide();
    HLRBRep_HLRToShape o(algo);
    collect(o, hidden, deflection, out);
  } else {
    // The polygonal algorithm works on the triangulation; mesh at the
    // requested deflection (existing finer meshes are kept).
    BRepMesh_IncrementalMesh(s.shape, deflection, false, 0.2, true);
    Handle(HLRBRep_PolyAlgo) algo = new HLRBRep_PolyAlgo();
    algo->Load(s.shape);
    algo->Projector(projector);
    algo->Update();
    HLRBRep_PolyHLRToShape o;
    o.Update(algo);
    collect(o, hidden, deflection, out);
  }
  return out;
}

rust::Vec<uint32_t> seam_edges(const Shape &s, const Vec3 &view) {
  gp_Dir v = dir(view);
  TopTools_IndexedMapOfShape em;
  TopExp::MapShapes(s.shape, TopAbs_EDGE, em);
  rust::Vec<uint32_t> out;
  for (TopExp_Explorer f(s.shape, TopAbs_FACE); f.More(); f.Next()) {
    const TopoDS_Face &face = TopoDS::Face(f.Current());
    for (TopExp_Explorer e(face, TopAbs_EDGE); e.More(); e.Next()) {
      const TopoDS_Edge &edge = TopoDS::Edge(e.Current());
      if (!BRep_Tool::IsClosed(edge, face)) continue;
      // A seam lying on the silhouette (surface normal across the view
      // direction) is the outline there and must stay.
      double f, l;
      Handle(Geom2d_Curve) pc = BRep_Tool::CurveOnSurface(edge, face, f, l);
      if (!pc.IsNull()) {
        gp_Pnt2d uv = pc->Value(0.5 * (f + l));
        BRepAdaptor_Surface surf(face);
        BRepLProp_SLProps props(surf, uv.X(), uv.Y(), 1, 1e-9);
        if (props.IsNormalDefined() && std::abs(props.Normal().Dot(v)) < 1e-7) continue;
      }
      out.push_back(static_cast<uint32_t>(em.FindIndex(edge) - 1));
    }
  }
  return out;
}

} // namespace ofk

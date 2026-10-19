// Sheet metal support: surface parameter frames of faces, and flat patterns
// built from faces mapped into one plane through their (u, v) parameters.

#include "of-kernel-sys/cpp/ofk.h"
#include "of-kernel-sys/src/lib.rs.h"

#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepLib.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepTools.hxx>
#include <BRepTools_History.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Geom2dConvert.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2d_Circle.hxx>
#include <Geom2d_Line.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <Geom_Plane.hxx>
#include <ShapeFix_Face.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <TColgp_Array1OfPnt2d.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_DataMapOfShapeInteger.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopTools_ListOfShape.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <gp_Ax22d.hxx>
#include <gp_Circ2d.hxx>
#include <gp_Lin2d.hxx>
#include <gp_Ax3.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Cone.hxx>
#include <gp_Pln.hxx>

#include <cmath>
#include <stdexcept>
#include <vector>

namespace ofk {
namespace {

Vec3 v3(const gp_XYZ &p) { return Vec3{p.X(), p.Y(), p.Z()}; }

// The face as it sits in the shape, with its orientation.
TopoDS_Face oriented_face(const TopoDS_Shape &s, uint32_t index) {
  TopTools_IndexedMapOfShape faces;
  TopExp::MapShapes(s, TopAbs_FACE, faces);
  if (index >= static_cast<uint32_t>(faces.Extent())) throw std::out_of_range("no such face");
  const TopoDS_Shape &face = faces(static_cast<int>(index) + 1);
  for (TopExp_Explorer ex(s, TopAbs_FACE); ex.More(); ex.Next())
    if (ex.Current().IsSame(face)) return TopoDS::Face(ex.Current());
  return TopoDS::Face(face);
}

// (u, v) to the flat plane's (x, y): x = a u + b v + c, y = d u + e v + f.
struct Affine {
  double a, b, c, d, e, f;
  gp_Pnt2d point(const gp_Pnt2d &p) const {
    return gp_Pnt2d(a * p.X() + b * p.Y() + c, d * p.X() + e * p.Y() + f);
  }
  gp_Vec2d vec(const gp_Vec2d &v) const {
    return gp_Vec2d(a * v.X() + b * v.Y(), d * v.X() + e * v.Y());
  }
};

// The image of a parameter-space curve under an affine map, with its new
// parameter range. Lines stay lines and circles under a rigid map stay
// circles, so holes and straight edges keep their exact type.
Handle(Geom2d_Curve) mapped(const Handle(Geom2d_Curve) &curve, double first, double last,
                            const Affine &m, double &nf, double &nl) {
  Handle(Geom2d_Curve) basis = curve;
  while (basis->IsKind(STANDARD_TYPE(Geom2d_TrimmedCurve)))
    basis = Handle(Geom2d_TrimmedCurve)::DownCast(basis)->BasisCurve();
  if (basis->IsKind(STANDARD_TYPE(Geom2d_Line))) {
    gp_Lin2d l = Handle(Geom2d_Line)::DownCast(basis)->Lin2d();
    gp_Vec2d d = m.vec(gp_Vec2d(l.Direction()));
    double scale = d.Magnitude();
    if (scale < 1e-12) throw std::runtime_error("degenerate edge in flat pattern");
    nf = first * scale;
    nl = last * scale;
    return new Geom2d_Line(m.point(l.Location()), gp_Dir2d(d));
  }
  if (basis->IsKind(STANDARD_TYPE(Geom2d_Circle))) {
    gp_Circ2d c = Handle(Geom2d_Circle)::DownCast(basis)->Circ2d();
    gp_Vec2d x = m.vec(gp_Vec2d(c.XAxis().Direction()));
    gp_Vec2d y = m.vec(gp_Vec2d(c.YAxis().Direction()));
    if (std::abs(x.Magnitude() - 1) < 1e-9 && std::abs(y.Magnitude() - 1) < 1e-9 &&
        std::abs(x.Dot(y)) < 1e-9) {
      nf = first;
      nl = last;
      gp_Ax22d axes(m.point(c.Location()), gp_Dir2d(x), gp_Dir2d(y));
      return new Geom2d_Circle(gp_Circ2d(axes, c.Radius()));
    }
  }
  Handle(Geom2d_BSplineCurve) bs =
      Geom2dConvert::CurveToBSplineCurve(new Geom2d_TrimmedCurve(curve, first, last));
  for (int i = 1; i <= bs->NbPoles(); ++i) bs->SetPole(i, m.point(bs->Pole(i)));
  nf = bs->FirstParameter();
  nl = bs->LastParameter();
  return bs;
}

// The face mapped into the plane by `m`, its edges rebuilt from their
// parameter curves. With `straighten`, curved edges become straight. When
// `edges` is given, `origin` records the edge of the source each new edge
// was mapped from.
TopoDS_Face flat_face(const TopoDS_Face &face, const Affine &m, const Handle(Geom_Plane) &plane,
                      bool straighten, const TopTools_IndexedMapOfShape *edges = nullptr,
                      TopTools_DataMapOfShapeInteger *origin = nullptr) {
  TopoDS_Wire outer = BRepTools::OuterWire(face);
  TopoDS_Wire outer_new;
  std::vector<TopoDS_Wire> inner;
  for (TopExp_Explorer wx(face, TopAbs_WIRE); wx.More(); wx.Next()) {
    const TopoDS_Wire &w = TopoDS::Wire(wx.Current());
    BRepBuilderAPI_MakeWire mw;
    for (BRepTools_WireExplorer ex(w, face); ex.More(); ex.Next()) {
      const TopoDS_Edge &e = ex.Current();
      if (BRep_Tool::Degenerated(e)) continue;
      double f, l;
      Handle(Geom2d_Curve) pc = BRep_Tool::CurveOnSurface(e, face, f, l);
      if (pc.IsNull()) throw std::runtime_error("a face edge has no parameter curve");
      double nf, nl;
      Handle(Geom2d_Curve) c = mapped(pc, f, l, m, nf, nl);
      if (straighten && !c->IsKind(STANDARD_TYPE(Geom2d_Line)) &&
          !c->IsKind(STANDARD_TYPE(Geom2d_Circle))) {
        gp_Pnt2d a = c->Value(nf), b = c->Value(nl);
        if (a.Distance(b) > 1e-9) {
          c = new Geom2d_Line(a, gp_Dir2d(gp_Vec2d(a, b)));
          nf = 0;
          nl = a.Distance(b);
        }
      }
      BRepBuilderAPI_MakeEdge me(c, plane, nf, nl);
      if (!me.IsDone()) throw std::runtime_error("could not map an edge into the flat");
      TopoDS_Edge ne = me.Edge();
      BRepLib::BuildCurves3d(ne);
      if (e.Orientation() == TopAbs_REVERSED) ne.Reverse();
      mw.Add(ne);
      if (!mw.IsDone()) throw std::runtime_error("a mapped face boundary is not connected");
      // The wire may copy the edge to share vertices; remember the copy.
      if (edges) {
        int k = edges->FindIndex(e);
        if (k > 0) origin->Bind(mw.Edge(), k - 1);
      }
    }
    if (!mw.IsDone()) continue;
    if (w.IsSame(outer))
      outer_new = mw.Wire();
    else
      inner.push_back(mw.Wire());
  }
  if (outer_new.IsNull()) throw std::runtime_error("a face has no outer boundary");
  BRepBuilderAPI_MakeFace mf(plane, outer_new, true);
  for (const TopoDS_Wire &w : inner) mf.Add(w);
  if (!mf.IsDone()) throw std::runtime_error("could not build a flat face");
  // A mirrored map reverses the loops; only reorient them, so edges stay.
  ShapeFix_Face fixer(mf.Face());
  fixer.FixOrientation();
  return fixer.Face();
}

} // namespace

FaceFrame face_frame(const Shape &s, uint32_t index) {
  TopoDS_Face face = oriented_face(s.shape, index);
  BRepAdaptor_Surface su(face);
  FaceFrame out{};
  gp_Ax3 ax;
  switch (su.GetType()) {
  case GeomAbs_Plane:
    out.kind = 1;
    ax = su.Plane().Position();
    break;
  case GeomAbs_Cylinder:
    out.kind = 2;
    ax = su.Cylinder().Position();
    out.radius = su.Cylinder().Radius();
    break;
  case GeomAbs_Cone:
    out.kind = 3;
    ax = su.Cone().Position();
    out.radius = su.Cone().RefRadius();
    out.semi_angle = su.Cone().SemiAngle();
    break;
  default:
    out.kind = 0;
    return out;
  }
  out.origin = v3(ax.Location().XYZ());
  out.x = v3(ax.XDirection().XYZ());
  out.y = v3(ax.YDirection().XYZ());
  out.z = v3(ax.Direction().XYZ());
  BRepTools::UVBounds(face, out.umin, out.umax, out.vmin, out.vmax);
  out.reversed = face.Orientation() == TopAbs_REVERSED;
  return out;
}

std::unique_ptr<Shape> flatten(const Shape &s, rust::Slice<const uint32_t> faces,
                               rust::Slice<const double> maps, const Frame &frame,
                               const Vec3 &thickness, rust::Vec<int64_t> &sources,
                               bool simplify) {
  if (maps.size() != faces.size() * 6) throw std::invalid_argument("one map per face");
  TopTools_IndexedMapOfShape edges;
  TopExp::MapShapes(s.shape, TopAbs_EDGE, edges);
  gp_Ax3 ax(gp_Pnt(frame.origin.x, frame.origin.y, frame.origin.z),
            gp_Dir(frame.normal.x, frame.normal.y, frame.normal.z),
            gp_Dir(frame.x_dir.x, frame.x_dir.y, frame.x_dir.z));
  Handle(Geom_Plane) plane = new Geom_Plane(ax);
  // Each new edge remembers the edge of `s` it was mapped from.
  TopTools_DataMapOfShapeInteger origin;
  TopTools_ListOfShape flat;
  for (size_t i = 0; i < faces.size(); ++i) {
    TopoDS_Face face = oriented_face(s.shape, faces[i]);
    Affine m{maps[6 * i], maps[6 * i + 1], maps[6 * i + 2],
             maps[6 * i + 3], maps[6 * i + 4], maps[6 * i + 5]};
    // Simplified bends: curved edges of bend faces become straight.
    const bool straighten = simplify && BRepAdaptor_Surface(face).GetType() != GeomAbs_Plane;
    flat.Append(flat_face(face, m, plane, straighten, &edges, &origin));
  }
  if (flat.IsEmpty()) throw std::invalid_argument("nothing to flatten");

  // Join the pieces into one face, merging the shared bend boundaries.
  TopoDS_Shape joined = flat.First();
  std::unique_ptr<BRepAlgoAPI_Fuse> fuse;
  if (flat.Extent() > 1) {
    TopTools_ListOfShape args, tools;
    args.Append(flat.First());
    bool first = true;
    for (const TopoDS_Shape &f : flat) {
      if (first) {
        first = false;
        continue;
      }
      tools.Append(f);
    }
    fuse = std::make_unique<BRepAlgoAPI_Fuse>();
    fuse->SetArguments(args);
    fuse->SetTools(tools);
    fuse->SetFuzzyValue(1e-6);
    fuse->Build();
    if (!fuse->IsDone() || fuse->HasErrors())
      throw std::runtime_error("the flat pattern faces overlap or do not join");
    joined = fuse->Shape();
  }
  ShapeUpgrade_UnifySameDomain unify(joined, true, true, false);
  unify.Build();
  TopoDS_Shape region = unify.Shape();

  // The edges of the region each original edge ended up as.
  TopTools_DataMapOfShapeInteger final_origin;
  for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(origin); it.More(); it.Next()) {
    TopTools_ListOfShape stage;
    if (fuse) {
      const TopTools_ListOfShape &mods = fuse->Modified(it.Key());
      if (!mods.IsEmpty())
        stage = mods;
      else if (!fuse->IsDeleted(it.Key()))
        stage.Append(it.Key());
    } else {
      stage.Append(it.Key());
    }
    for (const TopoDS_Shape &e : stage) {
      Handle(BRepTools_History) h = unify.History();
      TopTools_ListOfShape images;
      if (!h.IsNull() && h->IsRemoved(e)) continue;
      if (!h.IsNull() && !h->Modified(e).IsEmpty())
        images = h->Modified(e);
      else
        images.Append(e);
      for (const TopoDS_Shape &img : images)
        if (!final_origin.IsBound(img)) final_origin.Bind(img, it.Value());
    }
  }

  int nfaces = 0;
  for (TopExp_Explorer ex(region, TopAbs_FACE); ex.More(); ex.Next()) ++nfaces;
  if (nfaces != 1)
    throw std::runtime_error("the flat pattern is not one connected region; check for overlaps");

  BRepPrimAPI_MakePrism prism(region, gp_Vec(thickness.x, thickness.y, thickness.z), false, true);
  prism.Build();
  if (!prism.IsDone()) throw std::runtime_error("could not thicken the flat pattern");
  TopoDS_Shape out = prism.Shape();
  TopTools_IndexedMapOfShape out_faces;
  TopExp::MapShapes(out, TopAbs_FACE, out_faces);
  sources.clear();
  for (int i = 1; i <= out_faces.Extent(); ++i) sources.push_back(-3);
  TopExp_Explorer fx(region, TopAbs_FACE);
  const TopoDS_Shape &base = fx.Current();
  int a = out_faces.FindIndex(prism.FirstShape(base));
  int b = out_faces.FindIndex(prism.LastShape(base));
  if (a > 0) sources[a - 1] = -1;
  if (b > 0) sources[b - 1] = -2;
  TopTools_IndexedMapOfShape region_edges;
  TopExp::MapShapes(region, TopAbs_EDGE, region_edges);
  for (int i = 1; i <= region_edges.Extent(); ++i) {
    const TopoDS_Shape &e = region_edges(i);
    if (!final_origin.IsBound(e)) continue;
    for (const TopoDS_Shape &g : prism.Generated(e)) {
      int k = out_faces.FindIndex(g);
      if (k > 0) sources[k - 1] = final_origin.Find(e);
    }
  }
  BRepCheck_Analyzer check(out);
  if (!check.IsValid()) throw std::runtime_error("the flat pattern is not a valid solid");
  return std::make_unique<Shape>(out);
}

} // namespace ofk

#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <ShapeFix_Solid.hxx>

namespace ofk {

std::unique_ptr<Shape> polyhedron(rust::Slice<const double> points,
                                  rust::Slice<const uint32_t> faces) {
  auto at = [&](uint32_t i) {
    if (3 * static_cast<size_t>(i) + 2 >= points.size())
      throw std::out_of_range("polyhedron vertex out of range");
    return gp_Pnt(points[3 * i], points[3 * i + 1], points[3 * i + 2]);
  };
  BRepBuilderAPI_Sewing sewing(1e-6);
  size_t i = 0;
  while (i < faces.size()) {
    uint32_t n = faces[i++];
    if (n < 3 || i + n > faces.size()) throw std::invalid_argument("bad polyhedron face");
    BRepBuilderAPI_MakePolygon poly;
    for (uint32_t k = 0; k < n; ++k) poly.Add(at(faces[i + k]));
    poly.Close();
    i += n;
    BRepBuilderAPI_MakeFace mf(poly.Wire(), true);
    if (!mf.IsDone()) throw std::runtime_error("a polyhedron face is not planar");
    sewing.Add(mf.Face());
  }
  sewing.Perform();
  TopoDS_Shape sewn = sewing.SewedShape();
  TopoDS_Shell shell;
  int shells = 0;
  for (TopExp_Explorer ex(sewn, TopAbs_SHELL); ex.More(); ex.Next()) {
    shell = TopoDS::Shell(ex.Current());
    ++shells;
  }
  if (shells != 1) throw std::runtime_error("the faces do not close into one solid");
  BRepBuilderAPI_MakeSolid ms(shell);
  if (!ms.IsDone()) throw std::runtime_error("could not make a solid from the faces");
  ShapeFix_Solid fix(ms.Solid());
  fix.Perform();
  TopoDS_Shape out = fix.Solid();
  BRepCheck_Analyzer check(out);
  if (!check.IsValid()) throw std::runtime_error("the faceted solid is not valid");
  return std::make_unique<Shape>(out);
}

} // namespace ofk

#include <BRepAlgoAPI_Common.hxx>
#include <GeomAPI.hxx>
#include <Geom_TrimmedCurve.hxx>

namespace ofk {

std::unique_ptr<Shape> fold_faces(const Shape &s, rust::Slice<const uint32_t> faces,
                                  rust::Slice<const double> maps, const Frame &frame,
                                  const Shape &region, rust::Vec<uint32_t> &owners) {
  if (maps.size() != faces.size() * 6) throw std::invalid_argument("one map per face");
  gp_Ax3 ax(gp_Pnt(frame.origin.x, frame.origin.y, frame.origin.z),
            gp_Dir(frame.normal.x, frame.normal.y, frame.normal.z),
            gp_Dir(frame.x_dir.x, frame.x_dir.y, frame.x_dir.z));
  Handle(Geom_Plane) plane = new Geom_Plane(ax);
  gp_Pln pln(ax);
  TopoDS_Compound out;
  BRep_Builder builder;
  builder.MakeCompound(out);
  owners.clear();
  for (size_t i = 0; i < faces.size(); ++i) {
    TopoDS_Face face = oriented_face(s.shape, faces[i]);
    Affine m{maps[6 * i], maps[6 * i + 1], maps[6 * i + 2],
             maps[6 * i + 3], maps[6 * i + 4], maps[6 * i + 5]};
    double det = m.a * m.e - m.b * m.d;
    if (std::abs(det) < 1e-15) throw std::invalid_argument("a face map is singular");
    // The inverse map, from the plane back to the face's (u, v).
    Affine inv{m.e / det, -m.b / det, 0, -m.d / det, m.a / det, 0};
    inv.c = -(inv.a * m.c + inv.b * m.f);
    inv.f = -(inv.d * m.c + inv.e * m.f);
    TopoDS_Face image = flat_face(face, m, plane, false);
    BRepAlgoAPI_Common common(region.shape, image);
    common.SetFuzzyValue(1e-7);
    common.Build();
    if (!common.IsDone() || common.HasErrors()) throw std::runtime_error("could not intersect a cut with the flat");
    Handle(Geom_Surface) surface = BRep_Tool::Surface(face);
    for (TopExp_Explorer px(common.Shape(), TopAbs_FACE); px.More(); px.Next()) {
      const TopoDS_Face &piece = TopoDS::Face(px.Current());
      TopoDS_Wire outer = BRepTools::OuterWire(piece);
      TopoDS_Wire outer_new;
      std::vector<TopoDS_Wire> inner;
      for (TopExp_Explorer wx(piece, TopAbs_WIRE); wx.More(); wx.Next()) {
        const TopoDS_Wire &w = TopoDS::Wire(wx.Current());
        BRepBuilderAPI_MakeWire mw;
        for (BRepTools_WireExplorer ex(w, piece); ex.More(); ex.Next()) {
          const TopoDS_Edge &e = ex.Current();
          if (BRep_Tool::Degenerated(e)) continue;
          double f, l, nf, nl;
          Handle(Geom_Curve) c3 = BRep_Tool::Curve(e, f, l);
          if (c3.IsNull()) continue;
          Handle(Geom2d_Curve) c2 = GeomAPI::To2d(new Geom_TrimmedCurve(c3, f, l), pln);
          BRepBuilderAPI_MakeEdge me(mapped(c2, c2->FirstParameter(), c2->LastParameter(), inv, nf, nl),
                                     surface, nf, nl);
          if (!me.IsDone()) throw std::runtime_error("could not map a cut edge onto the sheet");
          TopoDS_Edge ne = me.Edge();
          BRepLib::BuildCurves3d(ne);
          if (e.Orientation() == TopAbs_REVERSED) ne.Reverse();
          mw.Add(ne);
          if (!mw.IsDone()) throw std::runtime_error("a mapped cut boundary is not connected");
        }
        if (!mw.IsDone()) continue;
        if (w.IsSame(outer))
          outer_new = mw.Wire();
        else
          inner.push_back(mw.Wire());
      }
      if (outer_new.IsNull()) continue;
      BRepBuilderAPI_MakeFace mf(surface, outer_new, true);
      for (const TopoDS_Wire &w : inner) mf.Add(w);
      if (!mf.IsDone()) throw std::runtime_error("could not map a cut onto the sheet");
      ShapeFix_Face fixer(mf.Face());
      fixer.Perform();
      TopoDS_Face mapped_face = fixer.Face();
      if (face.Orientation() == TopAbs_REVERSED) mapped_face.Reverse();
      builder.Add(out, mapped_face);
      owners.push_back(faces[i]);
    }
  }
  return std::make_unique<Shape>(out);
}

} // namespace ofk

#include "of-kernel-sys/cpp/ofk.h"
#include "of-kernel-sys/src/lib.rs.h"

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepFilletAPI_MakeChamfer.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepGProp.hxx>
#include <BRepGProp_Face.hxx>
#include <BRepLib.hxx>
#include <BRepLib_ToolTriangulatedShape.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepOffsetAPI_MakeOffsetShape.hxx>
#include <BRepOffsetAPI_MakeThickSolid.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeHalfSpace.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <BRepTools.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <GCPnts_AbscissaPoint.hxx>
#include <GCPnts_TangentialDeflection.hxx>
#include <GC_MakeArcOfCircle.hxx>
#include <GC_MakeSegment.hxx>
#include <GProp_GProps.hxx>
#include <GProp_PrincipalProps.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <Geom_Circle.hxx>
#include <IFSelect_ReturnStatus.hxx>
#include <IGESControl_Controller.hxx>
#include <IGESControl_Reader.hxx>
#include <IGESControl_Writer.hxx>
#include <Interface_Static.hxx>
#include <Poly_Triangulation.hxx>
#include <STEPControl_Reader.hxx>
#include <STEPControl_Writer.hxx>
#include <ShapeFix_Face.hxx>
#include <ShapeFix_Shape.hxx>
#include <StlAPI_Writer.hxx>
#include <Standard_Version.hxx>
#include <TColgp_HArray1OfPnt.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopTools_ListOfShape.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <gp_Ax2.hxx>
#include <gp_Ax3.hxx>
#include <gp_Circ.hxx>
#include <gp_Pln.hxx>
#include <gp_Trsf.hxx>

#include <stdexcept>
#include <string>
#include <vector>

namespace ofk {

namespace {

// Sub-shape kinds shared with Rust: 0 face, 1 edge, 2 vertex, 3 solid.
const TopAbs_ShapeEnum KINDS[4] = {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX, TopAbs_SOLID};

// History relations shared with Rust.
enum Rel : uint8_t { SAME = 0, MODIFIED = 1, GENERATED = 2, FIRST_CAP = 3, LAST_CAP = 4 };

TopAbs_ShapeEnum kind_of(uint8_t kind) {
  if (kind > 3) throw std::invalid_argument("invalid sub-shape kind");
  return KINDS[kind];
}

gp_Pnt pnt(const Vec3 &v) { return gp_Pnt(v.x, v.y, v.z); }
gp_Dir dir(const Vec3 &v) { return gp_Dir(v.x, v.y, v.z); }
Vec3 vec3(const gp_XYZ &p) { return Vec3{p.X(), p.Y(), p.Z()}; }

std::unique_ptr<Shape> wrap(const TopoDS_Shape &s) { return std::make_unique<Shape>(s); }

void require_valid(const TopoDS_Shape &s, const char *what) {
  if (s.IsNull()) throw std::runtime_error(std::string(what) + " produced no geometry");
  BRepCheck_Analyzer check(s);
  if (!check.IsValid()) throw std::runtime_error(std::string(what) + " produced invalid geometry");
}

struct Maps {
  TopTools_IndexedMapOfShape m[4];
  explicit Maps(const TopoDS_Shape &s) {
    for (int k = 0; k < 4; ++k) TopExp::MapShapes(s, KINDS[k], m[k]);
  }
};

void push(rust::Vec<HistoryEntry> &hist, uint8_t input, uint8_t from_kind, int from, uint8_t rel,
          uint8_t to_kind, int to) {
  hist.push_back(HistoryEntry{input, from_kind, static_cast<uint32_t>(from - 1), rel, to_kind,
                              static_cast<uint32_t>(to - 1)});
}

// Record which faces, edges and vertices of the output came from which
// sub-shapes of `input`. This is what lets OpenForge keep stable names for
// faces and edges across rebuilds.
void record_history(BRepBuilderAPI_MakeShape &op, const TopoDS_Shape &input, uint8_t which,
                    const Maps &out, rust::Vec<HistoryEntry> &hist) {
  for (uint8_t k = 0; k < 3; ++k) {
    TopTools_IndexedMapOfShape in;
    TopExp::MapShapes(input, KINDS[k], in);
    for (int i = 1; i <= in.Extent(); ++i) {
      const TopoDS_Shape &s = in(i);
      bool modified = false;
      TopTools_ListOfShape mods = op.Modified(s);
      for (const TopoDS_Shape &m : mods) {
        for (uint8_t k2 = 0; k2 < 3; ++k2) {
          int idx = out.m[k2].FindIndex(m);
          if (idx > 0) {
            push(hist, which, k, i, MODIFIED, k2, idx);
            modified = true;
          }
        }
      }
      if (!modified && !op.IsDeleted(s)) {
        int idx = out.m[k].FindIndex(s);
        if (idx > 0) push(hist, which, k, i, SAME, k, idx);
      }
      TopTools_ListOfShape gens = op.Generated(s);
      for (const TopoDS_Shape &g : gens) {
        for (uint8_t k2 = 0; k2 < 3; ++k2) {
          int idx = out.m[k2].FindIndex(g);
          if (idx > 0) push(hist, which, k, i, GENERATED, k2, idx);
        }
      }
    }
  }
}

gp_Pnt to3d(const gp_Ax3 &ax, double u, double v) {
  gp_XYZ p = ax.Location().XYZ() + ax.XDirection().XYZ() * u + ax.YDirection().XYZ() * v;
  return gp_Pnt(p);
}

gp_Ax3 frame_ax3(const Frame &f) { return gp_Ax3(pnt(f.origin), dir(f.normal), dir(f.x_dir)); }

// Curve kinds shared with Rust (coordinates are 2D, in the frame):
// 0 line [x0 y0 x1 y1], 1 arc counter-clockwise [cx cy sx sy ex ey],
// 2 circle [cx cy r], 3 open interpolated spline [x0 y0 ...],
// 4 closed interpolated spline [x0 y0 ...].
TopoDS_Edge make_edge(const gp_Ax3 &ax, const Curve &c) {
  const auto &d = c.data;
  auto need = [&](size_t n) {
    if (d.size() < n) throw std::invalid_argument("sketch curve has too few values");
  };
  switch (c.kind) {
  case 0: {
    need(4);
    gp_Pnt a = to3d(ax, d[0], d[1]), b = to3d(ax, d[2], d[3]);
    if (a.Distance(b) < Precision::Confusion()) throw std::invalid_argument("zero-length line");
    return BRepBuilderAPI_MakeEdge(GC_MakeSegment(a, b).Value());
  }
  case 1: {
    need(6);
    gp_Pnt c0 = to3d(ax, d[0], d[1]), s = to3d(ax, d[2], d[3]), e = to3d(ax, d[4], d[5]);
    double r = 0.5 * (c0.Distance(s) + c0.Distance(e));
    gp_Ax2 a2(c0, ax.Direction(), ax.XDirection());
    gp_Circ circ(a2, r);
    return BRepBuilderAPI_MakeEdge(GC_MakeArcOfCircle(circ, s, e, true).Value());
  }
  case 2: {
    need(3);
    gp_Ax2 a2(to3d(ax, d[0], d[1]), ax.Direction(), ax.XDirection());
    return BRepBuilderAPI_MakeEdge(gp_Circ(a2, d[2]));
  }
  case 3:
  case 4: {
    size_t n = d.size() / 2;
    if (n < 2) throw std::invalid_argument("spline needs at least two points");
    Handle(TColgp_HArray1OfPnt) pts = new TColgp_HArray1OfPnt(1, static_cast<int>(n));
    for (size_t i = 0; i < n; ++i) pts->SetValue(static_cast<int>(i + 1), to3d(ax, d[2 * i], d[2 * i + 1]));
    GeomAPI_Interpolate interp(pts, c.kind == 4, Precision::Confusion());
    interp.Perform();
    if (!interp.IsDone()) throw std::runtime_error("could not fit spline");
    return BRepBuilderAPI_MakeEdge(interp.Curve());
  }
  default:
    throw std::invalid_argument("unknown sketch curve kind");
  }
}

TopoDS_Wire make_wire_from(const gp_Ax3 &ax, rust::Slice<const Curve> curves, size_t start,
                           size_t n) {
  BRepBuilderAPI_MakeWire mw;
  for (size_t i = start; i < start + n; ++i) {
    mw.Add(make_edge(ax, curves[i]));
    if (!mw.IsDone()) throw std::runtime_error("sketch contour is not connected");
  }
  return mw.Wire();
}

TopoDS_Shape fix(const TopoDS_Shape &s) {
  ShapeFix_Shape fixer(s);
  fixer.Perform();
  return fixer.Shape();
}


} // namespace

std::unique_ptr<Shape> null_shape() { return std::make_unique<Shape>(); }
std::unique_ptr<Shape> clone_shape(const Shape &s) { return wrap(s.shape); }
bool is_null(const Shape &s) { return s.shape.IsNull(); }
bool is_valid(const Shape &s) { return !s.shape.IsNull() && BRepCheck_Analyzer(s.shape).IsValid(); }

uint32_t count(const Shape &s, uint8_t kind) {
  TopTools_IndexedMapOfShape m;
  TopExp::MapShapes(s.shape, kind_of(kind), m);
  return static_cast<uint32_t>(m.Extent());
}

std::unique_ptr<Shape> sub_shape(const Shape &s, uint8_t kind, uint32_t index) {
  TopTools_IndexedMapOfShape m;
  TopExp::MapShapes(s.shape, kind_of(kind), m);
  if (index >= static_cast<uint32_t>(m.Extent())) throw std::out_of_range("sub-shape index out of range");
  return wrap(m(static_cast<int>(index) + 1));
}

std::unique_ptr<Shape> compound_new() {
  TopoDS_Compound c;
  BRep_Builder().MakeCompound(c);
  return wrap(c);
}

void compound_add(Shape &compound, const Shape &s) { BRep_Builder().Add(compound.shape, s.shape); }

std::unique_ptr<Shape> make_box(double dx, double dy, double dz) {
  return wrap(BRepPrimAPI_MakeBox(dx, dy, dz).Shape());
}

std::unique_ptr<Shape> make_faces(const Frame &frame, rust::Slice<const Curve> curves,
                                  rust::Slice<const uint32_t> loop_sizes,
                                  rust::Slice<const uint32_t> face_loops) {
  gp_Ax3 ax = frame_ax3(frame);
  gp_Pln pln(ax);
  std::vector<TopoDS_Wire> wires;
  size_t start = 0;
  for (uint32_t n : loop_sizes) {
    if (start + n > curves.size()) throw std::invalid_argument("loop sizes exceed curve count");
    wires.push_back(make_wire_from(ax, curves, start, n));
    start += n;
  }
  TopoDS_Compound result;
  BRep_Builder builder;
  builder.MakeCompound(result);
  size_t w = 0;
  int faces = 0;
  for (uint32_t nloops : face_loops) {
    if (nloops == 0 || w + nloops > wires.size()) throw std::invalid_argument("bad face loop counts");
    BRepBuilderAPI_MakeFace mf(pln, wires[w], true);
    for (uint32_t i = 1; i < nloops; ++i) mf.Add(TopoDS::Wire(wires[w + i].Reversed()));
    if (!mf.IsDone()) throw std::runtime_error("could not build a face from the sketch");
    ShapeFix_Face ff(mf.Face());
    ff.FixOrientation();
    ff.Perform();
    builder.Add(result, ff.Face());
    w += nloops;
    ++faces;
  }
  if (faces == 1) {
    TopExp_Explorer ex(result, TopAbs_FACE);
    return wrap(ex.Current());
  }
  return wrap(result);
}

std::unique_ptr<Shape> make_wire(const Frame &frame, rust::Slice<const Curve> curves) {
  return wrap(make_wire_from(frame_ax3(frame), curves, 0, curves.size()));
}

std::unique_ptr<Shape> half_space(const Shape &face, const Vec3 &inside) {
  TopExp_Explorer ex(face.shape, TopAbs_FACE);
  if (!ex.More()) throw std::invalid_argument("half space needs a face");
  return wrap(BRepPrimAPI_MakeHalfSpace(TopoDS::Face(ex.Current()), pnt(inside)).Solid());
}

std::unique_ptr<Shape> prism(const Shape &s, const Vec3 &d, rust::Vec<HistoryEntry> &hist) {
  BRepPrimAPI_MakePrism op(s.shape, gp_Vec(d.x, d.y, d.z), false, true);
  op.Build();
  if (!op.IsDone()) throw std::runtime_error("extrude failed");
  TopoDS_Shape out = op.Shape();
  require_valid(out, "extrude");
  Maps maps(out);
  record_history(op, s.shape, 0, maps, hist);
  TopTools_IndexedMapOfShape in;
  TopExp::MapShapes(s.shape, TopAbs_FACE, in);
  for (int i = 1; i <= in.Extent(); ++i) {
    int a = maps.m[0].FindIndex(op.FirstShape(in(i)));
    int b = maps.m[0].FindIndex(op.LastShape(in(i)));
    if (a > 0) push(hist, 0, 0, i, FIRST_CAP, 0, a);
    if (b > 0) push(hist, 0, 0, i, LAST_CAP, 0, b);
  }
  return wrap(out);
}

std::unique_ptr<Shape> revol(const Shape &s, const Vec3 &origin, const Vec3 &d, double angle,
                             rust::Vec<HistoryEntry> &hist) {
  BRepPrimAPI_MakeRevol op(s.shape, gp_Ax1(pnt(origin), dir(d)), angle, false);
  op.Build();
  if (!op.IsDone()) throw std::runtime_error("revolve failed");
  TopoDS_Shape out = op.Shape();
  require_valid(out, "revolve");
  Maps maps(out);
  record_history(op, s.shape, 0, maps, hist);
  TopTools_IndexedMapOfShape in;
  TopExp::MapShapes(s.shape, TopAbs_FACE, in);
  for (int i = 1; i <= in.Extent(); ++i) {
    int a = maps.m[0].FindIndex(op.FirstShape(in(i)));
    int b = maps.m[0].FindIndex(op.LastShape(in(i)));
    if (a > 0) push(hist, 0, 0, i, FIRST_CAP, 0, a);
    if (b > 0 && b != a) push(hist, 0, 0, i, LAST_CAP, 0, b);
  }
  return wrap(out);
}

std::unique_ptr<Shape> boolean(uint8_t kind, const Shape &a, const Shape &b,
                               rust::Vec<HistoryEntry> &hist) {
  std::unique_ptr<BRepAlgoAPI_BooleanOperation> op;
  switch (kind) {
  case 0: op = std::make_unique<BRepAlgoAPI_Fuse>(); break;
  case 1: op = std::make_unique<BRepAlgoAPI_Cut>(); break;
  case 2: op = std::make_unique<BRepAlgoAPI_Common>(); break;
  default: throw std::invalid_argument("unknown boolean operation");
  }
  TopTools_ListOfShape args, tools;
  args.Append(a.shape);
  tools.Append(b.shape);
  op->SetArguments(args);
  op->SetTools(tools);
  op->SetRunParallel(true);
  op->SetNonDestructive(true);
  op->Build();
  if (!op->IsDone() || op->HasErrors()) throw std::runtime_error("boolean operation failed");
  op->SimplifyResult(true, true);
  TopoDS_Shape out = op->Shape();
  if (out.IsNull()) throw std::runtime_error("boolean operation produced no geometry");
  Maps maps(out);
  record_history(*op, a.shape, 0, maps, hist);
  record_history(*op, b.shape, 1, maps, hist);
  return wrap(out);
}

std::unique_ptr<Shape> fillet(const Shape &s, rust::Slice<const uint32_t> edges, double radius,
                              rust::Vec<HistoryEntry> &hist) {
  TopTools_IndexedMapOfShape em;
  TopExp::MapShapes(s.shape, TopAbs_EDGE, em);
  BRepFilletAPI_MakeFillet op(s.shape);
  for (uint32_t e : edges) {
    if (e >= static_cast<uint32_t>(em.Extent())) throw std::out_of_range("fillet edge out of range");
    op.Add(radius, TopoDS::Edge(em(static_cast<int>(e) + 1)));
  }
  op.Build();
  if (!op.IsDone()) throw std::runtime_error("fillet failed; the radius may be too large for the selected edges");
  TopoDS_Shape out = op.Shape();
  require_valid(out, "fillet");
  record_history(op, s.shape, 0, Maps(out), hist);
  return wrap(out);
}

std::unique_ptr<Shape> chamfer(const Shape &s, rust::Slice<const uint32_t> edges, double d1,
                               double d2, rust::Vec<HistoryEntry> &hist) {
  TopTools_IndexedMapOfShape em;
  TopExp::MapShapes(s.shape, TopAbs_EDGE, em);
  TopTools_IndexedDataMapOfShapeListOfShape ef;
  TopExp::MapShapesAndAncestors(s.shape, TopAbs_EDGE, TopAbs_FACE, ef);
  BRepFilletAPI_MakeChamfer op(s.shape);
  for (uint32_t e : edges) {
    if (e >= static_cast<uint32_t>(em.Extent())) throw std::out_of_range("chamfer edge out of range");
    const TopoDS_Edge &edge = TopoDS::Edge(em(static_cast<int>(e) + 1));
    const TopTools_ListOfShape &faces = ef.FindFromKey(edge);
    if (faces.IsEmpty()) throw std::runtime_error("chamfer edge has no adjacent face");
    op.Add(d1, d2, edge, TopoDS::Face(faces.First()));
  }
  op.Build();
  if (!op.IsDone()) throw std::runtime_error("chamfer failed; the distance may be too large for the selected edges");
  TopoDS_Shape out = op.Shape();
  require_valid(out, "chamfer");
  record_history(op, s.shape, 0, Maps(out), hist);
  return wrap(out);
}

std::unique_ptr<Shape> shell(const Shape &s, rust::Slice<const uint32_t> faces, double thickness,
                             rust::Vec<HistoryEntry> &hist) {
  TopTools_IndexedMapOfShape fm;
  TopExp::MapShapes(s.shape, TopAbs_FACE, fm);
  if (faces.empty()) {
    // No faces removed: hollow the part with a closed inner void.
    BRepOffsetAPI_MakeOffsetShape off;
    off.PerformByJoin(s.shape, -thickness, 1e-4, BRepOffset_Skin, false, false, GeomAbs_Intersection);
    if (!off.IsDone()) throw std::runtime_error("shell failed; the thickness may be too large");
    TopoDS_Shape inner = off.Shape();
    TopExp_Explorer ex(inner, TopAbs_SHELL);
    if (!ex.More()) throw std::runtime_error("shell failed");
    BRepBuilderAPI_MakeSolid ms(TopoDS::Shell(ex.Current()));
    TopoDS_Solid void_solid = ms.Solid();
    BRepLib::OrientClosedSolid(void_solid);
    BRepAlgoAPI_Cut cut(s.shape, void_solid);
    if (!cut.IsDone()) throw std::runtime_error("shell failed");
    TopoDS_Shape out = cut.Shape();
    Maps maps(out);
    record_history(cut, s.shape, 0, maps, hist);
    return wrap(out);
  }
  TopTools_ListOfShape remove;
  for (uint32_t f : faces) {
    if (f >= static_cast<uint32_t>(fm.Extent())) throw std::out_of_range("shell face out of range");
    remove.Append(fm(static_cast<int>(f) + 1));
  }
  BRepOffsetAPI_MakeThickSolid op;
  op.MakeThickSolidByJoin(s.shape, remove, -thickness, 1e-4, BRepOffset_Skin, false, false,
                          GeomAbs_Intersection);
  op.Build();
  if (!op.IsDone()) throw std::runtime_error("shell failed; the thickness may be too large");
  TopoDS_Shape out = op.Shape();
  require_valid(out, "shell");
  record_history(op, s.shape, 0, Maps(out), hist);
  return wrap(out);
}

std::unique_ptr<Shape> transform(const Shape &s, rust::Slice<const double> m,
                                 rust::Vec<HistoryEntry> &hist) {
  if (m.size() != 12) throw std::invalid_argument("transform needs a 3x4 matrix");
  gp_Trsf t;
  t.SetValues(m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8], m[9], m[10], m[11]);
  BRepBuilderAPI_Transform op(s.shape, t, true);
  TopoDS_Shape out = op.Shape();
  record_history(op, s.shape, 0, Maps(out), hist);
  return wrap(out);
}

std::unique_ptr<Shape> mirror(const Shape &s, const Vec3 &origin, const Vec3 &normal,
                              rust::Vec<HistoryEntry> &hist) {
  gp_Trsf t;
  t.SetMirror(gp_Ax2(pnt(origin), dir(normal)));
  BRepBuilderAPI_Transform op(s.shape, t, true);
  TopoDS_Shape out = op.Shape();
  record_history(op, s.shape, 0, Maps(out), hist);
  return wrap(out);
}

rust::Vec<uint32_t> edge_faces(const Shape &s) {
  TopTools_IndexedMapOfShape fm;
  TopExp::MapShapes(s.shape, TopAbs_FACE, fm);
  TopTools_IndexedDataMapOfShapeListOfShape ef;
  TopExp::MapShapesAndUniqueAncestors(s.shape, TopAbs_EDGE, TopAbs_FACE, ef);
  TopTools_IndexedMapOfShape em;
  TopExp::MapShapes(s.shape, TopAbs_EDGE, em);
  rust::Vec<uint32_t> out;
  for (int i = 1; i <= em.Extent(); ++i) {
    const TopTools_ListOfShape *faces = ef.Seek(em(i));
    out.push_back(faces ? static_cast<uint32_t>(faces->Extent()) : 0);
    if (faces)
      for (const TopoDS_Shape &f : *faces) out.push_back(static_cast<uint32_t>(fm.FindIndex(f) - 1));
  }
  return out;
}

rust::Vec<uint32_t> face_edges(const Shape &s) {
  TopTools_IndexedMapOfShape fm, em;
  TopExp::MapShapes(s.shape, TopAbs_FACE, fm);
  TopExp::MapShapes(s.shape, TopAbs_EDGE, em);
  rust::Vec<uint32_t> out;
  for (int i = 1; i <= fm.Extent(); ++i) {
    TopTools_IndexedMapOfShape fe;
    TopExp::MapShapes(fm(i), TopAbs_EDGE, fe);
    out.push_back(static_cast<uint32_t>(fe.Extent()));
    for (int j = 1; j <= fe.Extent(); ++j) out.push_back(static_cast<uint32_t>(em.FindIndex(fe(j)) - 1));
  }
  return out;
}

Mesh tessellate(const Shape &s, double linear, double angular) {
  BRepMesh_IncrementalMesh mesher(s.shape, linear, false, angular, true);
  Mesh mesh;
  TopTools_IndexedMapOfShape fm, em;
  TopExp::MapShapes(s.shape, TopAbs_FACE, fm);
  TopExp::MapShapes(s.shape, TopAbs_EDGE, em);
  uint32_t base = 0;
  mesh.face_tri_start.push_back(0);
  for (int i = 1; i <= fm.Extent(); ++i) {
    const TopoDS_Face &face = TopoDS::Face(fm(i));
    TopLoc_Location loc;
    Handle(Poly_Triangulation) tri = BRep_Tool::Triangulation(face, loc);
    if (!tri.IsNull()) {
      if (!tri->HasNormals()) BRepLib_ToolTriangulatedShape::ComputeNormals(face, tri);
      const gp_Trsf &tr = loc.Transformation();
      const bool rev = face.Orientation() == TopAbs_REVERSED;
      for (int n = 1; n <= tri->NbNodes(); ++n) {
        gp_Pnt p = tri->Node(n).Transformed(tr);
        gp_Dir nr = tri->Normal(n);
        nr.Transform(tr);
        if (rev) nr.Reverse();
        mesh.positions.push_back(static_cast<float>(p.X()));
        mesh.positions.push_back(static_cast<float>(p.Y()));
        mesh.positions.push_back(static_cast<float>(p.Z()));
        mesh.normals.push_back(static_cast<float>(nr.X()));
        mesh.normals.push_back(static_cast<float>(nr.Y()));
        mesh.normals.push_back(static_cast<float>(nr.Z()));
      }
      for (int t = 1; t <= tri->NbTriangles(); ++t) {
        int a, b, c;
        tri->Triangle(t).Get(a, b, c);
        if (rev) std::swap(b, c);
        mesh.indices.push_back(base + a - 1);
        mesh.indices.push_back(base + b - 1);
        mesh.indices.push_back(base + c - 1);
      }
      base += static_cast<uint32_t>(tri->NbNodes());
    }
    mesh.face_tri_start.push_back(static_cast<uint32_t>(mesh.indices.size() / 3));
  }
  mesh.edge_start.push_back(0);
  for (int i = 1; i <= em.Extent(); ++i) {
    const TopoDS_Edge &edge = TopoDS::Edge(em(i));
    if (!BRep_Tool::Degenerated(edge)) {
      BRepAdaptor_Curve c(edge);
      GCPnts_TangentialDeflection pts(c, angular, linear, 2);
      for (int j = 1; j <= pts.NbPoints(); ++j) {
        gp_Pnt p = pts.Value(j);
        mesh.edge_points.push_back(static_cast<float>(p.X()));
        mesh.edge_points.push_back(static_cast<float>(p.Y()));
        mesh.edge_points.push_back(static_cast<float>(p.Z()));
      }
    }
    mesh.edge_start.push_back(static_cast<uint32_t>(mesh.edge_points.size() / 3));
  }
  return mesh;
}

rust::Vec<double> edge_samples(const Shape &s, uint32_t edge, double linear, double angular) {
  auto e = sub_shape(s, 1, edge);
  rust::Vec<double> out;
  if (BRep_Tool::Degenerated(TopoDS::Edge(e->shape))) return out;
  BRepAdaptor_Curve c(TopoDS::Edge(e->shape));
  GCPnts_TangentialDeflection pts(c, angular, linear, 2);
  for (int j = 1; j <= pts.NbPoints(); ++j) {
    gp_Pnt p = pts.Value(j);
    out.push_back(p.X());
    out.push_back(p.Y());
    out.push_back(p.Z());
  }
  return out;
}

MassProps mass_props(const Shape &s) {
  GProp_GProps vp, sp;
  BRepGProp::VolumeProperties(s.shape, vp);
  BRepGProp::SurfaceProperties(s.shape, sp);
  MassProps out{};
  out.volume = vp.Mass();
  out.area = sp.Mass();
  out.center = vec3(vp.CentreOfMass().XYZ());
  gp_Mat m = vp.MatrixOfInertia();
  for (int r = 1; r <= 3; ++r)
    for (int c = 1; c <= 3; ++c) out.inertia.push_back(m(r, c));
  GProp_PrincipalProps pp = vp.PrincipalProperties();
  double i1, i2, i3;
  pp.Moments(i1, i2, i3);
  out.principal_moments = Vec3{i1, i2, i3};
  out.principal_axes.push_back(vec3(pp.FirstAxisOfInertia().XYZ()));
  out.principal_axes.push_back(vec3(pp.SecondAxisOfInertia().XYZ()));
  out.principal_axes.push_back(vec3(pp.ThirdAxisOfInertia().XYZ()));
  return out;
}

BBox bbox(const Shape &s) {
  Bnd_Box b;
  BRepBndLib::Add(s.shape, b, false);
  BBox out{};
  if (b.IsVoid()) return out;
  double x0, y0, z0, x1, y1, z1;
  b.Get(x0, y0, z0, x1, y1, z1);
  out.min = Vec3{x0, y0, z0};
  out.max = Vec3{x1, y1, z1};
  out.valid = true;
  return out;
}

FaceInfo face_info(const Shape &s, uint32_t index) {
  auto sub = sub_shape(s, 0, index);
  const TopoDS_Face &face = TopoDS::Face(sub->shape);
  // Orientation is lost when the face is taken out of the map; restore it
  // from the face as it sits in the solid.
  TopoDS_Face oriented = face;
  for (TopExp_Explorer ex(s.shape, TopAbs_FACE); ex.More(); ex.Next())
    if (ex.Current().IsSame(face)) {
      oriented = TopoDS::Face(ex.Current());
      break;
    }
  FaceInfo out{};
  GProp_GProps sp;
  BRepGProp::SurfaceProperties(oriented, sp);
  out.area = sp.Mass();
  out.centroid = vec3(sp.CentreOfMass().XYZ());
  BRepAdaptor_Surface su(oriented);
  // Normal at the middle of the face's parameter range.
  double u0, u1, v0, v1;
  BRepTools::UVBounds(oriented, u0, u1, v0, v1);
  BRepGProp_Face gf(oriented);
  gp_Pnt p;
  gp_Vec n;
  gf.Normal(0.5 * (u0 + u1), 0.5 * (v0 + v1), p, n);
  if (n.Magnitude() > 1e-12) n.Normalize();
  out.normal = Vec3{n.X(), n.Y(), n.Z()};
  out.point = vec3(p.XYZ());
  switch (su.GetType()) {
  case GeomAbs_Plane: {
    out.kind = 1;
    gp_Pln pl = su.Plane();
    out.axis_origin = vec3(pl.Location().XYZ());
    out.x_dir = vec3(pl.XAxis().Direction().XYZ());
    break;
  }
  case GeomAbs_Cylinder: {
    out.kind = 2;
    gp_Cylinder c = su.Cylinder();
    out.axis_origin = vec3(c.Axis().Location().XYZ());
    out.axis_dir = vec3(c.Axis().Direction().XYZ());
    out.radius = c.Radius();
    break;
  }
  case GeomAbs_Cone: {
    out.kind = 3;
    gp_Cone c = su.Cone();
    out.axis_origin = vec3(c.Axis().Location().XYZ());
    out.axis_dir = vec3(c.Axis().Direction().XYZ());
    out.radius = c.RefRadius();
    break;
  }
  case GeomAbs_Sphere: {
    out.kind = 4;
    gp_Sphere sph = su.Sphere();
    out.axis_origin = vec3(sph.Location().XYZ());
    out.radius = sph.Radius();
    break;
  }
  case GeomAbs_Torus: {
    out.kind = 5;
    gp_Torus t = su.Torus();
    out.axis_origin = vec3(t.Axis().Location().XYZ());
    out.axis_dir = vec3(t.Axis().Direction().XYZ());
    out.radius = t.MajorRadius();
    break;
  }
  default:
    out.kind = 0;
  }
  return out;
}

EdgeInfo edge_info(const Shape &s, uint32_t index) {
  auto sub = sub_shape(s, 1, index);
  const TopoDS_Edge &edge = TopoDS::Edge(sub->shape);
  EdgeInfo out{};
  if (BRep_Tool::Degenerated(edge)) {
    out.kind = 255;
    return out;
  }
  BRepAdaptor_Curve c(edge);
  double f = c.FirstParameter(), l = c.LastParameter();
  out.length = GCPnts_AbscissaPoint::Length(c);
  out.start = vec3(c.Value(f).XYZ());
  out.end = vec3(c.Value(l).XYZ());
  out.mid = vec3(c.Value(0.5 * (f + l)).XYZ());
  out.closed = BRep_Tool::IsClosed(edge) || c.Value(f).Distance(c.Value(l)) < Precision::Confusion();
  switch (c.GetType()) {
  case GeomAbs_Line:
    out.kind = 1;
    out.axis = vec3(c.Line().Direction().XYZ());
    break;
  case GeomAbs_Circle: {
    out.kind = 2;
    gp_Circ circ = c.Circle();
    out.center = vec3(circ.Location().XYZ());
    out.axis = vec3(circ.Axis().Direction().XYZ());
    out.radius = circ.Radius();
    break;
  }
  case GeomAbs_Ellipse: out.kind = 3; break;
  default: out.kind = 0;
  }
  return out;
}

Vec3 vertex_point(const Shape &s, uint32_t index) {
  auto sub = sub_shape(s, 2, index);
  return vec3(BRep_Tool::Pnt(TopoDS::Vertex(sub->shape)).XYZ());
}

Distance distance(const Shape &a, const Shape &b) {
  BRepExtrema_DistShapeShape d(a.shape, b.shape);
  if (!d.IsDone() || d.NbSolution() == 0) throw std::runtime_error("could not measure distance");
  return Distance{d.Value(), vec3(d.PointOnShape1(1).XYZ()), vec3(d.PointOnShape2(1).XYZ())};
}

void write_step(const Shape &s, rust::Str path) {
  STEPControl_Writer w;
  Interface_Static::SetCVal("write.step.unit", "MM");
  Interface_Static::SetCVal("write.step.product.name", "OpenForge part");
  if (w.Transfer(s.shape, STEPControl_AsIs) != IFSelect_RetDone) throw std::runtime_error("could not convert part to STEP");
  if (w.Write(std::string(path).c_str()) != IFSelect_RetDone) throw std::runtime_error("could not write STEP file");
}

std::unique_ptr<Shape> read_step(rust::Str path) {
  STEPControl_Reader r;
  if (r.ReadFile(std::string(path).c_str()) != IFSelect_RetDone) throw std::runtime_error("could not read STEP file");
  r.TransferRoots();
  TopoDS_Shape s = r.OneShape();
  if (s.IsNull()) throw std::runtime_error("STEP file contains no geometry");
  return wrap(fix(s));
}

void write_iges(const Shape &s, rust::Str path) {
  IGESControl_Controller::Init();
  IGESControl_Writer w("MM", 1);
  if (!w.AddShape(s.shape)) throw std::runtime_error("could not convert part to IGES");
  w.ComputeModel();
  if (!w.Write(std::string(path).c_str())) throw std::runtime_error("could not write IGES file");
}

std::unique_ptr<Shape> read_iges(rust::Str path) {
  IGESControl_Controller::Init();
  IGESControl_Reader r;
  if (r.ReadFile(std::string(path).c_str()) != IFSelect_RetDone) throw std::runtime_error("could not read IGES file");
  r.TransferRoots();
  TopoDS_Shape s = r.OneShape();
  if (s.IsNull()) throw std::runtime_error("IGES file contains no geometry");
  return wrap(fix(s));
}

void write_stl(const Shape &s, rust::Str path, double linear, double angular, bool ascii) {
  BRepMesh_IncrementalMesh mesher(s.shape, linear, false, angular, true);
  StlAPI_Writer w;
  w.ASCIIMode() = ascii;
  if (!w.Write(s.shape, std::string(path).c_str())) throw std::runtime_error("could not write STL file");
}

} // namespace ofk

#include "of-kernel-sys/cpp/ofk.h"
#include "of-kernel-sys/src/lib.rs.h"

#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepAlgoAPI_Section.hxx>
#include <BRepAlgoAPI_Splitter.hxx>
#include <BOPAlgo_ArgumentAnalyzer.hxx>
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
#include <Geom2d_BSplineCurve.hxx>
#include <mutex>
#include <TColgp_HArray1OfPnt.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <BRepAdaptor_CompCurve.hxx>
#include <BRepLib_ToolTriangulatedShape.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepOffsetAPI_MakeOffsetShape.hxx>
#include <BRepOffsetAPI_MakePipeShell.hxx>
#include <BRepOffsetAPI_DraftAngle.hxx>
#include <TColgp_Array1OfPnt2d.hxx>
#include <BRepOffsetAPI_ThruSections.hxx>
#include <BRepProj_Projection.hxx>
#include <BRepFeat_SplitShape.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom2d_Line.hxx>
#include <gp_Dir2d.hxx>
#include <gp_Pnt2d.hxx>
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
#include <Geom_Circle.hxx>
#include <Geom_BezierCurve.hxx>
#include <Geom_BSplineCurve.hxx>
#include <TColgp_Array1OfPnt.hxx>
#include <TColStd_Array1OfReal.hxx>
#include <TColStd_Array1OfInteger.hxx>
#include <Geom_OffsetSurface.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <GeomLProp_SLProps.hxx>
#include <IFSelect_ReturnStatus.hxx>
#include <Message.hxx>
#include <Message_Messenger.hxx>
#include <Message_PrinterOStream.hxx>
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
#include <cmath>
#include <algorithm>
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
    if (s.Distance(e) < Precision::Confusion()) {
      // An arc ending where it starts is a full turn from that point.
      if (r < Precision::Confusion()) throw std::invalid_argument("zero-radius arc");
      return BRepBuilderAPI_MakeEdge(gp_Circ(gp_Ax2(c0, ax.Direction(), gp_Dir(gp_Vec(c0, s))), r));
    }
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
    const bool closed = c.kind == 4;
    auto get = [&](int i) {
      if (closed) i = (i % static_cast<int>(n) + static_cast<int>(n)) % static_cast<int>(n);
      if (i < 0) return to3d(ax, 2*d[0]-d[2], 2*d[1]-d[3]).XYZ();
      if (i >= static_cast<int>(n)) return to3d(ax, 2*d[2*n-2]-d[2*n-4], 2*d[2*n-1]-d[2*n-3]).XYZ();
      return to3d(ax, d[2*i], d[2*i+1]).XYZ();
    };
    const int spans = static_cast<int>(closed ? n : n-1);
    TColgp_Array1OfPnt poles(1, spans*3+1);
    TColStd_Array1OfReal knots(1, spans+1);
    TColStd_Array1OfInteger mults(1, spans+1);
    for (int i = 0; i < spans; ++i) {
      const gp_XYZ a = get(i), b = get(i+1);
      poles.SetValue(3*i+1, gp_Pnt(a));
      poles.SetValue(3*i+2, gp_Pnt(a+(b-get(i-1))/6.0));
      poles.SetValue(3*i+3, gp_Pnt(b-(get(i+2)-a)/6.0));
    }
    poles.SetValue(spans*3+1, gp_Pnt(get(spans)));
    for (int i = 0; i <= spans; ++i) {
      knots.SetValue(i+1, i);
      mults.SetValue(i+1, i == 0 || i == spans ? 4 : 3);
    }
    Handle(Geom_BSplineCurve) curve = new Geom_BSplineCurve(poles, knots, mults, 3);
    return BRepBuilderAPI_MakeEdge(curve);
  }
  case 5: {
    need(8);
    TColgp_Array1OfPnt poles(1,4);
    for (int i = 0; i < 4; ++i) poles.SetValue(i+1, to3d(ax,d[2*i],d[2*i+1]));
    Handle(Geom_BezierCurve) curve = new Geom_BezierCurve(poles);
    return BRepBuilderAPI_MakeEdge(curve);
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

// Data exchange prints progress to stdout, which the core uses for its
// protocol. Silence it once.
void quiet() {
  static bool done = false;
  if (!done) {
    Message::DefaultMessenger()->RemovePrinters(STANDARD_TYPE(Message_PrinterOStream));
    done = true;
  }
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

std::shared_ptr<const TopTools_IndexedMapOfShape> Shape::map(TopAbs_ShapeEnum kind) const {
  std::lock_guard<std::mutex> guard(lock);
  if (!keyed.IsEqual(shape)) {
    for (auto &m : maps) m.reset();
    keyed = shape;
  }
  auto &m = maps[kind];
  if (!m) {
    auto built = std::make_shared<TopTools_IndexedMapOfShape>();
    TopExp::MapShapes(shape, kind, *built);
    m = built;
  }
  return m;
}

void Shape::forget() const {
  std::lock_guard<std::mutex> guard(lock);
  for (auto &m : maps) m.reset();
  keyed.Nullify();
}

uint32_t count(const Shape &s, uint8_t kind) { return static_cast<uint32_t>(s.map(kind_of(kind))->Extent()); }

std::unique_ptr<Shape> sub_shape(const Shape &s, uint8_t kind, uint32_t index) {
  auto m = s.map(kind_of(kind));
  if (index >= static_cast<uint32_t>(m->Extent())) throw std::out_of_range("sub-shape index out of range");
  return wrap((*m)(static_cast<int>(index) + 1));
}

std::unique_ptr<Shape> compound_new() {
  TopoDS_Compound c;
  BRep_Builder().MakeCompound(c);
  return wrap(c);
}

void compound_add(Shape &compound, const Shape &s) {
  BRep_Builder().Add(compound.shape, s.shape);
  compound.forget();
}

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

std::unique_ptr<Shape> make_wire3(rust::Slice<const Curve> curves) {
  if (curves.empty()) throw std::invalid_argument("path needs at least one curve");
  BRepBuilderAPI_MakeWire wire;
  for (const auto &curve : curves) {
    for (double value : curve.data) if (!std::isfinite(value)) throw std::invalid_argument("curve coordinates must be finite");
    if (curve.kind == 0 && curve.data.size() == 6) {
      const auto &p = curve.data;
      gp_Pnt a(p[0],p[1],p[2]), b(p[3],p[4],p[5]);
      if (a.Distance(b) < 1e-9) throw std::invalid_argument("line has no length");
      wire.Add(BRepBuilderAPI_MakeEdge(a,b).Edge());
    } else if (curve.kind == 1 && curve.data.size() == 12) {
      TColgp_Array1OfPnt points(1,4);
      for (int i=0;i<4;++i) points(i+1)=gp_Pnt(curve.data[3*i],curve.data[3*i+1],curve.data[3*i+2]);
      wire.Add(BRepBuilderAPI_MakeEdge(new Geom_BezierCurve(points)).Edge());
    } else throw std::invalid_argument("unsupported 3D curve");
    if (!wire.IsDone()) throw std::invalid_argument("path curves must form a connected chain");
  }
  require_valid(wire.Wire(), "3D path");
  return wrap(wire.Wire());
}

std::unique_ptr<Shape> join_wires(const Shape &curves) {
  if (curves.shape.ShapeType()==TopAbs_WIRE) { require_valid(curves.shape,"path"); return wrap(curves.shape); }
  BRepBuilderAPI_MakeWire wire;
  int count=0;
  for (TopExp_Explorer ex(curves.shape, TopAbs_EDGE); ex.More(); ex.Next()) {
    wire.Add(TopoDS::Edge(ex.Current()));
    if (!wire.IsDone()) throw std::invalid_argument("selected curves are disconnected");
    ++count;
  }
  if (!count) throw std::invalid_argument("select at least one curve");
  require_valid(wire.Wire(), "composite curve");
  TopTools_IndexedDataMapOfShapeListOfShape adjacency;
  TopExp::MapShapesAndUniqueAncestors(wire.Wire(), TopAbs_VERTEX, TopAbs_EDGE, adjacency);
  for (int i=1;i<=adjacency.Extent();++i) {
    if (adjacency.FindFromIndex(i).Extent()>2) throw std::invalid_argument("selected curves form a branch");
  }
  return wrap(wire.Wire());
}

std::unique_ptr<Shape> reverse_shape(const Shape &shape) { return wrap(shape.shape.Reversed()); }

std::unique_ptr<Shape> make_helix(const Frame &frame, double radius, double pitch, double turns, double start, bool left) {
  if (!(radius>0 && pitch>0 && turns>0) || !std::isfinite(radius+pitch+turns+start) || turns>10000)
    throw std::invalid_argument("helix needs positive finite radius/pitch/turns (at most 10000 turns)");
  const double angle=turns*2*std::acos(-1.0)*(left?-1:1), height=turns*pitch;
  Handle(Geom_CylindricalSurface) surface=new Geom_CylindricalSurface(frame_ax3(frame), radius);
  Handle(Geom2d_Line) curve=new Geom2d_Line(gp_Pnt2d(start,0),gp_Dir2d(angle,height));
  TopoDS_Edge edge=BRepBuilderAPI_MakeEdge(curve,surface,0,std::hypot(angle,height)).Edge();
  if (!BRepLib::BuildCurves3d(edge,1e-7)) throw std::runtime_error("helix approximation failed");
  TopoDS_Wire wire=BRepBuilderAPI_MakeWire(edge).Wire();
  require_valid(wire,"helix");
  return wrap(wire);
}

// A helix whose pitch changes linearly with turn count between table rows. In the
// cylinder's (angle, height) parameters each span is a quadratic, so the curve is
// an exact degree-2 B-spline there.
std::unique_ptr<Shape> make_helix_law(const Frame &frame, double radius, rust::Slice<const double> turns,
                                      rust::Slice<const double> pitches, double start, bool left) {
  const size_t n=turns.size();
  if (n<2 || pitches.size()!=n || !(radius>0) || !std::isfinite(radius+start))
    throw std::invalid_argument("a variable helix needs a positive radius and at least two pitch rows");
  if (turns[0]!=0) throw std::invalid_argument("the first pitch row must be at turn 0");
  for (size_t i=0;i<n;++i) {
    if (!std::isfinite(turns[i]+pitches[i]) || !(pitches[i]>0)) throw std::invalid_argument("helix pitches must be positive and finite");
    if (i && !(turns[i]>turns[i-1])) throw std::invalid_argument("helix pitch rows must have increasing turn counts");
  }
  if (turns[n-1]>10000) throw std::invalid_argument("helix needs at most 10000 turns");
  const double tau=2*std::acos(-1.0)*(left?-1:1);
  const int spans=static_cast<int>(n-1);
  TColgp_Array1OfPnt2d poles(1,2*spans+1);
  TColStd_Array1OfReal knots(1,spans+1);
  TColStd_Array1OfInteger mults(1,spans+1);
  double z=0;
  for (int i=0;i<spans;++i) {
    const double t0=turns[i], dt=turns[i+1]-turns[i];
    const double rise=(pitches[i]+pitches[i+1])/2*dt;
    poles(2*i+1)=gp_Pnt2d(start+tau*t0,z);
    poles(2*i+2)=gp_Pnt2d(start+tau*(t0+dt/2),z+pitches[i]*dt/2);
    z+=rise;
    knots(i+1)=t0;
    mults(i+1)=i==0?3:2;
  }
  poles(2*spans+1)=gp_Pnt2d(start+tau*turns[n-1],z);
  knots(spans+1)=turns[n-1];
  mults(spans+1)=3;
  Handle(Geom2d_BSplineCurve) law=new Geom2d_BSplineCurve(poles,knots,mults,2);
  Handle(Geom_CylindricalSurface) surface=new Geom_CylindricalSurface(frame_ax3(frame), radius);
  TopoDS_Edge edge=BRepBuilderAPI_MakeEdge(law,surface).Edge();
  if (!BRepLib::BuildCurves3d(edge,1e-7)) throw std::runtime_error("helix approximation failed");
  TopoDS_Wire wire=BRepBuilderAPI_MakeWire(edge).Wire();
  require_valid(wire,"helix");
  return wrap(wire);
}

// The edges where two shapes meet, as a compound.
std::unique_ptr<Shape> section(const Shape &a, const Shape &b) {
  BRepAlgoAPI_Section op(a.shape,b.shape,false);
  op.ComputePCurveOn1(true);
  op.Approximation(true);
  op.Build();
  if (!op.IsDone() || op.HasErrors()) throw std::runtime_error("could not intersect the shapes");
  TopoDS_Compound edges;
  BRep_Builder builder;
  builder.MakeCompound(edges);
  int count=0;
  for (TopExp_Explorer e(op.Shape(),TopAbs_EDGE);e.More();e.Next()) {
    builder.Add(edges,e.Current());
    ++count;
  }
  if (!count) throw std::runtime_error("the shapes do not intersect");
  return wrap(edges);
}

std::unique_ptr<Shape> project_wire(const Shape &wire, const Shape &target, const Vec3 &direction) {
  BRepProj_Projection projection(wire.shape,target.shape,dir(direction));
  if (!projection.IsDone() || !projection.More()) throw std::runtime_error("curve projection does not reach the target");
  // One wire, or a compound of the separate pieces (where a hole or the face's
  // edge interrupts the curve).
  TopoDS_Wire first=projection.Current();
  projection.Next();
  if (!projection.More()) {
    require_valid(first,"projected curve");
    return wrap(first);
  }
  TopoDS_Compound pieces;
  BRep_Builder builder;
  builder.MakeCompound(pieces);
  builder.Add(pieces,first);
  for (;projection.More();projection.Next()) builder.Add(pieces,projection.Current());
  require_valid(pieces,"projected curve");
  return wrap(pieces);
}

std::unique_ptr<Shape> split_face(const Shape &body, uint32_t face, const Shape &wire, rust::Vec<HistoryEntry> &hist) {
  auto target=sub_shape(body,0,face);
  auto path=join_wires(wire);
  BRepFeat_SplitShape op(body.shape);
  op.Add(TopoDS::Wire(path->shape),TopoDS::Face(target->shape));
  op.Build();
  if (!op.IsDone()) throw std::runtime_error("split line failed; use a closed loop or a path crossing the face boundary");
  require_valid(op.Shape(),"split line");
  record_history(op,body.shape,0,Maps(op.Shape()),hist);
  return wrap(op.Shape());
}

// Identify planar caps by their boundary topology, never by output face order.
static void record_caps(const TopoDS_Shape &section, uint8_t relation, const Maps &out, rust::Vec<HistoryEntry> &hist) {
  if (section.IsNull()) return;
  TopTools_IndexedMapOfShape edges;
  TopExp::MapShapes(section,TopAbs_EDGE,edges);
  for (int f=1;f<=out.m[0].Extent();++f) {
    TopTools_IndexedMapOfShape boundary;
    TopExp::MapShapes(out.m[0](f),TopAbs_EDGE,boundary);
    if (boundary.Extent()!=edges.Extent()) continue;
    bool same=true;
    for (int e=1;e<=edges.Extent();++e) if (!boundary.Contains(edges(e))) { same=false; break; }
    if (same) push(hist,0,1,1,relation,0,f);
  }
}

std::unique_ptr<Shape> sweep(const Shape &profile, const Shape &path, const Frame &frame, uint8_t orientation, rust::Vec<HistoryEntry> &hist) {
  auto profile_wire=join_wires(profile), path_wire=join_wires(path);
  const auto &section=TopoDS::Wire(profile_wire->shape);
  if (!section.Closed()) throw std::invalid_argument("sweep requires a closed section");
  BRepOffsetAPI_MakePipeShell op(TopoDS::Wire(path_wire->shape));
  if (orientation<2) op.SetMode(orientation==1);
  else if (orientation==2) op.SetMode(gp_Ax2(pnt(frame.origin),dir(frame.normal),dir(frame.x_dir)));
  else throw std::invalid_argument("invalid sweep orientation");
  // Sharp path corners are mitred; the default transition silently stops at the first one.
  op.SetTransitionMode(BRepBuilderAPI_RightCorner);
  op.Add(section,false,false);
  op.Build();
  if (!op.IsDone() || !op.MakeSolid()) throw std::runtime_error("sweep failed; check path continuity and profile attachment");
  require_valid(op.Shape(),"sweep");
  // A truncated or folded pipe passes BRepCheck, so check self-intersection explicitly.
  BOPAlgo_ArgumentAnalyzer self_check;
  self_check.SetShape1(op.Shape());
  self_check.SelfInterMode()=true;
  self_check.Perform();
  if (self_check.HasFaulty())
    throw std::runtime_error("the swept body intersects itself; reduce the profile size, increase the path's bend radius or change the profile orientation");
  Maps maps(op.Shape());
  record_history(op,profile.shape,0,maps,hist);
  record_history(op,path.shape,1,maps,hist);
  record_caps(op.FirstShape(),FIRST_CAP,maps,hist);
  record_caps(op.LastShape(),LAST_CAP,maps,hist);
  return wrap(op.Shape());
}

// A curve that circles `path` at `radius`, turning `twist` radians about it from
// start to end relative to a rotation-minimizing frame. Used as an auxiliary spine
// it rotates the swept section by exactly that angle, distributed by arc length.
static TopoDS_Wire twist_spine(const TopoDS_Wire &path, double twist, double radius) {
  BRepAdaptor_CompCurve curve(path);
  const double first = curve.FirstParameter(), last = curve.LastParameter();
  const double length = GCPnts_AbscissaPoint::Length(curve);
  if (length <= Precision::Confusion()) throw std::invalid_argument("the sweep path has no length");
  const int n = std::max(64, static_cast<int>(std::ceil(std::abs(twist) / (M_PI / 16))) + 1);
  Handle(TColgp_HArray1OfPnt) points = new TColgp_HArray1OfPnt(1, n + 1);
  gp_Pnt p;
  gp_Vec t;
  curve.D1(first, p, t);
  gp_Vec r = t.Crossed(gp_Vec(1, 0, 0));
  if (r.Magnitude() < 1e-6 * t.Magnitude()) r = t.Crossed(gp_Vec(0, 1, 0));
  r.Normalize();
  gp_Pnt prev_p = p;
  gp_Vec prev_t = t.Normalized();
  for (int i = 0; i <= n; ++i) {
    const double s = length * i / n;
    const double u = i == 0 ? first : i == n ? last : GCPnts_AbscissaPoint(curve, s, first).Parameter();
    curve.D1(u, p, t);
    t.Normalize();
    if (i > 0) {
      // Double reflection carries the reference vector without spurious twist.
      gp_Vec v1(prev_p, p);
      const double c1 = v1.SquareMagnitude();
      if (c1 > 0) {
        const gp_Vec rl = r - v1 * (2 / c1 * v1.Dot(r));
        const gp_Vec tl = prev_t - v1 * (2 / c1 * v1.Dot(prev_t));
        const gp_Vec v2 = t - tl;
        const double c2 = v2.SquareMagnitude();
        r = c2 > 0 ? rl - v2 * (2 / c2 * v2.Dot(rl)) : rl;
      }
      r = (r - t * r.Dot(t)).Normalized();
    }
    const double angle = twist * i / n;
    const gp_Vec b = t.Crossed(r);
    points->SetValue(i + 1, p.Translated((r * std::cos(angle) + b * std::sin(angle)) * radius));
    prev_p = p;
    prev_t = t;
  }
  GeomAPI_Interpolate interpolate(points, false, 1e-9);
  interpolate.Perform();
  if (!interpolate.IsDone()) throw std::runtime_error("could not build the twist guide");
  return BRepBuilderAPI_MakeWire(BRepBuilderAPI_MakeEdge(interpolate.Curve()).Edge()).Wire();
}

Frame wire_start(const Shape &path) {
  auto wire = join_wires(path);
  BRepAdaptor_CompCurve curve(TopoDS::Wire(wire->shape));
  gp_Pnt p;
  gp_Vec t;
  curve.D1(curve.FirstParameter(), p, t);
  if (t.Magnitude() < Precision::Confusion()) throw std::invalid_argument("the path has no direction at its start");
  t.Normalize();
  gp_Vec x = t.Crossed(gp_Vec(0, 0, 1));
  if (x.Magnitude() < 1e-6) x = t.Crossed(gp_Vec(0, 1, 0));
  x.Normalize();
  return Frame{vec3(p.XYZ()), vec3(x.XYZ()), vec3(t.XYZ())};
}

std::unique_ptr<Shape> sweep_aux(const Shape &profile, const Shape &path, const Shape &guide, double twist,
                                 rust::Vec<HistoryEntry> &hist) {
  auto profile_wire = join_wires(profile), path_wire = join_wires(path);
  const auto &section = TopoDS::Wire(profile_wire->shape);
  const auto &spine = TopoDS::Wire(path_wire->shape);
  if (!section.Closed()) throw std::invalid_argument("sweep requires a closed section");
  if (!std::isfinite(twist)) throw std::invalid_argument("the twist angle must be finite");
  TopoDS_Wire aux;
  if (!guide.shape.IsNull()) {
    if (twist != 0) throw std::invalid_argument("a sweep follows a guide curve or twists, not both");
    aux = TopoDS::Wire(join_wires(guide)->shape);
  } else {
    Bnd_Box box;
    BRepBndLib::Add(spine, box);
    BRepBndLib::Add(section, box);
    aux = twist_spine(spine, twist, std::max(1.0, std::sqrt(box.SquareExtent())));
  }
  BRepOffsetAPI_MakePipeShell op(spine);
  // The guide sets the section's rotation where it crosses each plane normal to the path.
  op.SetMode(aux, false, BRepFill_NoContact);
  op.SetTransitionMode(BRepBuilderAPI_RightCorner);
  op.Add(section, false, false);
  op.Build();
  if (!op.IsDone() || !op.MakeSolid())
    throw std::runtime_error("guided sweep failed; the guide must stay beside the path along its whole length");
  require_valid(op.Shape(), "sweep");
  BOPAlgo_ArgumentAnalyzer self_check;
  self_check.SetShape1(op.Shape());
  self_check.SelfInterMode() = true;
  self_check.Perform();
  if (self_check.HasFaulty())
    throw std::runtime_error("the swept body intersects itself; reduce the twist or the profile size");
  Maps maps(op.Shape());
  record_history(op, profile.shape, 0, maps, hist);
  record_history(op, path.shape, 1, maps, hist);
  record_caps(op.FirstShape(), FIRST_CAP, maps, hist);
  record_caps(op.LastShape(), LAST_CAP, maps, hist);
  return wrap(op.Shape());
}

std::unique_ptr<Shape> loft(const Shape &profiles, bool ruled, bool preserve_correspondence, rust::Vec<HistoryEntry> &hist) {
  BRepOffsetAPI_ThruSections op(true,ruled,1e-7);
  int count=0;
  for (TopExp_Explorer ex(profiles.shape,TopAbs_WIRE);ex.More();ex.Next()) {
    const auto &wire=TopoDS::Wire(ex.Current());
    if (!wire.Closed()) throw std::invalid_argument("loft profiles must be closed");
    op.AddWire(wire); ++count;
  }
  if (count<2) throw std::invalid_argument("loft requires at least two profiles");
  op.CheckCompatibility(!preserve_correspondence);
  op.SetMutableInput(false);
  op.Build();
  if (!op.IsDone()) throw std::runtime_error("loft failed; check profile ordering and compatibility");
  require_valid(op.Shape(),"loft");
  Maps maps(op.Shape());
  record_history(op,profiles.shape,0,maps,hist);
  record_caps(op.FirstShape(),FIRST_CAP,maps,hist);
  record_caps(op.LastShape(),LAST_CAP,maps,hist);
  return wrap(op.Shape());
}

std::unique_ptr<Shape> half_space(const Shape &face, const Vec3 &inside) {
  TopExp_Explorer ex(face.shape, TopAbs_FACE);
  if (!ex.More()) throw std::invalid_argument("half space needs a face");
  return wrap(BRepPrimAPI_MakeHalfSpace(TopoDS::Face(ex.Current()), pnt(inside)).Solid());
}

std::unique_ptr<Shape> offset_face(const Shape &face, double distance, const Vec3 &toward) {
  TopExp_Explorer ex(face.shape, TopAbs_FACE);
  if (!ex.More() || !std::isfinite(distance))
    throw std::invalid_argument("offset needs a face and a finite distance");
  const TopoDS_Face f = TopoDS::Face(ex.Current());
  double u0, u1, v0, v1;
  BRepTools::UVBounds(f, u0, u1, v0, v1);
  Handle(Geom_Surface) base = BRep_Tool::Surface(f);
  GeomAPI_ProjectPointOnSurf projection(pnt(toward), base);
  if (projection.NbPoints() == 0) throw std::runtime_error("cannot find the offset side of the surface");
  double u, v;
  projection.LowerDistanceParameters(u, v);
  GeomLProp_SLProps props(base, u, v, 1, 1e-7);
  if (!props.IsNormalDefined()) throw std::runtime_error("the target surface has no normal");
  const double side = gp_Vec(projection.NearestPoint(), pnt(toward)).Dot(gp_Vec(props.Normal()));
  if (std::abs(side) < 1e-7) throw std::invalid_argument("the sketch touches the target surface");
  Handle(Geom_OffsetSurface) offset = new Geom_OffsetSurface(base, side > 0 ? distance : -distance);
  // Preserve analytic planes/cylinders so downstream sketches and axes can
  // still recognize them after an offset.
  Handle(Geom_Surface) surface = offset->Surface();
  if (surface.IsNull()) surface = offset;
  BRepBuilderAPI_MakeFace op(surface, u0, u1, v0, v1, 1e-7);
  if (!op.IsDone()) throw std::runtime_error("surface offset failed");
  TopoDS_Shape out = op.Face();
  out.Orientation(f.Orientation());
  require_valid(out, "surface offset");
  return wrap(out);
}

std::unique_ptr<Shape> revol_until(const Shape &s, const Shape &until, const Vec3 &origin,
                                  const Vec3 &d, rust::Vec<HistoryEntry> &hist) {
  const gp_Ax1 axis(pnt(origin), dir(d));
  BRepPrimAPI_MakeRevol sweep(s.shape, axis, 2.0 * std::acos(-1.0), false);
  sweep.Build();
  if (!sweep.IsDone()) throw std::runtime_error("revolve failed");
  require_valid(sweep.Shape(), "revolve");
  BRepAlgoAPI_Splitter op;
  TopTools_ListOfShape args, tools;
  args.Append(sweep.Shape());
  tools.Append(s.shape);
  tools.Append(until.shape);
  op.SetArguments(args);
  op.SetTools(tools);
  op.Build();
  if (!op.IsDone()) throw std::runtime_error("the revolve cannot terminate at the selected surface");

  // Identify the starting caps by topology. Their outward normals must
  // oppose the forward tangential velocity. This is exact even for very
  // small angles and does not depend on a probe size or model scale.
  TopTools_IndexedMapOfShape starts, source_faces;
  TopExp::MapShapes(s.shape, TopAbs_FACE, source_faces);
  std::vector<TopTools_IndexedMapOfShape> source_starts(source_faces.Extent());
  std::vector<double> covered(source_faces.Extent(), 0.0);
  for (int i = 1; i <= source_faces.Extent(); ++i) {
    starts.Add(source_faces(i));
    source_starts[i - 1].Add(source_faces(i));
    for (const TopoDS_Shape &f : op.Modified(source_faces(i))) {
      starts.Add(f);
      source_starts[i - 1].Add(f);
    }
  }
  TopTools_IndexedMapOfShape ends;
  for (TopExp_Explorer ex(until.shape, TopAbs_FACE); ex.More(); ex.Next()) {
    ends.Add(ex.Current());
    for (const TopoDS_Shape &f : op.Modified(ex.Current())) ends.Add(f);
  }
  TopoDS_Compound out;
  BRep_Builder builder;
  builder.MakeCompound(out);
  int kept = 0, total = 0;
  for (TopExp_Explorer ex(op.Shape(), TopAbs_SOLID); ex.More(); ex.Next()) {
    ++total;
    bool forward = false, terminated = false;
    for (TopExp_Explorer fx(ex.Current(), TopAbs_FACE); fx.More(); fx.Next()) {
      if (ends.Contains(fx.Current())) terminated = true;
      if (!starts.Contains(fx.Current())) continue;
      const TopoDS_Face cap = TopoDS::Face(fx.Current());
      BRepAdaptor_Surface surface(cap);
      if (surface.GetType() != GeomAbs_Plane) continue;
      gp_Vec normal(surface.Plane().Axis().Direction());
      if (cap.Orientation() == TopAbs_REVERSED) normal.Reverse();
      GProp_GProps props;
      BRepGProp::SurfaceProperties(cap, props);
      const gp_Vec velocity = gp_Vec(axis.Direction()).Crossed(
          gp_Vec(axis.Location(), props.CentreOfMass()));
      if (normal.Dot(velocity) < -1e-9) {
        forward = true;
        for (int i = 1; i <= source_faces.Extent(); ++i)
          if (source_starts[i - 1].Contains(cap)) covered[i - 1] += props.Mass();
      }
    }
    if (forward) {
      if (!terminated)
        throw std::invalid_argument("the selected surface must terminate every sketch contour");
      builder.Add(out, ex.Current());
      ++kept;
    }
  }
  if (kept == 0 || kept == total)
    throw std::invalid_argument("the selected surface does not terminate the revolve ahead of the sketch");
  for (int i = 1; i <= source_faces.Extent(); ++i) {
    GProp_GProps props;
    BRepGProp::SurfaceProperties(source_faces(i), props);
    if (std::abs(covered[i - 1] - props.Mass()) > 1e-7 * std::max(1.0, props.Mass()))
      throw std::invalid_argument("the selected surface must terminate every sketch contour");
  }
  require_valid(out, "revolve up to surface");
  Maps maps(out);
  // Carry each generated side through the partition so sketch edge names
  // survive target edits. The start and termination faces get cap names.
  TopTools_IndexedMapOfShape edges;
  TopExp::MapShapes(s.shape, TopAbs_EDGE, edges);
  for (int i = 1; i <= edges.Extent(); ++i) {
    for (const TopoDS_Shape &generated : sweep.Generated(edges(i))) {
      int same = maps.m[0].FindIndex(generated);
      if (same > 0) push(hist, 0, 1, i, GENERATED, 0, same);
      for (const TopoDS_Shape &modified : op.Modified(generated)) {
        int index = maps.m[0].FindIndex(modified);
        if (index > 0) push(hist, 0, 1, i, GENERATED, 0, index);
      }
    }
  }
  TopTools_IndexedMapOfShape faces;
  TopExp::MapShapes(s.shape, TopAbs_FACE, faces);
  for (int i = 1; i <= faces.Extent(); ++i) {
    int same = maps.m[0].FindIndex(faces(i));
    if (same > 0) push(hist, 0, 0, i, FIRST_CAP, 0, same);
    for (const TopoDS_Shape &modified : op.Modified(faces(i))) {
      int index = maps.m[0].FindIndex(modified);
      if (index > 0) push(hist, 0, 0, i, FIRST_CAP, 0, index);
    }
  }
  record_history(op, until.shape, 1, maps, hist);
  return wrap(out);
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

// OpenCASCADE's blend builders keep shared state, so concurrent fillets and
// chamfers in one process can fail at random. Run them one at a time.
static std::mutex &blend_mutex() {
  static std::mutex m;
  return m;
}

std::unique_ptr<Shape> fillet(const Shape &s, rust::Slice<const uint32_t> edges, double radius,
                              rust::Vec<HistoryEntry> &hist) {
  std::lock_guard<std::mutex> lock(blend_mutex());
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
  std::lock_guard<std::mutex> lock(blend_mutex());
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

std::unique_ptr<Shape> plane_face(const Frame &plane) {
  return wrap(BRepBuilderAPI_MakeFace(gp_Pln(pnt(plane.origin), dir(plane.normal))).Face());
}

int8_t face_orientation(const Shape &s, uint32_t face) {
  auto selected = sub_shape(s, 0, face);
  for (TopExp_Explorer ex(s.shape, TopAbs_FACE); ex.More(); ex.Next()) {
    if (ex.Current().IsSame(selected->shape)) {
      if (ex.Current().Orientation() == TopAbs_FORWARD) return 1;
      if (ex.Current().Orientation() == TopAbs_REVERSED) return -1;
      throw std::runtime_error("split face has no boundary orientation");
    }
  }
  throw std::runtime_error("split face no longer exists");
}

std::unique_ptr<Shape> split_tools(const Shape &s, const Shape &tool, rust::Vec<HistoryEntry> &hist) {
  TopTools_ListOfShape arguments, tools;
  arguments.Append(s.shape);
  tools.Append(tool.shape);
  BRepAlgoAPI_Splitter op;
  op.SetArguments(arguments);
  op.SetTools(tools);
  op.SetNonDestructive(true);
  op.Build();
  if (!op.IsDone() || op.HasErrors()) throw std::runtime_error("tool split failed; check that the tools cross the selected bodies");
  require_valid(op.Shape(), "tool split");
  Maps maps(op.Shape());
  record_history(op, s.shape, 0, maps, hist);
  record_history(op, tool.shape, 1, maps, hist);
  return wrap(op.Shape());
}

std::unique_ptr<Shape> split_plane(const Shape &s, const Frame &plane, rust::Vec<HistoryEntry> &hist) {
  TopTools_ListOfShape arguments, tools;
  arguments.Append(s.shape);
  tools.Append(BRepBuilderAPI_MakeFace(gp_Pln(pnt(plane.origin), dir(plane.normal))).Face());
  BRepAlgoAPI_Splitter op;
  op.SetArguments(arguments);
  op.SetTools(tools);
  op.SetNonDestructive(true);
  op.Build();
  if (!op.IsDone() || op.HasErrors()) throw std::runtime_error("plane split failed");
  require_valid(op.Shape(), "split");
  record_history(op, s.shape, 0, Maps(op.Shape()), hist);
  return wrap(op.Shape());
}

// Station positions run along the whole tangent chain through `edge`, from the
// chain end that the selected edge's own direction points away from.
std::unique_ptr<Shape> variable_fillet(const Shape &s, uint32_t edge, rust::Slice<const double> stations, bool straight, rust::Vec<HistoryEntry> &hist) {
  std::lock_guard<std::mutex> lock(blend_mutex());
  if (stations.size()<4 || stations.size()%2 || stations.size()>512) throw std::invalid_argument("fillet requires 2–256 radius stations");
  double previous=-1;
  std::vector<std::pair<double,double>> points;
  for (size_t i=0;i<stations.size();i+=2) {
    double u=stations[i], r=stations[i+1];
    if (!std::isfinite(u+r) || u<0 || u>1 || u<=previous || r<=0) throw std::invalid_argument("fillet stations must increase from 0 to 1 with positive finite radii");
    previous=u; points.emplace_back(u,r);
  }
  if (stations[0]!=0 || stations[stations.size()-2]!=1) throw std::invalid_argument("include radius stations at 0 and 1");
  auto selected=sub_shape(s,1,edge);
  const TopoDS_Edge &e=TopoDS::Edge(selected->shape);
  BRepFilletAPI_MakeFillet op(s.shape);
  op.Add(e);
  const int contour=op.Contour(e);
  if (contour<1) throw std::runtime_error("could not build a fillet chain from the selected edge");
  // Follow the selected edge's direction along the chain.
  {
    const double a=op.RelativeAbscissa(contour,TopExp::FirstVertex(e,true));
    const double b=op.RelativeAbscissa(contour,TopExp::LastVertex(e,true));
    // On a closed chain an edge may straddle the chain's start, wrapping from near 1 to 0.
    const bool reversed=op.Closed(contour) && std::abs(a-b)>0.5 ? a<b : a>b;
    if (reversed) {
      std::reverse(points.begin(),points.end());
      for (auto &p:points) p.first=1-p.first;
    }
  }
  // Two stations interpolate linearly either way, but the interpolator needs three.
  if (straight || points.size() < 3) {
    // The station interpolator is smooth; feeding it points sampled from the
    // piecewise-linear law keeps each segment linear, rounding only at interior
    // stations.
    std::vector<std::pair<double,double>> dense;
    const int per=16;
    for (size_t i=0;i+1<points.size();++i)
      for (int k=0;k<per;++k) {
        const double t=static_cast<double>(k)/per;
        dense.emplace_back(points[i].first+t*(points[i+1].first-points[i].first),
                           points[i].second+t*(points[i+1].second-points[i].second));
      }
    dense.push_back(points.back());
    points=dense;
  }
  TColgp_Array1OfPnt2d radii(1,static_cast<int>(points.size()));
  for (size_t i=0;i<points.size();++i) radii(static_cast<int>(i)+1)=gp_Pnt2d(points[i].first,points[i].second);
  op.SetRadius(radii,contour,1);
  op.Build();
  if (!op.IsDone()) throw std::runtime_error("variable fillet failed; reduce radii or change the selected edge");
  require_valid(op.Shape(),"variable fillet");
  record_history(op,s.shape,0,Maps(op.Shape()),hist);
  return wrap(op.Shape());
}

// Each face hinges about its own neutral plane, for parting-line drafts.
std::unique_ptr<Shape> draft_planes(const Shape &s, rust::Slice<const uint32_t> faces, rust::Slice<const Frame> planes, const Vec3 &direction, double angle, rust::Vec<HistoryEntry> &hist) {
  if (!faces.size() || faces.size()!=planes.size() || !std::isfinite(angle) || std::abs(angle)>=std::acos(-1.0)/2 || std::abs(angle)<1e-12)
    throw std::invalid_argument("draft needs faces, one hinge plane per face and a nonzero angle smaller than 90 degrees");
  BRepOffsetAPI_DraftAngle op(s.shape);
  for (size_t i=0;i<faces.size();++i) {
    auto face=sub_shape(s,0,faces[i]);
    op.Add(TopoDS::Face(face->shape),dir(direction),angle,gp_Pln(pnt(planes[i].origin),dir(planes[i].normal)));
    if (!op.AddDone()) throw std::runtime_error("a face next to the parting line cannot be drafted; check the pull direction");
  }
  op.Build();
  if (!op.IsDone()) throw std::runtime_error("parting line draft failed; change the edges, pull direction or angle");
  require_valid(op.Shape(),"draft");
  record_history(op,s.shape,0,Maps(op.Shape()),hist);
  return wrap(op.Shape());
}

std::unique_ptr<Shape> draft(const Shape &s, rust::Slice<const uint32_t> faces, const Frame &plane, const Vec3 &direction, double angle, rust::Vec<HistoryEntry> &hist) {
  if (!faces.size() || !std::isfinite(angle) || std::abs(angle)>=std::acos(-1.0)/2 || std::abs(angle)<1e-12)
    throw std::invalid_argument("draft needs faces and a nonzero angle smaller than 90 degrees");
  BRepOffsetAPI_DraftAngle op(s.shape);
  for (uint32_t index:faces) {
    auto face=sub_shape(s,0,index);
    op.Add(TopoDS::Face(face->shape),dir(direction),angle,gp_Pln(pnt(plane.origin),dir(plane.normal)));
    if (!op.AddDone()) throw std::runtime_error("this face cannot be drafted with the selected neutral plane");
  }
  op.Build();
  if (!op.IsDone()) throw std::runtime_error("draft failed; change faces, neutral plane or angle");
  require_valid(op.Shape(),"draft");
  record_history(op,s.shape,0,Maps(op.Shape()),hist);
  return wrap(op.Shape());
}

rust::Vec<uint32_t> indices_in(const Shape &part, const Shape &whole, uint8_t kind) {
  TopTools_IndexedMapOfShape source, target;
  TopExp::MapShapes(part.shape, kind_of(kind), source);
  TopExp::MapShapes(whole.shape, kind_of(kind), target);
  rust::Vec<uint32_t> out;
  for (int i = 1; i <= source.Extent(); ++i) {
    int index = target.FindIndex(source(i));
    if (!index) throw std::invalid_argument("shape is not contained in the target");
    out.push_back(static_cast<uint32_t>(index - 1));
  }
  return out;
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

rust::Vec<uint32_t> vertex_faces(const Shape &s) {
  TopTools_IndexedMapOfShape fm, vm;
  TopExp::MapShapes(s.shape, TopAbs_FACE, fm);
  TopExp::MapShapes(s.shape, TopAbs_VERTEX, vm);
  TopTools_IndexedDataMapOfShapeListOfShape vf;
  TopExp::MapShapesAndUniqueAncestors(s.shape, TopAbs_VERTEX, TopAbs_FACE, vf);
  rust::Vec<uint32_t> out;
  for (int i = 1; i <= vm.Extent(); ++i) {
    const TopTools_ListOfShape *faces = vf.Seek(vm(i));
    out.push_back(faces ? static_cast<uint32_t>(faces->Extent()) : 0);
    if (faces)
      for (const TopoDS_Shape &f : *faces) out.push_back(static_cast<uint32_t>(fm.FindIndex(f) - 1));
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
  // Adaptive integration matters for faces trimmed by offset surfaces. Its error
  // target is relative per face, so a reference whose contribution from some face
  // is near zero (a point near the part, or a plane parallel to a curved wall)
  // can stall it. An oblique plane outside the bounding box avoids both.
  Bnd_Box box;
  BRepBndLib::Add(s.shape, box, false);
  double error;
  if (box.IsVoid()) {
    error = BRepGProp::VolumePropertiesGK(s.shape, vp, 1e-9, false, true, true, true);
  } else {
    double x0, y0, z0, x1, y1, z1;
    box.Get(x0, y0, z0, x1, y1, z1);
    const gp_Dir normal(0.48, 0.61, 0.63);
    const double reach = std::sqrt(box.SquareExtent()) + 1.0;
    const gp_Pln reference(gp_Pnt(x0, y0, z0).Translated(-gp_Vec(normal) * reach), normal);
    error = BRepGProp::VolumePropertiesGK(s.shape, vp, reference, 1e-9, false, true, true, true);
  }
  // Some periodic spline faces defeat the plane-referenced integration; fall back
  // to the point-referenced and then the classic adaptive method.
  if (error < 0) {
    vp = GProp_GProps();
    error = BRepGProp::VolumePropertiesGK(s.shape, vp, 1e-9, false, true, true, true);
  }
  if (error < 0) {
    vp = GProp_GProps();
    error = BRepGProp::VolumeProperties(s.shape, vp, 1e-9, false);
  }
  if (error < 0) throw std::runtime_error("volume integration failed");
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
  quiet();
  STEPControl_Writer w;
  Interface_Static::SetCVal("write.step.unit", "MM");
  Interface_Static::SetCVal("write.step.product.name", "OpenForge part");
  if (w.Transfer(s.shape, STEPControl_AsIs) != IFSelect_RetDone) throw std::runtime_error("could not convert part to STEP");
  if (w.Write(std::string(path).c_str()) != IFSelect_RetDone) throw std::runtime_error("could not write STEP file");
}

std::unique_ptr<Shape> read_step(rust::Str path) {
  quiet();
  STEPControl_Reader r;
  if (r.ReadFile(std::string(path).c_str()) != IFSelect_RetDone) throw std::runtime_error("could not read STEP file");
  r.TransferRoots();
  TopoDS_Shape s = r.OneShape();
  if (s.IsNull()) throw std::runtime_error("STEP file contains no geometry");
  return wrap(fix(s));
}

void write_iges(const Shape &s, rust::Str path) {
  quiet();
  IGESControl_Controller::Init();
  IGESControl_Writer w("MM", 1);
  if (!w.AddShape(s.shape)) throw std::runtime_error("could not convert part to IGES");
  w.ComputeModel();
  if (!w.Write(std::string(path).c_str())) throw std::runtime_error("could not write IGES file");
}

std::unique_ptr<Shape> read_iges(rust::Str path) {
  quiet();
  IGESControl_Controller::Init();
  IGESControl_Reader r;
  if (r.ReadFile(std::string(path).c_str()) != IFSelect_RetDone) throw std::runtime_error("could not read IGES file");
  r.TransferRoots();
  TopoDS_Shape s = r.OneShape();
  if (s.IsNull()) throw std::runtime_error("IGES file contains no geometry");
  return wrap(fix(s));
}

void write_stl(const Shape &s, rust::Str path, double linear, double angular, bool ascii) {
  quiet();
  BRepMesh_IncrementalMesh mesher(s.shape, linear, false, angular, true);
  StlAPI_Writer w;
  w.ASCIIMode() = ascii;
  if (!w.Write(s.shape, std::string(path).c_str())) throw std::runtime_error("could not write STL file");
}

} // namespace ofk

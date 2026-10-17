// Surface bodies: knitting faces into shells and solids, offsets and
// thickening, deleting and patching faces, filled patches, and the surface
// versions of sweeps and lofts.

#include "of-kernel-sys/cpp/ofk.h"
#include "of-kernel-sys/src/lib.rs.h"

#include <BOPAlgo_ArgumentAnalyzer.hxx>
#include <BRepAlgoAPI_Defeaturing.hxx>
#include <BRepAlgoAPI_Splitter.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepGProp_Face.hxx>
#include <BRepTools.hxx>
#include <ShapeAnalysis_FreeBounds.hxx>
#include <TopTools_HSequenceOfShape.hxx>
#include <gp_Vec.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepLib.hxx>
#include <BRepOffsetAPI_MakeFilling.hxx>
#include <BRepOffset_MakeOffset.hxx>
#include <BRepOffsetAPI_MakeOffsetShape.hxx>
#include <BRepOffsetAPI_MakePipeShell.hxx>
#include <BRepOffsetAPI_ThruSections.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <ShapeFix_Shell.hxx>
#include <ShapeFix_Solid.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopoDS_Iterator.hxx>
#include <TopTools_ListOfShape.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>

#include <stdexcept>
#include <string>
#include <vector>

namespace ofk {
namespace {

const TopAbs_ShapeEnum SUB[3] = {TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX};
enum Rel : uint8_t { SAME = 0, MODIFIED = 1, GENERATED = 2, FIRST_CAP = 3, LAST_CAP = 4 };

std::unique_ptr<Shape> wrap(const TopoDS_Shape &s) { return std::make_unique<Shape>(s); }

struct Maps {
  TopTools_IndexedMapOfShape m[3];
  explicit Maps(const TopoDS_Shape &s) {
    for (int k = 0; k < 3; ++k) TopExp::MapShapes(s, SUB[k], m[k]);
  }
};

void push(rust::Vec<HistoryEntry> &hist, uint8_t input, uint8_t from_kind, int from, uint8_t rel,
          uint8_t to_kind, int to) {
  hist.push_back(HistoryEntry{input, from_kind, static_cast<uint32_t>(from - 1), rel, to_kind,
                              static_cast<uint32_t>(to - 1)});
}

// History through any algorithm with Modified/Generated/IsDeleted, so faces
// and edges keep their names.
template <class Op>
void record(Op &op, const TopoDS_Shape &input, uint8_t which, const Maps &out,
            rust::Vec<HistoryEntry> &hist) {
  for (uint8_t k = 0; k < 3; ++k) {
    TopTools_IndexedMapOfShape in;
    TopExp::MapShapes(input, SUB[k], in);
    for (int i = 1; i <= in.Extent(); ++i) {
      const TopoDS_Shape &s = in(i);
      bool modified = false;
      for (const TopoDS_Shape &m : op.Modified(s)) {
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
      for (const TopoDS_Shape &g : op.Generated(s)) {
        for (uint8_t k2 = 0; k2 < 3; ++k2) {
          int idx = out.m[k2].FindIndex(g);
          if (idx > 0) push(hist, which, k, i, GENERATED, k2, idx);
        }
      }
    }
  }
}

// Sub-shapes of `input` found unchanged in `out`.
void record_same(const TopoDS_Shape &input, const Maps &out, rust::Vec<HistoryEntry> &hist) {
  for (uint8_t k = 0; k < 3; ++k) {
    TopTools_IndexedMapOfShape in;
    TopExp::MapShapes(input, SUB[k], in);
    for (int i = 1; i <= in.Extent(); ++i) {
      int idx = out.m[k].FindIndex(in(i));
      if (idx > 0) push(hist, 0, k, i, SAME, k, idx);
    }
  }
}

void record_caps(const TopoDS_Shape &section, uint8_t relation, const Maps &out,
                 rust::Vec<HistoryEntry> &hist) {
  if (section.IsNull()) return;
  for (TopExp_Explorer ex(section, TopAbs_EDGE); ex.More(); ex.Next()) {
    int idx = out.m[1].FindIndex(ex.Current());
    if (idx > 0) push(hist, 0, 1, 1, relation, 1, idx);
  }
}

TopTools_IndexedMapOfShape faces_of(const TopoDS_Shape &s) {
  TopTools_IndexedMapOfShape m;
  TopExp::MapShapes(s, TopAbs_FACE, m);
  return m;
}

TopoDS_Shape face_at(const TopTools_IndexedMapOfShape &m, uint32_t index) {
  if (index >= static_cast<uint32_t>(m.Extent())) throw std::out_of_range("no such face");
  return m(static_cast<int>(index) + 1);
}

// Sew faces with `tolerance`, recording which input face and edge became
// which output one.
TopoDS_Shape sew_faces(const TopoDS_Shape &input, const std::vector<TopoDS_Shape> &faces,
                       double tolerance, rust::Vec<HistoryEntry> &hist, uint32_t &free_edges) {
  BRepBuilderAPI_Sewing sew(tolerance, true, true, true, false);
  for (const auto &f : faces) sew.Add(f);
  sew.Perform();
  TopoDS_Shape out = sew.SewedShape();
  if (out.IsNull()) throw std::runtime_error("knitting produced no geometry");
  free_edges = static_cast<uint32_t>(sew.NbFreeEdges());
  Maps maps(out);
  for (uint8_t k = 0; k < 2; ++k) {
    TopTools_IndexedMapOfShape in;
    TopExp::MapShapes(input, SUB[k], in);
    for (int i = 1; i <= in.Extent(); ++i) {
      TopoDS_Shape s = in(i);
      TopoDS_Shape m = sew.IsModifiedSubShape(s) ? sew.ModifiedSubShape(s) : s;
      if (m.IsNull()) continue;
      for (TopExp_Explorer ex(m, SUB[k]); ex.More(); ex.Next()) {
        int idx = maps.m[k].FindIndex(ex.Current());
        if (idx > 0) push(hist, 0, k, i, m.IsSame(s) ? SAME : MODIFIED, k, idx);
      }
    }
  }
  return out;
}

GeomAbs_Shape order_of(uint8_t order) {
  switch (order) {
  case 0: return GeomAbs_C0;
  case 1: return GeomAbs_G1;
  case 2: return GeomAbs_G2;
  default: throw std::invalid_argument("unknown continuity");
  }
}

} // namespace

std::unique_ptr<Shape> sew(const Shape &faces, double tolerance, rust::Vec<HistoryEntry> &hist,
                           uint32_t &free_edges) {
  std::vector<TopoDS_Shape> list;
  for (TopExp_Explorer ex(faces.shape, TopAbs_FACE); ex.More(); ex.Next()) list.push_back(ex.Current());
  if (list.empty()) throw std::invalid_argument("there are no faces to knit");
  return wrap(sew_faces(faces.shape, list, tolerance, hist, free_edges));
}

std::unique_ptr<Shape> solidify(const Shape &shells, rust::Vec<HistoryEntry> &hist) {
  BRep_Builder b;
  TopoDS_Compound out;
  b.MakeCompound(out);
  int count = 0;
  for (TopExp_Explorer ex(shells.shape, TopAbs_SHELL); ex.More(); ex.Next()) {
    TopoDS_Shell shell = TopoDS::Shell(ex.Current());
    if (!BRep_Tool::IsClosed(shell))
      throw std::runtime_error("the surfaces don't enclose a volume; there are gaps between them");
    BRepBuilderAPI_MakeSolid make(shell);
    if (!make.IsDone()) throw std::runtime_error("couldn't make a solid from the surfaces");
    ShapeFix_Solid fix(make.Solid());
    fix.Perform();
    TopoDS_Shape solid = fix.Shape();
    if (!BRepCheck_Analyzer(solid).IsValid())
      throw std::runtime_error("the knitted solid is invalid; check the surfaces for overlaps");
    b.Add(out, solid);
    ++count;
  }
  if (count == 0) throw std::runtime_error("there are no closed surfaces to make a solid from");
  TopoDS_Shape result = count == 1 ? TopoDS_Shape(TopoDS_Iterator(out).Value()) : TopoDS_Shape(out);
  Maps maps(result);
  record_same(shells.shape, maps, hist);
  return wrap(result);
}

std::unique_ptr<Shape> offset_shape(const Shape &s, double distance, rust::Vec<HistoryEntry> &hist) {
  if (std::abs(distance) < 1e-9) {
    BRepBuilderAPI_Copy copy(s.shape);
    TopoDS_Shape out = copy.Shape();
    Maps maps(out);
    record(copy, s.shape, 0, maps, hist);
    return wrap(out);
  }
  BRepOffsetAPI_MakeOffsetShape op;
  op.PerformByJoin(s.shape, distance, 1e-6, BRepOffset_Skin, false, false, GeomAbs_Arc, false);
  if (!op.IsDone())
    throw std::runtime_error(
        "offset failed; the distance may be larger than the smallest radius of curvature");
  TopoDS_Shape out = op.Shape();
  if (out.IsNull() || !BRepCheck_Analyzer(out).IsValid())
    throw std::runtime_error("offset produced invalid geometry");
  Maps maps(out);
  record(op, s.shape, 0, maps, hist);
  return wrap(out);
}

std::unique_ptr<Shape> thicken(const Shape &s, double thickness, rust::Vec<HistoryEntry> &hist) {
  // Thickening works on one shell; a bare face goes into a shell of its own.
  TopoDS_Shape input = s.shape;
  TopExp_Explorer shell_ex(input, TopAbs_SHELL);
  if (shell_ex.More()) {
    input = shell_ex.Current();
    shell_ex.Next();
    if (shell_ex.More()) throw std::invalid_argument("thicken one surface body at a time");
  } else {
    BRep_Builder b;
    TopoDS_Shell shell;
    b.MakeShell(shell);
    int n = 0;
    for (TopExp_Explorer ex(s.shape, TopAbs_FACE); ex.More(); ex.Next(), ++n) b.Add(shell, ex.Current());
    if (n == 0) throw std::invalid_argument("there's no surface to thicken");
    input = shell;
  }
  BRepOffset_MakeOffset op;
  op.Initialize(input, thickness, 1e-6, BRepOffset_Skin, false, false, GeomAbs_Intersection, true);
  op.MakeOffsetShape();
  if (!op.IsDone())
    throw std::runtime_error(
        "thicken failed; the thickness may be larger than the smallest radius of curvature");
  TopoDS_Shape out = op.Shape();
  if (out.IsNull()) throw std::runtime_error("thicken produced no geometry");
  if (out.ShapeType() == TopAbs_COMPOUND) {
    TopExp_Explorer solids(out, TopAbs_SOLID);
    if (!solids.More()) throw std::runtime_error("thicken didn't produce a solid");
    out = solids.Current();
  }
  ShapeFix_Solid fix;
  TopoDS_Shape solid = out;
  if (out.ShapeType() == TopAbs_SOLID) {
    fix.Init(TopoDS::Solid(out));
    fix.Perform();
    solid = fix.Shape();
  }
  if (!BRepCheck_Analyzer(solid).IsValid()) throw std::runtime_error("thicken produced invalid geometry");
  Maps maps(solid);
  record(op, input, 0, maps, hist);
  return wrap(solid);
}

std::unique_ptr<Shape> defeature(const Shape &s, rust::Slice<const uint32_t> faces,
                                 rust::Vec<HistoryEntry> &hist) {
  auto map = faces_of(s.shape);
  BRepAlgoAPI_Defeaturing op;
  op.SetShape(s.shape);
  for (uint32_t f : faces) op.AddFaceToRemove(face_at(map, f));
  op.SetRunParallel(true);
  op.Build();
  if (!op.IsDone() || op.HasErrors())
    throw std::runtime_error(
        "the faces can't be removed and patched; the neighbouring faces don't meet when extended");
  TopoDS_Shape out = op.Shape();
  if (out.IsNull() || !BRepCheck_Analyzer(out).IsValid())
    throw std::runtime_error("removing the faces produced invalid geometry");
  Maps maps(out);
  record(op, s.shape, 0, maps, hist);
  return wrap(out);
}

std::unique_ptr<Shape> remove_faces(const Shape &s, rust::Slice<const uint32_t> faces,
                                    rust::Vec<HistoryEntry> &hist) {
  auto map = faces_of(s.shape);
  TopTools_IndexedMapOfShape doomed;
  for (uint32_t f : faces) doomed.Add(face_at(map, f));
  std::vector<TopoDS_Shape> keep;
  for (TopExp_Explorer ex(s.shape, TopAbs_FACE); ex.More(); ex.Next())
    if (!doomed.Contains(ex.Current())) keep.push_back(ex.Current());
  if (keep.empty()) throw std::runtime_error("deleting every face leaves nothing");
  uint32_t free_edges = 0;
  // The kept faces already share their edges; sewing only regroups them into shells.
  return wrap(sew_faces(s.shape, keep, 1e-7, hist, free_edges));
}

std::unique_ptr<Shape> fill(const Shape &edges, const Shape &supports,
                            rust::Slice<const int32_t> support_of, rust::Slice<const uint8_t> orders,
                            const Shape &constraints, double &max_error) {
  auto support_map = faces_of(supports.shape);
  BRepOffsetAPI_MakeFilling op(3, 15, 2, false, 1e-5, 1e-4, 1e-2, 0.1, 8, 9);
  size_t i = 0;
  for (TopExp_Explorer ex(edges.shape, TopAbs_EDGE); ex.More(); ex.Next(), ++i) {
    const TopoDS_Edge &e = TopoDS::Edge(ex.Current());
    uint8_t order = i < orders.size() ? orders[i] : 0;
    int32_t support = i < support_of.size() ? support_of[i] : -1;
    if (order > 0 && support >= 0) {
      op.Add(e, TopoDS::Face(face_at(support_map, static_cast<uint32_t>(support))),
             order_of(order), true);
    } else {
      op.Add(e, GeomAbs_C0, true);
    }
  }
  if (i < 1) throw std::invalid_argument("select the edges around the patch");
  for (TopExp_Explorer ex(constraints.shape, TopAbs_EDGE); ex.More(); ex.Next())
    op.Add(TopoDS::Edge(ex.Current()), GeomAbs_C0, false);
  op.Build();
  if (!op.IsDone())
    throw std::runtime_error("the patch can't be built; check that the boundary is closed");
  TopoDS_Shape out = op.Shape();
  if (out.IsNull() || !BRepCheck_Analyzer(out).IsValid())
    throw std::runtime_error("the patch is invalid");
  max_error = op.G0Error();
  return wrap(out);
}

std::unique_ptr<Shape> sweep_surface(const Shape &profile, const Shape &path, const Frame &frame,
                                     uint8_t orientation, rust::Vec<HistoryEntry> &hist) {
  auto profile_wire = join_wires(profile), path_wire = join_wires(path);
  BRepOffsetAPI_MakePipeShell op(TopoDS::Wire(path_wire->shape));
  gp_Pnt o(frame.origin.x, frame.origin.y, frame.origin.z);
  gp_Dir n(frame.normal.x, frame.normal.y, frame.normal.z), x(frame.x_dir.x, frame.x_dir.y, frame.x_dir.z);
  if (orientation < 2) op.SetMode(orientation == 1);
  else if (orientation == 2) op.SetMode(gp_Ax2(o, n, x));
  else throw std::invalid_argument("invalid sweep orientation");
  op.SetTransitionMode(BRepBuilderAPI_RightCorner);
  op.Add(TopoDS::Wire(profile_wire->shape), false, false);
  op.Build();
  if (!op.IsDone()) throw std::runtime_error("swept surface failed; check the path's continuity");
  TopoDS_Shape out = op.Shape();
  if (out.IsNull() || !BRepCheck_Analyzer(out).IsValid())
    throw std::runtime_error("swept surface produced invalid geometry");
  Maps maps(out);
  record(op, profile.shape, 0, maps, hist);
  record(op, path.shape, 1, maps, hist);
  record_caps(op.FirstShape(), FIRST_CAP, maps, hist);
  record_caps(op.LastShape(), LAST_CAP, maps, hist);
  return wrap(out);
}

std::unique_ptr<Shape> loft_surface(const Shape &profiles, bool ruled, bool preserve,
                                    rust::Vec<HistoryEntry> &hist) {
  BRepOffsetAPI_ThruSections op(false, ruled, 1e-7);
  int count = 0;
  for (TopExp_Explorer ex(profiles.shape, TopAbs_WIRE); ex.More(); ex.Next()) {
    op.AddWire(TopoDS::Wire(ex.Current()));
    ++count;
  }
  if (count < 2) throw std::invalid_argument("a lofted surface needs at least two profiles");
  op.CheckCompatibility(!preserve);
  op.SetMutableInput(false);
  op.Build();
  if (!op.IsDone()) throw std::runtime_error("lofted surface failed; check the profile order");
  TopoDS_Shape out = op.Shape();
  if (out.IsNull() || !BRepCheck_Analyzer(out).IsValid())
    throw std::runtime_error("lofted surface produced invalid geometry");
  Maps maps(out);
  record(op, profiles.shape, 0, maps, hist);
  record_caps(op.FirstShape(), FIRST_CAP, maps, hist);
  record_caps(op.LastShape(), LAST_CAP, maps, hist);
  return wrap(out);
}

std::unique_ptr<Shape> shells_of(const Shape &s) {
  BRep_Builder b;
  TopoDS_Compound out;
  b.MakeCompound(out);
  for (TopExp_Explorer ex(s.shape, TopAbs_SHELL); ex.More(); ex.Next()) b.Add(out, ex.Current());
  // Faces outside any shell (a bare face) count as shells of their own.
  for (TopExp_Explorer ex(s.shape, TopAbs_FACE, TopAbs_SHELL); ex.More(); ex.Next()) {
    TopoDS_Shell shell;
    b.MakeShell(shell);
    b.Add(shell, ex.Current());
    b.Add(out, shell);
  }
  return wrap(out);
}

std::unique_ptr<Shape> free_edges(const Shape &s) {
  TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
  TopExp::MapShapesAndAncestors(s.shape, TopAbs_EDGE, TopAbs_FACE, edge_faces);
  BRep_Builder b;
  TopoDS_Compound out;
  b.MakeCompound(out);
  for (int i = 1; i <= edge_faces.Extent(); ++i) {
    const TopoDS_Edge &e = TopoDS::Edge(edge_faces.FindKey(i));
    if (BRep_Tool::Degenerated(e)) continue;
    if (edge_faces(i).Extent() == 1) b.Add(out, e);
  }
  return wrap(out);
}

// The outward normal of a face as it sits in a shape, at its middle.
static gp_Vec mid_normal(const TopoDS_Face &face) {
  double u0, u1, v0, v1;
  BRepTools::UVBounds(face, u0, u1, v0, v1);
  BRepGProp_Face prop(face);
  gp_Pnt p;
  gp_Vec n;
  prop.Normal(0.5 * (u0 + u1), 0.5 * (v0 + v1), p, n);
  return n;
}

std::unique_ptr<Shape> cut_with_surface(const Shape &body, const Shape &surface, bool flip,
                                        rust::Vec<HistoryEntry> &hist) {
  BRepAlgoAPI_Splitter op;
  TopTools_ListOfShape args, tools;
  args.Append(body.shape);
  tools.Append(surface.shape);
  op.SetArguments(args);
  op.SetTools(tools);
  op.SetNonDestructive(true);
  op.Build();
  if (!op.IsDone() || op.HasErrors()) throw std::runtime_error("the surface can't cut the solids");
  // Each piece of the cutting surface, with the surface's normal there.
  TopTools_IndexedMapOfShape tool_faces;
  std::vector<gp_Vec> tool_normals;
  for (TopExp_Explorer ex(surface.shape, TopAbs_FACE); ex.More(); ex.Next()) {
    const TopoDS_Face &f = TopoDS::Face(ex.Current());
    const TopTools_ListOfShape &images = op.Modified(f);
    if (images.IsEmpty()) {
      tool_faces.Add(f);
      tool_normals.push_back(mid_normal(f));
    }
    for (const TopoDS_Shape &image : images) {
      if (tool_faces.Add(image) > static_cast<int>(tool_normals.size()))
        tool_normals.push_back(mid_normal(TopoDS::Face(image)));
    }
  }
  BRep_Builder b;
  TopoDS_Compound out;
  b.MakeCompound(out);
  int split = 0, kept = 0;
  for (TopExp_Explorer sx(op.Shape(), TopAbs_SOLID); sx.More(); sx.Next()) {
    int side = 0;
    for (TopExp_Explorer fx(sx.Current(), TopAbs_FACE); fx.More() && side == 0; fx.Next()) {
      int i = tool_faces.FindIndex(fx.Current());
      if (i == 0) continue;
      // The piece's outward normal on the cut face, against the surface's.
      gp_Vec outward = mid_normal(TopoDS::Face(fx.Current()));
      double d = outward.Dot(tool_normals[i - 1]);
      side = d > 0 ? -1 : 1;
    }
    if (side != 0) ++split;
    // Material on the normal's side (+1) goes, unless flipped.
    bool remove = side != 0 && ((side > 0) != flip);
    if (!remove) {
      b.Add(out, sx.Current());
      ++kept;
    }
  }
  if (split == 0) throw std::runtime_error("the surface doesn't cut through the solids");
  if (kept == 0) throw std::runtime_error("the cut removes every solid");
  TopoDS_Shape result = out;
  Maps maps(result);
  record(op, body.shape, 0, maps, hist);
  record(op, surface.shape, 1, maps, hist);
  return wrap(result);
}

std::unique_ptr<Shape> planar_face(const Shape &edges) {
  Handle(TopTools_HSequenceOfShape) seq = new TopTools_HSequenceOfShape;
  for (TopExp_Explorer ex(edges.shape, TopAbs_EDGE); ex.More(); ex.Next()) seq->Append(ex.Current());
  if (seq->IsEmpty()) throw std::invalid_argument("select edges forming a closed loop");
  Handle(TopTools_HSequenceOfShape) wires;
  ShapeAnalysis_FreeBounds::ConnectEdgesToWires(seq, 1e-5, false, wires);
  if (wires->Length() != 1) throw std::invalid_argument("the edges must form one closed loop");
  const TopoDS_Wire &wire = TopoDS::Wire(wires->Value(1));
  if (!BRep_Tool::IsClosed(wire)) throw std::invalid_argument("the edges don't form a closed loop");
  BRepBuilderAPI_MakeFace make(wire, true);
  if (!make.IsDone()) throw std::invalid_argument("the edges don't lie in one plane");
  return wrap(make.Face());
}

uint32_t child_count(const Shape &s) {
  uint32_t n = 0;
  for (TopoDS_Iterator it(s.shape); it.More(); it.Next()) ++n;
  return n;
}

std::unique_ptr<Shape> child(const Shape &s, uint32_t index) {
  uint32_t n = 0;
  for (TopoDS_Iterator it(s.shape); it.More(); it.Next(), ++n)
    if (n == index) return wrap(it.Value());
  throw std::out_of_range("no such child shape");
}

} // namespace ofk

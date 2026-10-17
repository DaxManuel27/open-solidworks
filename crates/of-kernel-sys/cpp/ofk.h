// The C++ half of the OpenCASCADE bridge. Everything here is called from
// `src/lib.rs` through cxx; nothing else in OpenForge touches C++.
#pragma once

#include "rust/cxx.h"

#include <Standard_Failure.hxx>
#include <TopoDS_Shape.hxx>

#include <exception>
#include <memory>

// Turn kernel exceptions into Rust `Err` values instead of aborting.
namespace rust {
namespace behavior {
template <typename Try, typename Fail>
static void trycatch(Try &&func, Fail &&fail) noexcept try {
  func();
} catch (const Standard_Failure &e) {
  const char *msg = e.GetMessageString();
  fail(msg && *msg ? msg : e.DynamicType()->Name());
} catch (const std::exception &e) {
  fail(e.what());
} catch (...) {
  fail("unknown geometry kernel error");
}
} // namespace behavior
} // namespace rust

namespace ofk {

struct Vec3;
struct Frame;
struct Curve;
struct HistoryEntry;
struct Mesh;
struct MassProps;
struct BBox;
struct FaceInfo;
struct EdgeInfo;
struct Distance;
struct StepNode;
struct HlrEdge;
struct FaceFrame;

class Shape {
public:
  Shape() = default;
  explicit Shape(TopoDS_Shape s) : shape(std::move(s)) {}
  TopoDS_Shape shape;
};

// Assembly STEP writer and reader; their XCAF state lives in assembly.cpp.
class StepAssembly {
public:
  struct Impl;
  StepAssembly();
  ~StepAssembly();
  std::unique_ptr<Impl> impl;
};

class StepRead {
public:
  struct Impl;
  StepRead();
  ~StepRead();
  std::unique_ptr<Impl> impl;
};

std::unique_ptr<Shape> null_shape();
std::unique_ptr<Shape> clone_shape(const Shape &s);
bool is_null(const Shape &s);
bool is_valid(const Shape &s);
uint32_t count(const Shape &s, uint8_t kind);
std::unique_ptr<Shape> sub_shape(const Shape &s, uint8_t kind, uint32_t index);
rust::Vec<uint32_t> indices_in(const Shape &part, const Shape &whole, uint8_t kind);
std::unique_ptr<Shape> compound_new();
void compound_add(Shape &compound, const Shape &s);

std::unique_ptr<Shape> make_box(double dx, double dy, double dz);
std::unique_ptr<Shape> make_faces(const Frame &frame, rust::Slice<const Curve> curves,
                                  rust::Slice<const uint32_t> loop_sizes,
                                  rust::Slice<const uint32_t> face_loops);
std::unique_ptr<Shape> make_wire(const Frame &frame, rust::Slice<const Curve> curves);
std::unique_ptr<Shape> make_wire3(rust::Slice<const Curve> curves);
std::unique_ptr<Shape> join_wires(const Shape &curves);
std::unique_ptr<Shape> reverse_shape(const Shape &shape);
std::unique_ptr<Shape> make_helix(const Frame &frame, double radius, double pitch, double turns, double start, bool left);
std::unique_ptr<Shape> make_helix_law(const Frame &frame, double radius, rust::Slice<const double> turns,
                                      rust::Slice<const double> pitches, double start, bool left);
std::unique_ptr<Shape> section(const Shape &a, const Shape &b);
std::unique_ptr<Shape> project_wire(const Shape &wire, const Shape &target, const Vec3 &direction);
std::unique_ptr<Shape> split_face(const Shape &body, uint32_t face, const Shape &wire, rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> sweep(const Shape &profile, const Shape &path, const Frame &frame, uint8_t orientation, rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> loft(const Shape &profiles, bool ruled, bool preserve_correspondence, rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> half_space(const Shape &face, const Vec3 &inside);
std::unique_ptr<Shape> offset_face(const Shape &face, double distance, const Vec3 &toward);
std::unique_ptr<Shape> revol_until(const Shape &s, const Shape &until, const Vec3 &origin,
                                  const Vec3 &d, rust::Vec<HistoryEntry> &hist);

std::unique_ptr<Shape> prism(const Shape &s, const Vec3 &dir, rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> split_plane(const Shape &s, const Frame &plane, rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> plane_face(const Frame &plane);
std::unique_ptr<Shape> split_tools(const Shape &s, const Shape &tools, rust::Vec<HistoryEntry> &hist);
int8_t face_orientation(const Shape &s, uint32_t face);
std::unique_ptr<Shape> wrap_faces(const Shape &body, uint32_t face, const Frame &frame,
                                  rust::Slice<const Curve> curves,
                                  rust::Slice<const uint32_t> loop_sizes,
                                  rust::Slice<const uint32_t> face_loops);
std::unique_ptr<Shape> wrap_solid(const Shape &faces, double depth, bool outward);
Frame wire_start(const Shape &path);
std::unique_ptr<Shape> sweep_aux(const Shape &profile, const Shape &path, const Shape &guide, double twist,
                                 rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> thread_tool(const Frame &frame, double radius, double pitch, double length,
                                   bool left, bool internal);
std::unique_ptr<Shape> boundary(const Shape &sections, const Shape &guides,
                                rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> skin(const Shape &sections, bool preserve, uint8_t start_mode, const Vec3 &start_dir,
                            double start_weight, uint8_t end_mode, const Vec3 &end_dir, double end_weight,
                            rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> sweep_scaled(const Shape &profile, const Shape &path, const Shape &guide1,
                                    const Shape &guide2, bool scale_section, rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> loft_centerline(const Shape &sections, const Shape &centerline, rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> dome_elliptical(const Shape &s, uint32_t face, double height, bool inward, rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> dome(const Shape &s, uint32_t face, double height, bool inward, rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> revol(const Shape &s, const Vec3 &origin, const Vec3 &dir, double angle,
                             rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> boolean(uint8_t op, const Shape &a, const Shape &b,
                               rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> fillet(const Shape &s, rust::Slice<const uint32_t> edges, double radius,
                              rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> variable_fillet(const Shape &s, uint32_t edge, rust::Slice<const double> stations, bool straight, rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> draft_planes(const Shape &s, rust::Slice<const uint32_t> faces, rust::Slice<const Frame> planes, const Vec3 &direction, double angle, rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> draft(const Shape &s, rust::Slice<const uint32_t> faces, const Frame &plane, const Vec3 &direction, double angle, rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> chamfer(const Shape &s, rust::Slice<const uint32_t> edges, double d1,
                               double d2, rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> shell(const Shape &s, rust::Slice<const uint32_t> faces, double thickness,
                             rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> transform(const Shape &s, rust::Slice<const double> matrix,
                                 rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> mirror(const Shape &s, const Vec3 &origin, const Vec3 &normal,
                              rust::Vec<HistoryEntry> &hist);

rust::Vec<uint32_t> edge_faces(const Shape &s);
rust::Vec<uint32_t> face_edges(const Shape &s);
rust::Vec<uint32_t> vertex_faces(const Shape &s);
Mesh tessellate(const Shape &s, double linear, double angular);
rust::Vec<double> edge_samples(const Shape &s, uint32_t edge, double linear, double angular);
MassProps mass_props(const Shape &s);
BBox bbox(const Shape &s);
FaceInfo face_info(const Shape &s, uint32_t face);
EdgeInfo edge_info(const Shape &s, uint32_t edge);
Vec3 vertex_point(const Shape &s, uint32_t vertex);
Distance distance(const Shape &a, const Shape &b);

void write_step(const Shape &s, rust::Str path);
std::unique_ptr<Shape> read_step(rust::Str path);
void write_iges(const Shape &s, rust::Str path);
std::unique_ptr<Shape> read_iges(rust::Str path);
void write_stl(const Shape &s, rust::Str path, double linear, double angular, bool ascii);

std::unique_ptr<Shape> located(const Shape &s, rust::Slice<const double> m);
double common_volume(const Shape &a, const Shape &b);
std::unique_ptr<Shape> common_shape(const Shape &a, const Shape &b);
std::unique_ptr<StepAssembly> step_assembly_new(rust::Str name);
uint32_t step_assembly_part(StepAssembly &a, rust::Str name, const Shape &s);
uint32_t step_assembly_node(StepAssembly &a, rust::Str name);
void step_assembly_place(StepAssembly &a, uint32_t parent, uint32_t child, rust::Str name,
                         rust::Slice<const double> m);
void step_assembly_write(StepAssembly &a, rust::Str path);
std::unique_ptr<StepRead> step_read(rust::Str path);
uint32_t step_read_count(const StepRead &r);
StepNode step_read_node(const StepRead &r, uint32_t i);
std::unique_ptr<Shape> step_read_shape(const StepRead &r, uint32_t i);
rust::Vec<HlrEdge> hlr(const Shape &s, const Frame &view, bool exact, bool hidden,
                       double deflection);
FaceFrame face_frame(const Shape &s, uint32_t face);
std::unique_ptr<Shape> fold_faces(const Shape &s, rust::Slice<const uint32_t> faces,
                                  rust::Slice<const double> maps, const Frame &frame,
                                  const Shape &region, rust::Vec<uint32_t> &owners);
std::unique_ptr<Shape> polyhedron(rust::Slice<const double> points,
                                  rust::Slice<const uint32_t> faces);
std::unique_ptr<Shape> flatten(const Shape &s, rust::Slice<const uint32_t> faces,
                               rust::Slice<const double> maps, const Frame &frame,
                               const Vec3 &thickness, rust::Vec<int64_t> &sources, bool simplify);
rust::Vec<uint32_t> seam_edges(const Shape &s, const Vec3 &view);
std::unique_ptr<Shape> sew(const Shape &faces, double tolerance, rust::Vec<HistoryEntry> &hist,
                           uint32_t &free_edges);
std::unique_ptr<Shape> solidify(const Shape &shells, rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> offset_shape(const Shape &s, double distance, rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> thicken(const Shape &s, double thickness, rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> defeature(const Shape &s, rust::Slice<const uint32_t> faces,
                                 rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> remove_faces(const Shape &s, rust::Slice<const uint32_t> faces,
                                    rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> fill(const Shape &edges, const Shape &supports,
                            rust::Slice<const int32_t> support_of, rust::Slice<const uint8_t> orders,
                            const Shape &constraints, double &max_error);
std::unique_ptr<Shape> sweep_surface(const Shape &profile, const Shape &path, const Frame &frame,
                                     uint8_t orientation, rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> loft_surface(const Shape &profiles, bool ruled, bool preserve,
                                    rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> shells_of(const Shape &s);
std::unique_ptr<Shape> free_edges(const Shape &s);
uint32_t child_count(const Shape &s);
std::unique_ptr<Shape> cut_with_surface(const Shape &body, const Shape &surface, bool flip,
                                        rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> planar_face(const Shape &edges);
std::unique_ptr<Shape> extend_face(const Shape &face_in, double length);
std::unique_ptr<Shape> cells(const Shape &faces);
int8_t classify(const Shape &solid, const Vec3 &point);
Vec3 inner_point(const Shape &solid);
rust::Vec<uint32_t> faces_on(const Shape &s, const Shape &target, double tolerance);
std::unique_ptr<Shape> unify(const Shape &s, rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> remove_holes(const Shape &s, rust::Slice<const uint32_t> faces,
                                    rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> boundary_surface(const Shape &sections_in, const Shape &guides_in,
                                        rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> child(const Shape &s, uint32_t index);

} // namespace ofk

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

class Shape {
public:
  Shape() = default;
  explicit Shape(TopoDS_Shape s) : shape(std::move(s)) {}
  TopoDS_Shape shape;
};

std::unique_ptr<Shape> null_shape();
std::unique_ptr<Shape> clone_shape(const Shape &s);
bool is_null(const Shape &s);
bool is_valid(const Shape &s);
uint32_t count(const Shape &s, uint8_t kind);
std::unique_ptr<Shape> sub_shape(const Shape &s, uint8_t kind, uint32_t index);
std::unique_ptr<Shape> compound_new();
void compound_add(Shape &compound, const Shape &s);

std::unique_ptr<Shape> make_box(double dx, double dy, double dz);
std::unique_ptr<Shape> make_faces(const Frame &frame, rust::Slice<const Curve> curves,
                                  rust::Slice<const uint32_t> loop_sizes,
                                  rust::Slice<const uint32_t> face_loops);
std::unique_ptr<Shape> make_wire(const Frame &frame, rust::Slice<const Curve> curves);
std::unique_ptr<Shape> half_space(const Shape &face, const Vec3 &inside);

std::unique_ptr<Shape> prism(const Shape &s, const Vec3 &dir, rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> revol(const Shape &s, const Vec3 &origin, const Vec3 &dir, double angle,
                             rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> boolean(uint8_t op, const Shape &a, const Shape &b,
                               rust::Vec<HistoryEntry> &hist);
std::unique_ptr<Shape> fillet(const Shape &s, rust::Slice<const uint32_t> edges, double radius,
                              rust::Vec<HistoryEntry> &hist);
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

} // namespace ofk

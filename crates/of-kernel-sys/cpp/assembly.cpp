// Assembly support: cheap placement of shapes, and STEP files that keep the
// product structure (parts placed as instances inside assemblies).

#include "of-kernel-sys/cpp/ofk.h"
#include "of-kernel-sys/src/lib.rs.h"

#include <BRepAlgoAPI_Common.hxx>
#include <BRepBndLib.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepGProp.hxx>
#include <Bnd_Box.hxx>
#include <GProp_GProps.hxx>
#include <IFSelect_ReturnStatus.hxx>
#include <Interface_Static.hxx>
#include <Message.hxx>
#include <Message_Messenger.hxx>
#include <Message_PrinterOStream.hxx>
#include <STEPCAFControl_Reader.hxx>
#include <STEPCAFControl_Writer.hxx>
#include <TDF_Label.hxx>
#include <TDF_LabelSequence.hxx>
#include <TDataStd_Name.hxx>
#include <TDocStd_Document.hxx>
#include <TopExp_Explorer.hxx>
#include <TopLoc_Location.hxx>
#include <TopoDS.hxx>
#include <XCAFApp_Application.hxx>
#include <XCAFDoc_DocumentTool.hxx>
#include <XCAFDoc_ShapeTool.hxx>
#include <gp_Trsf.hxx>

#include <stdexcept>
#include <string>
#include <vector>

namespace ofk {

namespace {

gp_Trsf trsf_of(rust::Slice<const double> m) {
  if (m.size() != 12) throw std::invalid_argument("a placement needs a 3x4 matrix");
  gp_Trsf t;
  t.SetValues(m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8], m[9], m[10], m[11]);
  return t;
}

rust::Vec<double> matrix_of(const gp_Trsf &t) {
  rust::Vec<double> out;
  for (int r = 1; r <= 3; ++r) {
    for (int c = 1; c <= 4; ++c) out.push_back(t.Value(r, c));
  }
  return out;
}

Handle(TDocStd_Document) new_document() {
  Handle(XCAFApp_Application) app = XCAFApp_Application::GetApplication();
  Handle(TDocStd_Document) doc;
  app->NewDocument("MDTV-XCAF", doc);
  if (doc.IsNull()) throw std::runtime_error("could not create an assembly document");
  return doc;
}

std::string label_name(const TDF_Label &l) {
  Handle(TDataStd_Name) n;
  if (!l.FindAttribute(TDataStd_Name::GetID(), n)) return "";
  TCollection_AsciiString s(n->Get());
  return std::string(s.ToCString());
}

// Data exchange prints progress to stdout, which carries the core's
// protocol: remove the console printers before reading or writing.
void quiet_step() {
  Message::DefaultMessenger()->RemovePrinters(STANDARD_TYPE(Message_PrinterOStream));
  Interface_Static::SetCVal("write.step.unit", "MM");
}

} // namespace

std::unique_ptr<Shape> located(const Shape &s, rust::Slice<const double> m) {
  return std::make_unique<Shape>(s.shape.Moved(TopLoc_Location(trsf_of(m))));
}

double common_volume(const Shape &a, const Shape &b) {
  Bnd_Box ba, bb;
  BRepBndLib::Add(a.shape, ba);
  BRepBndLib::Add(b.shape, bb);
  if (ba.IsOut(bb)) return 0.0;
  BRepAlgoAPI_Common op(a.shape, b.shape);
  if (!op.IsDone()) throw std::runtime_error("could not intersect the components");
  GProp_GProps props;
  BRepGProp::VolumeProperties(op.Shape(), props);
  return props.Mass();
}

std::unique_ptr<Shape> common_shape(const Shape &a, const Shape &b) {
  BRepAlgoAPI_Common op(a.shape, b.shape);
  if (!op.IsDone()) throw std::runtime_error("could not intersect the components");
  return std::make_unique<Shape>(op.Shape());
}

struct StepAssembly::Impl {
  Handle(TDocStd_Document) doc;
  Handle(XCAFDoc_ShapeTool) tool;
  std::vector<TDF_Label> labels;
};

StepAssembly::StepAssembly() : impl(std::make_unique<Impl>()) {}
StepAssembly::~StepAssembly() = default;

std::unique_ptr<StepAssembly> step_assembly_new(rust::Str name) {
  auto out = std::make_unique<StepAssembly>();
  auto &a = *out->impl;
  a.doc = new_document();
  a.tool = XCAFDoc_DocumentTool::ShapeTool(a.doc->Main());
  TDF_Label root = a.tool->NewShape();
  TDataStd_Name::Set(root, TCollection_ExtendedString(std::string(name).c_str(), true));
  a.labels.push_back(root);
  return out;
}

uint32_t step_assembly_part(StepAssembly &sa, rust::Str name, const Shape &s) {
  auto &a = *sa.impl;
  TDF_Label l = a.tool->AddShape(s.shape, false);
  TDataStd_Name::Set(l, TCollection_ExtendedString(std::string(name).c_str(), true));
  a.labels.push_back(l);
  return static_cast<uint32_t>(a.labels.size() - 1);
}

uint32_t step_assembly_node(StepAssembly &sa, rust::Str name) {
  auto &a = *sa.impl;
  TDF_Label l = a.tool->NewShape();
  TDataStd_Name::Set(l, TCollection_ExtendedString(std::string(name).c_str(), true));
  a.labels.push_back(l);
  return static_cast<uint32_t>(a.labels.size() - 1);
}

void step_assembly_place(StepAssembly &sa, uint32_t parent, uint32_t child, rust::Str name,
                         rust::Slice<const double> m) {
  auto &a = *sa.impl;
  if (parent >= a.labels.size() || child >= a.labels.size()) {
    throw std::invalid_argument("unknown assembly label");
  }
  TDF_Label c = a.tool->AddComponent(a.labels[parent], a.labels[child], TopLoc_Location(trsf_of(m)));
  if (c.IsNull()) throw std::runtime_error("could not place a component");
  TDataStd_Name::Set(c, TCollection_ExtendedString(std::string(name).c_str(), true));
}

void step_assembly_write(StepAssembly &sa, rust::Str path) {
  auto &a = *sa.impl;
  quiet_step();
  a.tool->UpdateAssemblies();
  STEPCAFControl_Writer w;
  w.SetNameMode(true);
  w.SetColorMode(false);
  if (!w.Transfer(a.doc, STEPControl_AsIs)) throw std::runtime_error("could not convert the assembly to STEP");
  if (w.Write(std::string(path).c_str()) != IFSelect_RetDone) throw std::runtime_error("could not write STEP file");
}

struct StepRead::Impl {
  std::vector<StepNode> nodes;
  std::vector<TopoDS_Shape> shapes;
};

StepRead::StepRead() : impl(std::make_unique<Impl>()) {}
StepRead::~StepRead() = default;

namespace {

void walk(StepRead::Impl &out, const Handle(XCAFDoc_ShapeTool) &tool, const TDF_Label &label, int64_t parent,
          const gp_Trsf &placement, const std::string &instance_name) {
  TDF_Label ref = label;
  if (XCAFDoc_ShapeTool::IsReference(label)) XCAFDoc_ShapeTool::GetReferredShape(label, ref);
  std::string name = instance_name.empty() ? label_name(ref) : instance_name;
  bool assembly = XCAFDoc_ShapeTool::IsAssembly(ref);
  StepNode node;
  node.parent = parent;
  node.name = rust::String(name);
  node.assembly = assembly;
  node.matrix = matrix_of(placement);
  node.part = label_name(ref);
  int64_t index = static_cast<int64_t>(out.nodes.size());
  out.nodes.push_back(node);
  out.shapes.push_back(assembly ? TopoDS_Shape() : XCAFDoc_ShapeTool::GetShape(ref));
  if (!assembly) return;
  TDF_LabelSequence comps;
  XCAFDoc_ShapeTool::GetComponents(ref, comps, false);
  for (Standard_Integer i = 1; i <= comps.Length(); ++i) {
    TDF_Label comp = comps.Value(i);
    gp_Trsf t = XCAFDoc_ShapeTool::GetLocation(comp).Transformation();
    walk(out, tool, comp, index, t, label_name(comp));
  }
}

} // namespace

std::unique_ptr<StepRead> step_read(rust::Str path) {
  quiet_step();
  Handle(TDocStd_Document) doc = new_document();
  STEPCAFControl_Reader r;
  r.SetNameMode(true);
  r.SetColorMode(false);
  if (r.ReadFile(std::string(path).c_str()) != IFSelect_RetDone) throw std::runtime_error("could not read STEP file");
  if (!r.Transfer(doc)) throw std::runtime_error("could not convert the STEP file");
  Handle(XCAFDoc_ShapeTool) tool = XCAFDoc_DocumentTool::ShapeTool(doc->Main());
  TDF_LabelSequence roots;
  tool->GetFreeShapes(roots);
  auto out = std::make_unique<StepRead>();
  for (Standard_Integer i = 1; i <= roots.Length(); ++i) {
    TDF_Label root = roots.Value(i);
    gp_Trsf t = XCAFDoc_ShapeTool::GetLocation(root).Transformation();
    walk(*out->impl, tool, root, -1, t, "");
  }
  if (out->impl->nodes.empty()) throw std::runtime_error("STEP file contains no geometry");
  return out;
}

uint32_t step_read_count(const StepRead &r) { return static_cast<uint32_t>(r.impl->nodes.size()); }

StepNode step_read_node(const StepRead &r, uint32_t i) {
  if (i >= r.impl->nodes.size()) throw std::out_of_range("no such STEP node");
  return r.impl->nodes[i];
}

std::unique_ptr<Shape> step_read_shape(const StepRead &r, uint32_t i) {
  const auto &shapes = r.impl->shapes;
  if (i >= shapes.size() || shapes[i].IsNull()) throw std::out_of_range("the STEP node has no shape");
  return std::make_unique<Shape>(shapes[i]);
}

} // namespace ofk

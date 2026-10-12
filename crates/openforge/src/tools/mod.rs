//! The tool catalog shared by MCP clients and the built-in assistant. Input
//! schemas are generated from the Rust argument types in [`params`]; examples
//! are validated against them and replayed on the core by the tests.

pub mod asm_params;
pub mod envelope;
pub mod generate;
pub mod params;

use std::sync::OnceLock;

use of_protocol::{ErrorCode, RpcError};
use schemars::generate::SchemaSettings;
use schemars::{JsonSchema, SchemaGenerator};
use serde_json::{Map, Value, json};

use asm_params::*;
use params::*;

/// One catalog entry.
pub struct Tool {
    pub name: &'static str,
    pub method: &'static str,
    pub description: &'static str,
    pub read_only: bool,
    /// Available to the built-in assistant inside a draft. File operations
    /// and undo stay with the user.
    pub draft: bool,
    schema: fn(&mut SchemaGenerator) -> schemars::Schema,
    examples: fn() -> Vec<Value>,
}

/// A catalog entry with its generated, strict input schema.
pub struct Entry {
    pub tool: &'static Tool,
    pub schema: Value,
    validator: jsonschema::Validator,
}

fn schema<T: JsonSchema>(g: &mut SchemaGenerator) -> schemars::Schema {
    g.subschema_for::<T>()
}

/// Body ID of the example part's plate (see `generate::tests`).
pub const EXAMPLE_BODY: &str = "b2:17a163b7910805aa";

macro_rules! tool {
    ($name:literal, $method:literal, $params:ty, read_only: $ro:literal, draft: $draft:literal,
     $description:literal, [$($example:tt),* $(,)?]) => {
        Tool {
            name: $name,
            method: $method,
            description: $description,
            read_only: $ro,
            draft: $draft,
            schema: schema::<$params>,
            examples: || vec![$(json!($example)),*],
        }
    };
}

pub const TOOLS: &[Tool] = &[
    tool!("core_info", "core.info", Empty, read_only: true, draft: true,
        "Protocol, scene and part-file versions and the capabilities this core implements. Check it before relying on a feature type.",
        [{}]),
    tool!("part_info", "doc.info", Empty, read_only: true, draft: true,
        "The feature tree with each feature's error, units, material, bodies, variables, equations, registered parameters (stable keys such as f2.end1.depth) and configurations. Read it before editing.",
        [{}]),
    tool!("get_feature", "doc.feature", FeatureRef, read_only: true, draft: true,
        "The complete authored definition of one feature, including sketch geometry and relations. Read it before update_feature, which replaces the whole definition.",
        [{"id": 2}]),
    tool!("topology", "doc.topology", Empty, read_only: true, draft: true,
        "Faces and edges of the visible model with stable refs, surface or curve type, size, centroid, normal and endpoints (mm). Use the refs in fillet, chamfer, shell, draft, sketch-on-face and up-to-surface features.",
        [{}]),
    tool!("get_sketch", "sketch.get", FeatureRef, read_only: true, draft: true,
        "Points, curves, relations and definition status (under, fully or over defined) of one sketch. Use it to find entity IDs before adding relations.",
        [{"id": 3}]),
    tool!("solve_spatial_sketch", "sketch3d.solve", SolveSpatialSketch, read_only: true, draft: true,
        "Solve or drag a supplied spatial sketch without changing the document. Returns sketch and solver DOF/conflict report. Commit the returned sketch with add_feature/update_feature type sketch3d. Failed solves preserve the supplied geometry.",
        [{"sketch": {"entities": [], "constraints": [], "next_id": 1}}]),
    tool!("measure", "measure", Measure, read_only: true, draft: true,
        "Lengths, areas, radii, distances and angles of face, edge or vertex refs in the visible model. Results are mm and radians.",
        [{"refs": [{"type": "face", "name": "2.end"}]},
         {"refs": [{"type": "face", "name": "2.start"}, {"type": "face", "name": "2.end"}]}]),
    tool!("mass_properties", "mass_properties", Empty, read_only: true, draft: true,
        "Volume (mm³), surface area (mm²), mass (kg) from the part's material, center of mass and inertia of the active configuration. Use it to check the requested size.",
        [{}]),
    tool!("new_sketch", "sketch.new", NewSketch, read_only: false, draft: true,
        "Create an empty sketch on a standard plane, a reference plane or a planar face, and start editing it. Returns the sketch ID as `id`. Add geometry with edit_sketch, then close_sketch.",
        [{"plane": {"type": "front_plane"}},
         {"plane": {"type": "face", "name": "2.end"}}]),
    tool!("edit_sketch", "sketch.edit", EditSketch, read_only: false, draft: true,
        "Apply one edit to a sketch: add geometry, relations or dimensions, change a value, or delete. Returns the solved sketch, its definition status and the new entity IDs in `created`. Coordinates are sketch-plane mm.",
        [{"id": 3, "op": {"op": "rectangle", "a": [5, 5], "b": [20, 15]}},
         {"id": 3, "op": {"op": "add_circle", "center": {"at": [45, 20]}, "radius": 4}},
         {"id": 3, "op": {"op": "add_line", "a": {"id": 3}, "b": {"at": [10, 30]}}},
         {"id": 3, "op": {"op": "add_point", "at": [50, 30]}},
         {"id": 3, "op": {"op": "slot", "a": [10, 30], "b": [25, 30], "width": 4}},
         {"id": 3, "op": {"op": "polygon", "center": [45, 30], "vertex": [48, 30], "sides": 6}},
         {"id": 3, "op": {"op": "constraint", "constraint": {"type": "horizontal_points", "a": 1, "b": 3}}},
         {"id": 3, "op": {"op": "dimension", "constraint": {"type": "horizontal_distance", "a": 0, "b": 1, "value": 0}, "value": 30}},
         {"id": 3, "op": {"op": "set_value", "id": 4, "value": 12}},
         {"id": 3, "op": {"op": "delete", "ids": [3]}}]),
    tool!("close_sketch", "sketch.close", Empty, read_only: false, draft: true,
        "Finish editing the open sketch. Close it before adding the feature that uses it. This does not commit the assistant's draft.",
        [{}]),
    tool!("add_feature", "feature.add", AddFeature, read_only: false, draft: true,
        "Add a feature after the last one and rebuild. Returns its ID as `id`. Check `features[].error` in the result: a geometry failure is stored on the feature, not raised. A cut sketched on a face needs `reverse: true` to go into the material.",
        [{"feature": {"type": "extrude", "sketch": 4, "end1": {"type": "blind", "depth": 10}}},
         {"feature": {"type": "extrude", "sketch": 3, "cut": true, "end1": {"type": "through_all"}}},
         {"feature": {"type": "revolve", "sketch": 4, "axis": {"type": "sketch_entity", "sketch": 4, "entity": 8}, "angle": {"type": "blind", "angle": std::f64::consts::TAU}}},
         {"feature": {"type": "fillet", "edges": [{"type": "edge", "name": "E(2.end|2.s5)"}], "radius": 1}},
         {"feature": {"type": "chamfer", "edges": [{"type": "edge", "name": "E(2.end|2.s5)"}], "mode": {"type": "equal", "distance": 1}}},
         {"feature": {"type": "shell", "faces": [{"type": "face", "name": "2.end"}], "thickness": 1}},
         {"feature": {"type": "hole", "sketch": 3, "kind": "counterbore", "standard": "iso", "size": "M3", "end": {"type": "through_all"}, "reverse": true}},
         {"feature": {"type": "plane", "mode": "offset", "reference": {"type": "front_plane"}, "distance": 20}},
         {"feature": {"type": "linear_pattern", "features": [2], "direction1": {"reference": {"type": "right_plane"}, "spacing": 70, "count": 2}}},
         {"feature": {"type": "mirror", "features": [2], "plane": {"type": "right_plane"}}},
         {"feature": {"type": "move_body", "bodies": [EXAMPLE_BODY], "translation": [0, 0, 10], "copy": true}},
         {"feature": {"type": "helix", "plane": {"type": "top_plane"}, "radius": 5, "pitch": 2, "turns": 3}, "name": "Spring path"}]),
    tool!("update_feature", "feature.update", UpdateFeature, read_only: false, draft: true,
        "Replace one feature's complete definition (same shape as add_feature) and rebuild from it. Read get_feature first and keep fields you are not changing. For one dimension, set_parameter is simpler.",
        [{"id": 2, "feature": {"type": "extrude", "sketch": 1, "end1": {"type": "blind", "depth": 8}}}]),
    tool!("delete_features", "feature.delete", DeleteFeatures, read_only: false, draft: true,
        "Delete features and every feature that depends on them. Returns the deleted IDs as `deleted`. Do not delete existing work to fix an error unless the user asked.",
        [{"ids": [3]}]),
    tool!("set_parameter", "parameter.set", SetParameter, read_only: false, draft: true,
        "Set one base parameter by its stable key from part_info `parameters`, in mm or radians. Removes that parameter's base equation; configuration overrides still apply. To change one configuration only, use update_configuration.",
        [{"key": "f2.end1.depth", "value": 8}]),
    tool!("set_equations", "equations.set", SetEquations, read_only: false, draft: true,
        "Replace shared base variables and equations, or the selected configuration's overrides. Optional parameters atomically set numeric values; omitted configuration edits the base, and removing variant entries resets them to base. Formulas use + - * /, min, max, abs, sqrt, sin, cos, tan, pi and units mm, cm, m, in, ft, deg, rad; lengths need units. Check part_info `equation_values` and feature errors afterwards.",
        [{"variables": {"thickness": "8mm"}, "equations": {"f2.end1.depth": "thickness"}}]),
    tool!("locate_reference", "ref.locate", LocateRef, read_only: true, draft: true,
        "Where a reference is now: `point` for a vertex or reference point, `plane` (origin and normal) for a plane or planar face. Values are mm.",
        [{"reference": {"type": "front_plane"}}]),
    tool!("evaluate_formula", "equations.evaluate", EvaluateFormula, read_only: true, draft: true,
        "Evaluate a formula with the part's variables and parameters (active configuration) without changing anything. Returns `value` with its value and unit powers.",
        [{"expression": "f2.end1.depth * 2"}]),
    tool!("create_configuration", "configuration.create", CreateConfiguration, read_only: false, draft: true,
        "Create a configuration and make it active. It inherits base values unless copy_from names a configuration to duplicate. Read its ID from part_info `configurations.items`.",
        [{"name": "Thin"}, {"name": "Long copy", "copy_from": 2}]),
    tool!("update_configuration", "configuration.update", UpdateConfiguration, read_only: false, draft: true,
        "Replace one configuration's overrides, keeping its ID: parameter values (mm, radians), equations, variables, suppressed features and material. Features themselves are shared by every configuration.",
        [{"configuration": {"id": 1, "name": "Default", "parameters": {"f2.end1.depth": 8}}},
         {"configuration": {"id": 2, "name": "Long", "suppressed": {"3": true}, "material": {"mode": "set", "value": {"name": "PLA", "density": 1240}}}}]),
    tool!("activate_configuration", "configuration.activate", ConfigurationRef, read_only: false, draft: true,
        "Make a configuration active, end any feature edit and rebuild the part with its overrides.",
        [{"id": 2}]),
    tool!("delete_configuration", "configuration.delete", ConfigurationRef, read_only: false, draft: true,
        "Delete a configuration. At least one must remain.",
        [{"id": 2}]),
    tool!("body_display", "body.display", BodyDisplay, read_only: false, draft: true,
        "Rename, hide or show a body by its stable ID from part_info `bodies`. Hidden bodies still count for mass and export.",
        [{"id": EXAMPLE_BODY, "name": "Plate"}, {"id": EXAMPLE_BODY, "hidden": true}]),
    tool!("set_material", "doc.set_material", SetMaterial, read_only: false, draft: true,
        "Set the base material by library name, or a custom name with density in kg/m³. Omit both to clear it.",
        [{"name": "Aluminum 6061"}, {"name": "Resin", "density": 1150}]),
    tool!("new_part", "doc.new", Empty, read_only: false, draft: false,
        "Replace the open part with an empty one. Lengths are mm and angles radians.",
        [{}]),
    tool!("open_part", "doc.open", FilePath, read_only: false, draft: false,
        "Open an .ofp part file, replacing the open part.",
        [{"path": "part.ofp"}]),
    tool!("save_part", "doc.save", SavePath, read_only: false, draft: false,
        "Save the part as an .ofp file.",
        [{"path": "part.ofp"}]),
    tool!("export", "export", Export, read_only: false, draft: false,
        "Export a configuration (default: active) to STEP, IGES or STL, chosen by extension. Fails while active features have errors. Hidden bodies are included.",
        [{"path": "part.step"}, {"path": "part.stl", "configuration": 2}]),
    tool!("import", "import", FilePath, read_only: false, draft: false,
        "Import a .step or .iges file as a new body feature.",
        [{"path": "part.step"}]),
    tool!("undo", "doc.undo", Empty, read_only: false, draft: false,
        "Undo the last change.",
        [{}]),
    tool!("new_assembly", "asm.new", Empty, read_only: false, draft: false,
        "Replace the open document with an empty assembly. Insert parts with insert_component; the first one is fixed at the origin.",
        [{}]),
    tool!("assembly_info", "asm.info", Empty, read_only: true, draft: false,
        "The open assembly: components (keys, files, fixed, remaining DOF, errors, children), mates (IDs, types, errors), patterns, mirrors, configurations, explode steps and solve diagnostics.",
        [{}]),
    tool!("component_topology", "asm.topology", ComponentPath, read_only: true, draft: false,
        "Faces and edges of one component in assembly coordinates, each with the component `ref` that add_mate and measure take.",
        [{"component": ["2"]}]),
    tool!("insert_component", "asm.insert", InsertComponent, read_only: false, draft: false,
        "Insert an .ofp part or .ofa subassembly. Returns its key as `key`. Transform rotation is a unit quaternion [x, y, z, w].",
        [{"path": "part.ofp", "transform": {"translation": [0, 0, 40], "rotation": [0, 0, 0, 1]}}]),
    tool!("hardware_catalog", "asm.hardware_catalog", Empty, read_only: true, draft: false,
        "Standard hardware kinds with their sizes and preferred lengths (mm).",
        [{}]),
    tool!("insert_hardware", "asm.insert_hardware", InsertHardware, read_only: false, draft: false,
        "Generate (or reuse) a standard screw, bolt, nut, washer or pin from hardware_catalog and insert it. Its axis is Z, its bearing face on Z = 0, the head toward +Z.",
        [{"hardware": {"kind": "hex_nut", "size": "M6"}},
         {"hardware": {"kind": "socket_head_cap_screw", "size": "M5", "length": 16}, "transform": {"translation": [30, 20, 5], "rotation": [0, 0, 0, 1]}}]),
    tool!("update_component", "asm.component.update", UpdateComponent, read_only: false, draft: false,
        "Rename, fix or float, hide, suppress, exclude from BOM, make a subassembly flexible, pick a part configuration or set the transform of a top-level component.",
        [{"key": "2", "name": "Lid"}, {"key": "2", "fixed": true}]),
    tool!("delete_components", "asm.component.delete", ComponentKeys, read_only: false, draft: false,
        "Delete components and every mate that uses them. Returns `deleted_mates`.",
        [{"keys": ["2"]}]),
    tool!("add_mate", "asm.mate.add", AddMate, read_only: false, draft: false,
        "Add a mate between component references from component_topology and solve. Fails without changing anything if the mate can't be satisfied, naming the mates it conflicts with. Lengths mm, angles radians.",
        [{"mate": {"type": "coincident", "a": {"component": ["1"], "entity": {"type": "face", "name": "2.end"}}, "b": {"component": ["2"], "entity": {"type": "face", "name": "2.start"}}}},
         {"mate": {"type": "distance", "a": {"component": [], "entity": {"type": "right_plane"}}, "b": {"component": ["2"], "entity": {"type": "right_plane"}}, "distance": 10}, "name": "Offset"}]),
    tool!("update_mate", "asm.mate.update", UpdateMate, read_only: false, draft: false,
        "Change a mate's definition, name or suppression by ID.",
        [{"id": 3, "name": "Seat"}, {"id": 3, "suppressed": true}]),
    tool!("delete_mates", "asm.mate.delete", ItemIds, read_only: false, draft: false,
        "Delete mates by ID.",
        [{"ids": [3]}]),
    tool!("move_component", "asm.move", MoveComponent, read_only: false, draft: false,
        "Move or rotate a floating component within its mates: by a delta, to a point, along or about a reference. With `collision` it stops before parts overlap.",
        [{"component": ["2"], "motion": {"mode": "delta", "translation": [10, 0, 0]}},
         {"component": ["2"], "motion": {"mode": "rotate", "angles": [0, 0, 0.5]}, "collision": {}}]),
    tool!("add_component_pattern", "asm.pattern.add", AddPattern, read_only: false, draft: false,
        "Pattern components: linear, circular, following a part's pattern feature, at sketch points or along a curve. Instances get keys like `2@p5.1`.",
        [{"pattern": {"type": "linear", "direction1": {"reference": {"component": [], "entity": {"type": "right_plane"}}, "spacing": 70, "count": 3}}, "seeds": ["2"]}]),
    tool!("mirror_components", "asm.mirror.add", AddMirror, read_only: false, draft: false,
        "Mirror components about a plane: reoriented copies, or opposite-hand part files written next to the assembly.",
        [{"plane": {"component": [], "entity": {"type": "right_plane"}}, "components": [{"component": "2"}]}]),
    tool!("add_explode_step", "asm.explode.add", AddExplodeStep, read_only: false, draft: false,
        "Add an explode step to the active configuration and show the exploded view. Explode steps move only the display, never mates.",
        [{"components": [["2"]], "motion": {"type": "translate", "direction": [0, 0, 1], "distance": 30}}]),
    tool!("interference", "asm.interference", Interference, read_only: true, draft: false,
        "Find components whose solids overlap, with each overlap's volume (mm³).",
        [{}, {"treat_coincidence": true}]),
    tool!("clearance", "asm.clearance", Clearance, read_only: true, draft: false,
        "List gaps between components smaller than `minimum` mm, smallest first.",
        [{"minimum": 50}]),
    tool!("bill_of_materials", "asm.bom", Bom, read_only: true, draft: false,
        "The bill of materials: top-level, parts-only or indented rows with quantities and part properties. `csv` also writes it to a file.",
        [{"type": "parts_only"}]),
    tool!("import_step_assembly", "asm.import_step", ImportStep, read_only: false, draft: false,
        "Read a STEP file with several parts into a new assembly: one .ofp per distinct part in `folder`, each instance placed and fixed where the file had it.",
        [{"path": "assembly.step"}]),
];

/// Inline `{"$ref", "properties"}` pairs (serde's internally tagged newtype
/// variants), close every object and drop annotations models do not need.
fn normalize(value: &mut Value, defs: &Map<String, Value>) {
    match value {
        Value::Object(o) => {
            if o.contains_key("properties")
                && let Some(Value::String(r)) = o.get("$ref").cloned()
            {
                o.remove("$ref");
                let def = &defs[r.trim_start_matches("#/$defs/")];
                for (key, extra) in def.as_object().unwrap() {
                    match (key.as_str(), o.get_mut(key)) {
                        ("properties", Some(Value::Object(props))) => {
                            props.extend(extra.as_object().unwrap().clone());
                        }
                        ("required", Some(Value::Array(req))) => {
                            req.extend(extra.as_array().unwrap().iter().cloned());
                        }
                        (_, None) => {
                            o.insert(key.clone(), extra.clone());
                        }
                        _ => {}
                    }
                }
            }
            // A tagged variant wrapping another tagged union (`type: plane`
            // with `mode: offset`): every inner branch gets the outer fields.
            let required = o_required(o);
            if let (Some(Value::Object(outer)), Some(Value::Array(branches))) =
                (o.get("properties").cloned(), o.get_mut("oneOf"))
            {
                for branch in branches.iter_mut() {
                    if let Value::Object(b) = branch {
                        let props = b.entry("properties").or_insert_with(|| json!({}));
                        props.as_object_mut().unwrap().extend(outer.clone());
                        let req = b.entry("required").or_insert_with(|| json!([]));
                        let req = req.as_array_mut().unwrap();
                        for r in required.iter().rev() {
                            req.insert(0, r.clone());
                        }
                    }
                }
                for k in ["properties", "required", "additionalProperties", "type"] {
                    o.remove(k);
                }
            }
            o.remove("format");
            o.remove("$schema");
            o.remove("title");
            // Unsigned IDs: non-negativity is implied and costs tokens everywhere.
            if o.get("type") == Some(&json!("integer")) && o.get("minimum") == Some(&json!(0)) {
                o.remove("minimum");
            }
            // A tag's `const` already fixes its type.
            if o.contains_key("const") {
                o.remove("type");
            }
            let open = [
                "oneOf",
                "anyOf",
                "allOf",
                "$ref",
                "additionalProperties",
                "patternProperties",
            ];
            if o.get("type") == Some(&json!("object")) && !open.iter().any(|k| o.contains_key(*k)) {
                o.entry("properties").or_insert_with(|| json!({}));
            }
            if o.contains_key("properties") && !o.contains_key("additionalProperties") {
                o.insert("additionalProperties".into(), false.into());
            }
            for v in o.values_mut() {
                normalize(v, defs);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|v| normalize(v, defs)),
        _ => {}
    }
}

fn o_required(o: &Map<String, Value>) -> Vec<Value> {
    o.get("required")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// Drop `$defs` no longer referenced after inlining.
fn prune(schema: &mut Value) {
    let Some(Value::Object(defs)) = schema.get("$defs").cloned() else {
        return;
    };
    let mut used = std::collections::BTreeSet::new();
    let mut stack: Vec<String> = Vec::new();
    let scan = |v: &Value, stack: &mut Vec<String>| {
        let text = v.to_string();
        for part in text.split("\"#/$defs/").skip(1) {
            stack.push(part.split('"').next().unwrap().to_owned());
        }
    };
    let mut root = schema.clone();
    root.as_object_mut().unwrap().remove("$defs");
    scan(&root, &mut stack);
    while let Some(name) = stack.pop() {
        if used.insert(name.clone()) {
            scan(&defs[&name], &mut stack);
        }
    }
    let kept: Map<String, Value> = defs.into_iter().filter(|(k, _)| used.contains(k)).collect();
    let o = schema.as_object_mut().unwrap();
    if kept.is_empty() {
        o.remove("$defs");
    } else {
        o.insert("$defs".into(), Value::Object(kept));
    }
}

fn input_schema(tool: &Tool) -> Value {
    let mut generator = SchemaGenerator::new(SchemaSettings::draft2020_12());
    let mut schema = (tool.schema)(&mut generator).to_value();
    // A tool's arguments are a top-level object: resolve the root reference.
    let mut defs: Map<String, Value> = generator.take_definitions(true).into_iter().collect();
    if let Some(Value::String(r)) = schema.get("$ref").cloned() {
        schema = defs[r.trim_start_matches("#/$defs/")].clone();
    }
    // Inline inside definitions first, so later merges copy finished objects.
    let snapshot = defs.clone();
    for def in defs.values_mut() {
        normalize(def, &snapshot);
    }
    normalize(&mut schema, &defs);
    let o = schema.as_object_mut().unwrap();
    // Every tool accepts `detail`; the envelope strips it before the core call.
    o.entry("properties")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .unwrap()
        .insert(
            "detail".into(),
            json!({
                "description": "`full` returns the untrimmed result; the default `summary` omits bulky snapshot fields.",
                "enum": ["summary", "full"],
                "default": "summary",
            }),
        );
    o.insert("examples".into(), Value::Array((tool.examples)()));
    if !defs.is_empty() {
        o.insert("$defs".into(), Value::Object(defs));
    }
    prune(&mut schema);
    schema
}

/// The catalog with generated schemas, built once.
pub fn catalog() -> &'static [Entry] {
    static CATALOG: OnceLock<Vec<Entry>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        TOOLS
            .iter()
            .map(|tool| {
                let schema = input_schema(tool);
                let validator = jsonschema::draft202012::new(&schema)
                    .unwrap_or_else(|e| panic!("{} schema: {e}", tool.name));
                Entry {
                    tool,
                    schema,
                    validator,
                }
            })
            .collect()
    })
}

pub fn by_name(name: &str) -> Option<&'static Entry> {
    catalog().iter().find(|e| e.tool.name == name)
}

pub fn by_method(method: &str) -> Option<&'static Entry> {
    catalog().iter().find(|e| e.tool.method == method)
}

impl Entry {
    /// Check arguments against the schema. Errors name the JSON path, so a
    /// misspelled field fails instead of being ignored.
    pub fn validate(&self, args: &Value) -> Result<(), RpcError> {
        if self.validator.is_valid(args) {
            return Ok(());
        }
        let defs = self.schema.get("$defs").cloned().unwrap_or(json!({}));
        let mut errors = Vec::new();
        explain(&self.schema, &defs, args, "", &mut errors);
        errors.truncate(3);
        let Some(first) = errors.first() else {
            return Ok(());
        };
        let text: Vec<String> = errors
            .iter()
            .map(|p| format!("{}: {}", p.path, p.message))
            .collect();
        let mut e = RpcError::coded(
            ErrorCode::InvalidArgs,
            format!("invalid {} arguments: {}", self.tool.name, text.join("; ")),
        );
        if let Some(data) = &mut e.data {
            data.path = Some(first.path.clone());
            data.allowed = first.allowed.clone();
        }
        Err(e)
    }

    #[cfg(test)]
    pub fn examples(&self) -> Vec<Value> {
        (self.tool.examples)()
    }
}

/// The tag field and branch of a tagged union that an instance selects.
fn branch<'a>(
    branches: &'a [Value],
    instance: &Value,
) -> Option<(String, Result<&'a Value, Vec<String>>)> {
    let tag = branches.iter().find_map(|b| {
        b["properties"]
            .as_object()?
            .iter()
            .find(|(_, p)| p.get("const").is_some())
            .map(|(k, _)| k.clone())
    })?;
    // A branch may itself be a union whose branches share the outer tag.
    fn consts<'v>(b: &'v Value, tag: &str) -> Vec<&'v Value> {
        match b.get("oneOf").and_then(Value::as_array) {
            Some(inner) => inner.iter().flat_map(|i| consts(i, tag)).collect(),
            None => b["properties"][tag].get("const").into_iter().collect(),
        }
    }
    let found = branches
        .iter()
        .find(|b| consts(b, &tag).contains(&&instance[&tag]));
    let mut names: Vec<String> = branches
        .iter()
        .flat_map(|b| consts(b, &tag))
        .filter_map(|c| c.as_str().map(str::to_owned))
        .collect();
    names.dedup();
    Some((tag, found.ok_or(names)))
}

/// Validation messages with the JSON path of each problem. Tagged unions are
/// explained through the branch the instance's tag selects, so a typo inside
/// a feature names the field instead of "no oneOf branch matched".
/// One argument problem: where it is, what is wrong, and what is allowed there.
pub struct Problem {
    pub path: String,
    pub message: String,
    pub allowed: Option<Vec<String>>,
}

fn explain(node: &Value, defs: &Value, instance: &Value, path: &str, out: &mut Vec<Problem>) {
    let resolve = |n: &Value| match n.get("$ref").and_then(Value::as_str) {
        Some(r) => defs[r.trim_start_matches("#/$defs/")].clone(),
        None => n.clone(),
    };
    let node = resolve(node);
    let at = if path.is_empty() { "/" } else { path };
    if let Some(Value::Array(branches)) = node.get("oneOf")
        && instance.is_object()
        && let Some((tag, chosen)) = branch(branches, instance)
    {
        match chosen {
            Ok(b) => explain(b, defs, instance, path, out),
            Err(consts) => out.push(Problem {
                path: format!("{path}/{tag}"),
                message: format!(
                    "{} is not one of: {}",
                    instance
                        .get(&tag)
                        .map_or("missing".into(), Value::to_string),
                    consts.join(", ")
                ),
                allowed: Some(consts),
            }),
        }
        return;
    }
    // Check this level, then descend into fields so nested unions are explained.
    let mut local = node.clone();
    if let Value::Object(o) = &mut local {
        o.insert("$defs".into(), defs.clone());
        for key in ["properties", "items"] {
            if let Some(v) = o.get_mut(key) {
                *v = if key == "items" { json!({}) } else { strip(v) };
            }
        }
    }
    if let Ok(v) = jsonschema::draft202012::new(&local) {
        for e in v.iter_errors(instance) {
            let allowed = match e.kind() {
                jsonschema::error::ValidationErrorKind::Enum { options } => {
                    options.as_array().map(|o| {
                        o.iter()
                            .map(|v| v.as_str().map_or(v.to_string(), str::to_owned))
                            .collect()
                    })
                }
                _ => None,
            };
            let inner = e.instance_path().to_string();
            out.push(Problem {
                path: if inner.is_empty() {
                    at.to_owned()
                } else {
                    format!("{path}{inner}")
                },
                message: e.to_string(),
                allowed,
            });
        }
    }
    match instance {
        Value::Object(fields) => {
            for (key, value) in fields {
                if let Some(child) = node.get("properties").and_then(|p| p.get(key)) {
                    explain(child, defs, value, &format!("{path}/{key}"), out);
                }
            }
        }
        Value::Array(items) => {
            if let Some(child) = node.get("items") {
                for (i, value) in items.iter().enumerate() {
                    explain(child, defs, value, &format!("{path}/{i}"), out);
                }
            }
        }
        _ => {}
    }
}

/// Field schemas reduced to "anything", so a level is checked on its own.
fn strip(properties: &Value) -> Value {
    match properties.as_object() {
        Some(p) => Value::Object(p.keys().map(|k| (k.clone(), json!({}))).collect()),
        None => json!({}),
    }
}

/// The result envelope's schema, shared by every tool.
pub fn output_schema() -> &'static Value {
    static SCHEMA: OnceLock<Value> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        let mut generator = SchemaGenerator::new(SchemaSettings::draft2020_12());
        let mut schema = generator.subschema_for::<envelope::Envelope>().to_value();
        let mut defs: Map<String, Value> = generator.take_definitions(true).into_iter().collect();
        if let Some(Value::String(r)) = schema.get("$ref").cloned() {
            schema = defs[r.trim_start_matches("#/$defs/")].clone();
        }
        let snapshot = defs.clone();
        for def in defs.values_mut() {
            normalize(def, &snapshot);
        }
        normalize(&mut schema, &defs);
        schema
            .as_object_mut()
            .unwrap()
            .insert("$defs".into(), Value::Object(defs));
        prune(&mut schema);
        schema
    })
}

/// Tool list for MCP `tools/list`.
pub fn mcp_tools() -> Value {
    json!({ "tools": catalog().iter().map(|e| json!({
        "name": e.tool.name,
        "description": e.tool.description,
        "inputSchema": e.schema,
        "outputSchema": output_schema(),
    })).collect::<Vec<_>>() })
}

/// The draft tools for the built-in assistant (`assistant.tools`).
pub fn assistant_catalog() -> Value {
    json!({ "tools": catalog().iter().filter(|e| e.tool.draft).map(|e| json!({
        "name": e.tool.name,
        "method": e.tool.method,
        "description": e.tool.description,
        "parameters": e.schema,
        "read_only": e.tool.read_only,
    })).collect::<Vec<_>>() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;

    /// The part every example runs on, each in a fresh copy:
    /// 1 Sketch1 (60 × 40 rectangle), 2 Boss-Extrude1 (5 mm, body
    /// [`EXAMPLE_BODY`]), 3 Sketch3 (circle 2 with center 1 at (30, 20),
    /// standalone point 3, diameter dimension 4), 4 Sketch4 (10 × 10 rectangle
    /// at x = 70, left line 8), and configurations 1 Default and 2 Long.
    pub(crate) fn example_part() -> Session {
        let mut s = Session::new();
        let mut ok = |method: &str, params: Value| {
            s.handle(method, params)
                .unwrap_or_else(|e| panic!("{method}: {}", e.message))
        };
        let edit = |id: u32, op: Value| json!({ "id": id, "op": op });
        ok("sketch.new", json!({ "plane": { "type": "front_plane" } }));
        let rect = json!({ "op": "rectangle", "a": [0, 0], "b": [60, 40] });
        ok("sketch.edit", edit(1, rect));
        for op in [
            json!({ "op": "constraint", "constraint": { "type": "coincident", "a": 1, "b": 0 } }),
            json!({ "op": "dimension", "constraint": { "type": "length", "line": 5, "value": 0 } }),
            json!({ "op": "dimension", "constraint": { "type": "length", "line": 6, "value": 0 } }),
        ] {
            ok("sketch.edit", edit(1, op));
        }
        ok("sketch.close", json!({}));
        let boss =
            json!({ "type": "extrude", "sketch": 1, "end1": { "type": "blind", "depth": 5 } });
        ok("feature.add", json!({ "feature": boss }));
        ok("sketch.new", json!({ "plane": { "type": "front_plane" } }));
        for op in [
            json!({ "op": "add_circle", "center": { "at": [30, 20] }, "radius": 5 }),
            json!({ "op": "add_point", "at": [10, 10] }),
            json!({ "op": "dimension", "constraint": { "type": "diameter", "curve": 2, "value": 0 } }),
        ] {
            ok("sketch.edit", edit(3, op));
        }
        ok("sketch.close", json!({}));
        ok("sketch.new", json!({ "plane": { "type": "front_plane" } }));
        ok(
            "sketch.edit",
            edit(4, json!({ "op": "rectangle", "a": [70, 0], "b": [80, 10] })),
        );
        ok("sketch.close", json!({}));
        ok("configuration.create", json!({ "name": "Long" }));
        let long = json!({ "id": 2, "name": "Long", "parameters": { "f2.end1.depth": 10 } });
        ok("configuration.update", json!({ "configuration": long }));
        ok("configuration.activate", json!({ "id": 1 }));
        s
    }

    /// The assembly assembly examples run on: the example part saved as
    /// `part.ofp`, inserted fixed (key 1) and again 20 mm above (key 2), with
    /// mate 3 seating the second's bottom face on the first's top face.
    pub(crate) fn example_assembly(dir: &std::path::Path) -> Session {
        let mut s = example_part();
        let part = dir.join("part.ofp");
        s.handle("doc.save", json!({ "path": part })).unwrap();
        s.handle("export", json!({ "path": dir.join("assembly.step") }))
            .unwrap();
        let mut ok = |method: &str, params: Value| {
            s.handle(method, params)
                .unwrap_or_else(|e| panic!("{method}: {}", e.message))
        };
        ok("asm.new", json!({}));
        ok("asm.insert", json!({ "path": part }));
        let up = json!({ "translation": [0, 0, 20], "rotation": [0, 0, 0, 1] });
        ok("asm.insert", json!({ "path": part, "transform": up }));
        let face =
            |c: &str, n: &str| json!({ "component": [c], "entity": { "type": "face", "name": n } });
        let seat =
            json!({ "type": "coincident", "a": face("1", "2.end"), "b": face("2", "2.start") });
        ok("asm.mate.add", json!({ "mate": seat }));
        s
    }

    #[test]
    fn example_assembly_matches_its_description() {
        let dir =
            std::env::temp_dir().join(format!("openforge-asm-example-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = example_assembly(&dir);
        let info = s.handle("asm.info", json!({})).unwrap();
        assert_eq!(info["mates"][0]["id"], 3);
        assert_eq!(info["components"][1]["key"], json!(["2"]));
        assert!(info["diagnostics"]["solved"].as_bool().unwrap());
        std::fs::remove_dir_all(&dir).ok();
    }

    fn errors(s: &mut Session) -> Vec<(u64, String)> {
        let info = s.handle("doc.info", json!({})).unwrap();
        info["features"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| !f["error"].is_null())
            .map(|f| (f["id"].as_u64().unwrap(), f["error"].to_string()))
            .collect()
    }

    #[test]
    fn example_part_matches_its_description() {
        let mut s = example_part();
        assert!(errors(&mut s).is_empty());
        let info = s.handle("doc.info", json!({})).unwrap();
        assert_eq!(info["bodies"][0]["id"], EXAMPLE_BODY, "{}", info["bodies"]);
        let sketch = s.handle("sketch.get", json!({ "id": 4 })).unwrap();
        let left = sketch["curves"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == 8)
            .unwrap();
        assert_eq!(left["samples"][0][0], 70.0);
    }

    #[test]
    fn every_example_validates_and_replays_on_the_core() {
        let dir = std::env::temp_dir().join(format!("openforge-examples-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut replayed = 0;
        for entry in catalog() {
            let examples = entry.examples();
            assert!(!examples.is_empty(), "{} has no example", entry.tool.name);
            for mut example in examples {
                entry
                    .validate(&example)
                    .unwrap_or_else(|e| panic!("{}: {example}: {}", entry.tool.name, e.message));
                // File examples name a file; replay them in a scratch directory.
                if let Some(path) = example.get_mut("path") {
                    *path = json!(dir.join(path.as_str().unwrap()));
                }
                if let Some(folder) = example.get_mut("folder") {
                    *folder = json!(dir.join("hardware"));
                }
                let method = entry.tool.method;
                let mut s = if method.starts_with("asm.") && method != "asm.new" {
                    example_assembly(&dir)
                } else {
                    example_part()
                };
                let volume = |s: &mut Session| {
                    s.handle("mass_properties", json!({})).unwrap()["volume"]
                        .as_f64()
                        .unwrap()
                };
                let before = volume(&mut s);
                // Reading a file needs one written first.
                match entry.tool.method {
                    "import" => drop(s.handle("export", example.clone()).unwrap()),
                    "doc.open" => drop(s.handle("doc.save", example.clone()).unwrap()),
                    // The assembly fixture already wrote part.ofp and assembly.step.
                    _ => {}
                }
                let result = s.handle(entry.tool.method, example.clone());
                let result = result
                    .unwrap_or_else(|e| panic!("{}: {example}: {}", entry.tool.name, e.message));
                let errors = errors(&mut s);
                assert!(
                    errors.is_empty(),
                    "{}: {example}: {errors:?}",
                    entry.tool.name
                );
                assert!(result.is_object(), "{}", entry.tool.name);
                // Material-removing examples must remove material (a cut in the
                // wrong direction succeeds silently; see docs/mcp-baseline.md).
                let feature = &example["feature"];
                if feature["cut"] == true || feature["type"] == "hole" || feature["type"] == "shell"
                {
                    assert!(
                        volume(&mut s) < before - 1.0,
                        "{}: {example} removed nothing",
                        entry.tool.name
                    );
                }
                replayed += 1;
            }
        }
        std::fs::remove_dir_all(&dir).ok();
        assert!(replayed > 50, "{replayed}");
    }

    fn walk<'a>(value: &'a Value, path: String, out: &mut Vec<(String, &'a Value)>) {
        match value {
            Value::Object(o) => {
                out.push((path.clone(), value));
                for (k, v) in o {
                    if k != "examples" && k != "default" {
                        walk(v, format!("{path}/{k}"), out);
                    }
                }
            }
            Value::Array(a) => {
                for (i, v) in a.iter().enumerate() {
                    walk(v, format!("{path}/{i}"), out);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn schemas_are_strict_and_descriptions_short() {
        for entry in catalog() {
            let name = entry.tool.name;
            assert!(
                entry.tool.description.len() <= 600,
                "{name} description is too long"
            );
            assert_eq!(entry.schema["type"], "object", "{name}");
            let mut nodes = Vec::new();
            walk(&entry.schema, String::new(), &mut nodes);
            for (path, node) in nodes {
                let o = node.as_object().unwrap();
                let object =
                    o.get("type") == Some(&json!("object")) || o.contains_key("properties");
                if object && !o.contains_key("patternProperties") {
                    assert!(
                        o.contains_key("additionalProperties"),
                        "{name}{path} is an open object"
                    );
                }
                if o.contains_key("$ref") {
                    assert!(
                        !o.contains_key("properties"),
                        "{name}{path} mixes $ref and fields"
                    );
                }
            }
        }
    }

    #[test]
    fn core_failures_carry_stable_codes() {
        let cases = [
            (
                "doc.feature",
                json!({ "id": 999 }),
                ErrorCode::UnknownFeature,
            ),
            ("sketch.get", json!({ "id": 2 }), ErrorCode::NotASketch),
            (
                "sketch.edit",
                json!({ "id": 3, "op": { "op": "delete", "ids": [3] } }),
                ErrorCode::Failed,
            ),
            (
                "sketch.edit",
                json!({ "id": 3, "op": { "op": "constraint", "constraint": { "type": "horizontal", "line": 2 } } }),
                ErrorCode::UnknownEntity,
            ),
            (
                "parameter.set",
                json!({ "key": "f9.depth", "value": 1 }),
                ErrorCode::UnknownParameter,
            ),
            (
                "configuration.activate",
                json!({ "id": 9 }),
                ErrorCode::UnknownConfiguration,
            ),
            (
                "body.display",
                json!({ "id": "b9:0", "hidden": true }),
                ErrorCode::UnknownBody,
            ),
            (
                "sketch.new",
                json!({ "plane": { "type": "face", "name": "9.nope" } }),
                ErrorCode::UnknownRef,
            ),
            (
                "measure",
                json!({ "refs": [{ "type": "edge", "name": "E(9|9)" }] }),
                ErrorCode::UnknownRef,
            ),
            (
                "equations.set",
                json!({ "variables": { "pi": "2mm" }, "equations": {} }),
                ErrorCode::InvalidFormula,
            ),
        ];
        for (method, params, code) in cases {
            let mut s = example_part();
            match s.handle(method, params.clone()) {
                // Deleting an existing point succeeds; it marks the Ok case.
                Ok(_) => assert_eq!(code, ErrorCode::Failed, "{method} {params}"),
                Err(e) => assert_eq!(e.error_code(), code, "{method} {params}: {}", e.message),
            }
        }
    }

    #[test]
    fn misspelled_and_out_of_range_fields_are_rejected_with_their_path() {
        let add = by_name("add_feature").unwrap();
        let fillet = json!({ "feature": {
            "type": "fillet", "edges": [{ "type": "edge", "name": "E(2.end|2.s5)" }],
            "radius": 3, "radios": 9 } });
        let e = add.validate(&fillet).unwrap_err();
        assert_eq!(e.code, RpcError::INVALID_PARAMS);
        assert!(
            e.message.contains("/feature") && e.message.contains("radios"),
            "{}",
            e.message
        );
        let negative = json!({ "feature": {
            "type": "extrude", "sketch": 1, "end1": { "type": "blind", "depth": -2 } } });
        let e = add.validate(&negative).unwrap_err();
        assert!(e.message.contains("/feature"), "{}", e.message);
        let unknown = json!({ "feature": { "type": "sheet_metal", "thickness": 1 } });
        let e = add.validate(&unknown).unwrap_err();
        let data = e.data.clone().unwrap();
        assert_eq!(
            (data.code, data.path.as_deref()),
            (ErrorCode::InvalidArgs, Some("/feature/type"))
        );
        assert!(data.allowed.unwrap().iter().any(|t| t == "extrude"));
        assert!(
            e.message.contains("/feature/type") && e.message.contains("extrude"),
            "{}",
            e.message
        );
        for e in [fillet, negative] {
            println!("{}", add.validate(&e).unwrap_err().message);
        }
        println!("{}", add.validate(&unknown).unwrap_err().message);
        let plane = json!({ "feature": { "type": "plane", "mode": "offset",
            "reference": { "type": "front_plane" }, "distanse": 4 } });
        let e = add.validate(&plane).unwrap_err();
        assert!(
            e.message.contains("distanse") && !e.message.contains("not one of"),
            "{}",
            e.message
        );
        let sketch = json!({ "id": 3, "op": { "op": "drag", "point": 1, "to": [0, 0] } });
        assert!(by_name("edit_sketch").unwrap().validate(&sketch).is_err());
    }
}

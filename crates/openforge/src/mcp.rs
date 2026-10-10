//! A Model Context Protocol server over stdio, so AI assistants can build
//! and edit parts. Each tool forwards to a [`Session`] method.

use std::io::{BufRead, Write};

use of_protocol::{Response, RpcError, parse_request};
use serde_json::{Value, json};

use crate::session::Session;

const PROTOCOL_VERSION: &str = "2025-06-18";

struct Tool {
    name: &'static str,
    method: &'static str,
    description: &'static str,
    schema: fn() -> Value,
}

fn obj(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required })
}

const TOOLS: &[Tool] = &[
    Tool {
        name: "get_feature",
        method: "doc.feature",
        description: "Read the complete authored feature definition, including sketch geometry and constraints.",
        schema: || obj(json!({"id":{"type":"integer","minimum":1}}), &["id"]),
    },
    Tool {
        name: "set_parameter",
        method: "parameter.set",
        description: "Set a registered base parameter from part_info.parameters. Core units: mm/radians. Removes its base equation; configuration overrides can still take precedence. To change only a variant use update_configuration.",
        schema: || {
            obj(
                json!({"key":{"type":"string"},"value":{"type":"number"}}),
                &["key", "value"],
            )
        },
    },
    Tool {
        name: "close_sketch",
        method: "sketch.close",
        description: "Finish the draft sketch before adding solid features. Does not commit the AI task.",
        schema: || obj(json!({}), &[]),
    },
    Tool {
        name: "set_equations",
        method: "equations.set",
        description: "Replace base global variables and equation bindings. Variables map names to formulas. Equations map stable keys from part_info.parameters (e.g. f2.end1.depth) to formulas such as width/2 + 1mm. Constants require explicit units for lengths/angles. Check equation_values and feature errors after the change.",
        schema: || {
            obj(
                json!({ "variables": {"type":"object","additionalProperties":{"type":"string"}}, "equations": {"type":"object","additionalProperties":{"type":"string"}} }),
                &["variables", "equations"],
            )
        },
    },
    Tool {
        name: "create_configuration",
        method: "configuration.create",
        description: "Create and activate a configuration. Optionally duplicate copy_from configuration ID; otherwise inherit base values.",
        schema: || {
            obj(
                json!({"name":{"type":"string"},"copy_from":{"type":"integer"}}),
                &["name"],
            )
        },
    },
    Tool {
        name: "update_configuration",
        method: "configuration.update",
        description: "Replace a configuration from part_info.configurations.items, retaining its ID. Supports name, parameters (numeric core units), equations, variables, suppressed feature IDs and material overrides. Feature creation/deletion/reorder edits the shared tree.",
        schema: || {
            obj(
                json!({"configuration":{"type":"object"}}),
                &["configuration"],
            )
        },
    },
    Tool {
        name: "activate_configuration",
        method: "configuration.activate",
        description: "Activate a configuration ID, discard editing state and rebuild the part.",
        schema: || obj(json!({"id":{"type":"integer"}}), &["id"]),
    },
    Tool {
        name: "delete_configuration",
        method: "configuration.delete",
        description: "Delete a configuration by ID. At least one configuration must remain. Undo is available.",
        schema: || obj(json!({"id":{"type":"integer"}}), &["id"]),
    },
    Tool {
        name: "body_display",
        method: "body.display",
        description: "Rename or hide/show a body by stable ID from part_info.bodies. Display visibility does not change export or feature scope.",
        schema: || {
            obj(
                json!({"id":{"type":"string"},"name":{"type":"string"},"hidden":{"type":"boolean"}}),
                &["id"],
            )
        },
    },
    Tool {
        name: "core_info",
        method: "core.info",
        description: "Protocol, scene and part-file versions and implemented core capabilities.",
        schema: || obj(json!({}), &[]),
    },
    Tool {
        name: "part_info",
        method: "doc.info",
        description: "The feature tree, units, material and any feature errors of the open part.",
        schema: || obj(json!({}), &[]),
    },
    Tool {
        name: "topology",
        method: "doc.topology",
        description: "Faces and edges of the current body with their stable references, surface/curve type, size, centroid and normal. Use the `ref` objects in features that need faces or edges (fillet, chamfer, shell, sketch on face, up-to-surface).",
        schema: || obj(json!({}), &[]),
    },
    Tool {
        name: "new_part",
        method: "doc.new",
        description: "Start a new empty part. Lengths are millimetres, angles radians.",
        schema: || obj(json!({}), &[]),
    },
    Tool {
        name: "open_part",
        method: "doc.open",
        description: "Open an .ofp part file.",
        schema: || obj(json!({ "path": { "type": "string" } }), &["path"]),
    },
    Tool {
        name: "save_part",
        method: "doc.save",
        description: "Save the part as an .ofp file.",
        schema: || obj(json!({ "path": { "type": "string" } }), &[]),
    },
    Tool {
        name: "new_sketch",
        method: "sketch.new",
        description: r#"Create an empty sketch. `plane` is {"type":"front_plane"} (XY), {"type":"top_plane"} (XZ), {"type":"right_plane"} (YZ), a reference plane {"type":"feature","id":N}, or a flat face ref from `topology`. Returns the sketch id."#,
        schema: || obj(json!({ "plane": { "type": "object" } }), &["plane"]),
    },
    Tool {
        name: "edit_sketch",
        method: "sketch.edit",
        description: r#"Apply one edit to sketch `id`. `op` examples: {"op":"rectangle","a":[0,0],"b":[40,20]}, {"op":"add_circle","center":{"at":[10,10]},"radius":3}, {"op":"add_line","a":{"at":[0,0]},"b":{"id":5}}, {"op":"slot","a":[0,0],"b":[20,0],"width":6}, {"op":"polygon","center":[0,0],"vertex":[10,0],"sides":6}, {"op":"add_point","at":[5,5]}, {"op":"constraint","constraint":{"type":"horizontal","line":3}}, {"op":"dimension","constraint":{"type":"length","line":3,"value":0},"value":25}, {"op":"set_value","id":9,"value":30}, {"op":"delete","ids":[4]}. Coordinates are in the sketch plane, mm. Returns the solved sketch with entity ids and its definition status."#,
        schema: || {
            obj(
                json!({ "id": { "type": "integer" }, "op": { "type": "object" } }),
                &["id", "op"],
            )
        },
    },
    Tool {
        name: "get_sketch",
        method: "sketch.get",
        description: "The entities, relations and status of sketch `id`.",
        schema: || obj(json!({ "id": { "type": "integer" } }), &["id"]),
    },
    Tool {
        name: "add_feature",
        method: "feature.add",
        description: r#"Add a feature. Examples (mm, radians): extrude {"type":"extrude","sketch":1,"end1":{"type":"blind","depth":10}}; cut through all {"type":"extrude","sketch":3,"cut":true,"end1":{"type":"through_all"},"reverse":true}; revolve {"type":"revolve","sketch":1,"axis":{"type":"sketch_entity","sketch":1,"entity":7},"angle":{"type":"blind","angle":6.283}}; fillet {"type":"fillet","edges":[edge refs],"radius":2}; chamfer {"type":"chamfer","edges":[...],"mode":{"type":"equal","distance":1}}; shell {"type":"shell","faces":[face refs],"thickness":2}; hole {"type":"hole","sketch":5,"kind":"counterbore","standard":"iso","size":"M6","end":{"type":"through_all"}} (holes go at the sketch's standalone points); plane {"type":"plane","mode":"offset","reference":{"type":"front_plane"},"distance":20}; linear pattern {"type":"linear_pattern","features":[4],"direction1":{"reference":{"type":"right_plane"},"spacing":10,"count":3}}; circular pattern {"type":"circular_pattern","features":[4],"axis":{...},"count":6,"angle":6.283}; mirror {"type":"mirror","plane":{"type":"right_plane"}}. Extrude end conditions: blind, through_all, up_to_next, up_to_vertex, up_to_surface, offset_from_surface, mid_plane. Revolve angle types: blind, mid_plane, two_direction, up_to_vertex {target}, up_to_surface {target}, offset_from_surface {target, offset, reverse_offset}; offset is a distance in mm.
Phase 2 examples: combine {"type":"combine","bodies":["<id1>","<id2>"],"operation":"cut"}; move/copy {"type":"move_body","bodies":["<id>"],"translation":[10,0,0],"rotation":[0,0,0],"origin":[0,0,0],"copy":true}; split {"type":"split_body","bodies":["<id>"],"plane":{"type":"front_plane"},"keep":"both"}; helix {"type":"helix","plane":{"type":"front_plane"},"radius":5,"pitch":2,"turns":3}; composite_curve {"type":"composite_curve","segments":[{"reference":{"type":"feature","id":7},"reverse":false}]}; projected_curve or split_line {"type":"projected_curve","source":{"type":"feature","id":3},"target":<face ref>,"direction":[0,0,1]}; sweep {"type":"sweep","profile":1,"path":{"type":"feature","id":3},"orientation":"minimum_twist","cut":false,"merge":true}; loft {"type":"loft","profiles":[1,3],"reversed":[],"ruled":false,"cut":false,"merge":true}; draft {"type":"draft","faces":[<face ref>],"neutral_plane":{"type":"front_plane"},"angle":0.05}; variable_fillet {"type":"variable_fillet","edge":<edge ref>,"stations":[{"position":0,"radius":1},{"position":1,"radius":2}]}.
Body IDs come from part_info.bodies. Optional scope:[body IDs] limits supported solid features to selected bodies. Sweep/loft require single closed sketch profiles without holes. A sweep path is one connected sketch, construction curve or edge. Split supports a plane and at most one piece per side of each body. See core_info for capabilities. Check the returned feature errors after each operation."#,
        schema: || {
            obj(
                json!({ "feature": { "type": "object" }, "name": { "type": "string" } }),
                &["feature"],
            )
        },
    },
    Tool {
        name: "update_feature",
        method: "feature.update",
        description: "Replace the parameters of feature `id` (same shape as add_feature) and rebuild.",
        schema: || {
            obj(
                json!({ "id": { "type": "integer" }, "feature": { "type": "object" } }),
                &["id", "feature"],
            )
        },
    },
    Tool {
        name: "delete_features",
        method: "feature.delete",
        description: "Delete features and everything that depends on them.",
        schema: || {
            obj(
                json!({ "ids": { "type": "array", "items": { "type": "integer" } } }),
                &["ids"],
            )
        },
    },
    Tool {
        name: "measure",
        method: "measure",
        description: "Measure one or more face/edge/vertex refs: lengths, areas, radii, distances and angles (mm, radians).",
        schema: || {
            obj(
                json!({ "refs": { "type": "array", "items": { "type": "object" } } }),
                &["refs"],
            )
        },
    },
    Tool {
        name: "mass_properties",
        method: "mass_properties",
        description: "Volume (mm³), surface area, mass (kg) using the part's material, center of mass and inertia.",
        schema: || obj(json!({}), &[]),
    },
    Tool {
        name: "set_material",
        method: "doc.set_material",
        description: "Set the material by name (e.g. \"Aluminum 6061\", \"PLA\", \"Plain Carbon Steel\") or with a custom density in kg/m³.",
        schema: || {
            obj(
                json!({ "name": { "type": "string" }, "density": { "type": "number" } }),
                &[],
            )
        },
    },
    Tool {
        name: "export",
        method: "export",
        description: "Export the active configuration to a .step, .iges or .stl file, or supply a configuration ID. Hidden bodies remain included.",
        schema: || {
            obj(
                json!({ "path": { "type": "string" }, "configuration": { "type": "integer" } }),
                &["path"],
            )
        },
    },
    Tool {
        name: "import",
        method: "import",
        description: "Import a .step or .iges file as a body.",
        schema: || obj(json!({ "path": { "type": "string" } }), &["path"]),
    },
    Tool {
        name: "undo",
        method: "doc.undo",
        description: "Undo the last change.",
        schema: || obj(json!({}), &[]),
    },
];

/// Strip the bulky scene and sketch payloads from results shown to the model.
fn compact(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.remove("scene");
    }
    v
}

fn handle(session: &mut Session, method: &str, params: Value) -> Result<Value, RpcError> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "openforge", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "OpenForge is a parametric CAD kernel. Build parts by creating sketches (new_sketch, edit_sketch) and features (add_feature). Use topology to find face and edge refs. Check part_info for feature errors after each change.",
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({
            "tools": TOOLS.iter().map(|t| json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": (t.schema)(),
            })).collect::<Vec<_>>()
        })),
        "tools/call" => {
            let name = params["name"].as_str().unwrap_or_default();
            let tool = TOOLS
                .iter()
                .find(|t| t.name == name)
                .ok_or_else(|| RpcError::params(format!("unknown tool {name}")))?;
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let (text, is_error) = match session.handle(tool.method, args) {
                Ok(v) => (serde_json::to_string_pretty(&compact(v)).unwrap(), false),
                Err(e) => (e.message, true),
            };
            Ok(json!({ "content": [{ "type": "text", "text": text }], "isError": is_error }))
        }
        _ => Err(RpcError::new(
            RpcError::METHOD_NOT_FOUND,
            format!("unknown method {method}"),
        )),
    }
}

pub fn run(mut session: Session) -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req = match parse_request(&line) {
            Ok(r) => r,
            Err(resp) => {
                writeln!(out, "{}", serde_json::to_string(&resp)?)?;
                out.flush()?;
                continue;
            }
        };
        let Some(id) = req.id else { continue }; // notifications need no reply
        let resp = match handle(&mut session, &req.method, req.params) {
            Ok(v) => Response::ok(id, v),
            Err(e) => Response::err(id, e),
        };
        writeln!(out, "{}", serde_json::to_string(&resp)?)?;
        out.flush()?;
    }
    Ok(())
}

/// Same operations as MCP, restricted to private document editing and inspection.
pub fn assistant_methods() -> Vec<&'static str> {
    TOOLS
        .iter()
        .filter(|t| {
            !matches!(
                t.method,
                "doc.new" | "doc.open" | "doc.save" | "export" | "import" | "doc.undo"
            )
        })
        .map(|t| t.method)
        .collect()
}
pub fn assistant_catalog() -> Value {
    let allowed = assistant_methods();
    json!({"tools": TOOLS.iter().filter(|t| allowed.contains(&t.method)).map(|t| json!({
        "name":t.name,"method":t.method,"description":t.description,"parameters":(t.schema)(),
        "read_only":matches!(t.method,"core.info"|"doc.info"|"doc.feature"|"doc.topology"|"sketch.get"|"measure"|"mass_properties")
    })).collect::<Vec<_>>()})
}

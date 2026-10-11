//! TypeScript types and protocol tables generated from the tool catalog.
//! `openforge generate` rewrites them; `openforge generate --check` and the
//! `generated_files_are_current` test fail when the committed copies are stale.

use std::collections::BTreeMap;
use std::fmt::Write;

use serde_json::{Map, Value};

use super::{Entry, catalog, output_schema};
use of_protocol::ErrorCode;

/// Repository-relative output paths.
pub const TYPESCRIPT: &str = "apps/desktop/src/shared/tools.gen.ts";
pub const PROTOCOL: &str = "docs/protocol.md";

const HEADER: &str = "Generated from the core tool catalog (crates/openforge/src/tools). Do not edit:\n\
     run `cargo run -p openforge -- generate` instead.";

/// Every named definition across the catalog. Tools embed their own copies.
fn definitions() -> BTreeMap<String, Value> {
    let mut defs = BTreeMap::new();
    let schemas = catalog().iter().map(|e| &e.schema).chain([output_schema()]);
    for schema in schemas {
        for (name, def) in schema["$defs"].as_object().into_iter().flatten() {
            if let Some(old) = defs.insert(name.clone(), def.clone()) {
                assert_eq!(&old, def, "definition {name} differs between tools");
            }
        }
    }
    defs
}

fn doc(out: &mut String, indent: &str, text: Option<&str>) {
    if let Some(text) = text {
        let lines: Vec<&str> = text.lines().collect();
        if let [line] = lines.as_slice() {
            let _ = writeln!(out, "{indent}/** {line} */");
        } else {
            let _ = writeln!(out, "{indent}/**");
            for line in lines {
                let _ = writeln!(out, "{indent} * {line}");
            }
            let _ = writeln!(out, "{indent} */");
        }
    }
}

fn literal(v: &Value) -> String {
    match v {
        Value::String(s) => format!("'{}'", s.replace('\'', "\\'")),
        other => other.to_string(),
    }
}

/// A TypeScript type for one schema node.
fn ts(node: &Value, indent: &str) -> String {
    let o = match node.as_object() {
        Some(o) => o,
        None => return "unknown".into(),
    };
    if let Some(r) = o.get("$ref").and_then(Value::as_str) {
        return r.trim_start_matches("#/$defs/").into();
    }
    if let Some(c) = o.get("const") {
        return literal(c);
    }
    if let Some(Value::Array(values)) = o.get("enum") {
        return values.iter().map(literal).collect::<Vec<_>>().join(" | ");
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(Value::Array(branches)) = o.get(key) {
            let inner = format!("{indent}  ");
            // A branch that is itself a union contributes its own branches.
            fn flat(branches: &[Value]) -> Vec<&Value> {
                branches
                    .iter()
                    .flat_map(|b| match b.get("oneOf").and_then(Value::as_array) {
                        Some(inner) => flat(inner),
                        None => vec![b],
                    })
                    .collect()
            }
            let parts: Vec<String> = flat(branches).iter().map(|b| ts(b, &inner)).collect();
            if parts.iter().all(|p| !p.contains('\n')) && parts.join(" | ").len() < 80 {
                return parts.join(" | ");
            }
            return parts
                .iter()
                .map(|p| format!("\n{inner}| {p}"))
                .collect::<String>();
        }
    }
    let types: Vec<&str> = match o.get("type") {
        Some(Value::String(t)) => vec![t.as_str()],
        Some(Value::Array(ts)) => ts.iter().filter_map(Value::as_str).collect(),
        _ => return "unknown".into(),
    };
    let rendered: Vec<String> = types.iter().map(|t| primitive(o, t, indent)).collect();
    rendered.join(" | ")
}

fn primitive(o: &Map<String, Value>, t: &str, indent: &str) -> String {
    match t {
        "integer" | "number" => "number".into(),
        "string" => "string".into(),
        "boolean" => "boolean".into(),
        "null" => "null".into(),
        "array" => {
            let item = o.get("items").map_or("unknown".into(), |i| ts(i, indent));
            let (min, max) = (o.get("minItems"), o.get("maxItems"));
            match (min.and_then(Value::as_u64), max.and_then(Value::as_u64)) {
                (Some(n), Some(m)) if n == m && n <= 4 => {
                    format!("[{}]", vec![item; n as usize].join(", "))
                }
                _ if item.contains(' ') => format!("({item})[]"),
                _ => format!("{item}[]"),
            }
        }
        "object" => object(o, indent),
        _ => "unknown".into(),
    }
}

fn object(o: &Map<String, Value>, indent: &str) -> String {
    let values = o
        .get("additionalProperties")
        .filter(|v| v.is_object())
        .or_else(|| {
            o.get("patternProperties")
                .and_then(Value::as_object)
                .and_then(|p| p.values().next())
        });
    if let Some(values) = values {
        return format!("Record<string, {}>", ts(values, indent));
    }
    let props = o.get("properties").and_then(Value::as_object);
    let Some(props) = props.filter(|p| !p.is_empty()) else {
        return "Record<string, never>".into();
    };
    let required: Vec<&str> = o
        .get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let inner = format!("{indent}  ");
    let mut out = String::from("{\n");
    // Tags first, then the declared order of required fields, then the rest.
    let mut keys: Vec<&String> = props.keys().collect();
    keys.sort_by_key(|k| {
        let tag = props[*k].get("const").is_some();
        let pos = required.iter().position(|r| r == k).unwrap_or(usize::MAX);
        (!tag, pos, (*k).clone())
    });
    for key in keys {
        let field = &props[key];
        doc(
            &mut out,
            &inner,
            field.get("description").and_then(Value::as_str),
        );
        let optional = if required.contains(&key.as_str()) {
            ""
        } else {
            "?"
        };
        let _ = writeln!(out, "{inner}{key}{optional}: {}", ts(field, &inner));
    }
    out.push_str(indent);
    out.push('}');
    out
}

pub fn typescript() -> String {
    let mut out = String::new();
    for line in HEADER.lines() {
        let _ = writeln!(out, "// {line}");
    }
    out.push_str("// Lengths are millimetres and angles radians.\n");
    for (name, def) in definitions() {
        out.push('\n');
        doc(&mut out, "", def.get("description").and_then(Value::as_str));
        let _ = writeln!(out, "export type {name} ={}", spaced(ts(&def, "")));
    }
    out.push('\n');
    doc(
        &mut out,
        "",
        Some("The result of every tool call (MCP `structuredContent`, `draft.apply`)."),
    );
    let _ = writeln!(
        out,
        "export type Envelope ={}",
        spaced(ts(output_schema(), ""))
    );
    out.push_str(
        "\n/** Arguments of each catalog tool, by tool name. */\nexport interface ToolArgs {\n",
    );
    for Entry { tool, schema, .. } in catalog() {
        doc(&mut out, "  ", Some(tool.description));
        let _ = writeln!(out, "  {}:{}", tool.name, spaced(ts(schema, "  ")));
    }
    out.push_str("}\n\nexport type ToolName = keyof ToolArgs\n");
    out.push_str(
        "\n/** Core JSON-RPC method behind each tool. */\nexport const TOOL_METHODS = {\n",
    );
    for e in catalog() {
        let _ = writeln!(out, "  {}: '{}',", e.tool.name, e.tool.method);
    }
    out.push_str("} as const satisfies Record<ToolName, string>\n");
    // tsc checks every validated example against the generated types.
    out.push_str("\n/** Schema examples, validated and replayed on the core in CI. */\n");
    out.push_str("export const TOOL_EXAMPLES: { [K in ToolName]: ToolArgs[K][] } = {\n");
    for e in catalog() {
        let examples: Vec<String> = (e.tool.examples)().iter().map(Value::to_string).collect();
        let _ = writeln!(out, "  {}: [{}],", e.tool.name, examples.join(", "));
    }
    out.push_str("}\n");
    out
}

/// A type after `=` or `:`; multi-line unions start on the next line.
fn spaced(t: String) -> String {
    if t.starts_with('\n') {
        t
    } else {
        format!(" {t}")
    }
}

fn cell(text: &str) -> String {
    text.replace('\n', " ").replace('|', "\\|")
}

/// Field list such as "`sketch`, `end1`, `cut?`".
fn fields(node: &Value, skip: &str) -> String {
    let required: Vec<&str> = node["required"]
        .as_array()
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let props = node["properties"].as_object();
    let mut names: Vec<String> = required
        .iter()
        .filter(|k| **k != skip && props.is_some_and(|p| p.contains_key(**k)))
        .map(|k| format!("`{k}`"))
        .collect();
    names.extend(
        props
            .into_iter()
            .flatten()
            .filter(|(k, _)| k.as_str() != skip && !required.contains(&k.as_str()))
            .map(|(k, _)| format!("`{k}?`")),
    );
    if names.is_empty() {
        "none".into()
    } else {
        names.join(", ")
    }
}

/// One table row per branch of a tagged union.
fn union_table(out: &mut String, def: &Value, tag: &str, heading: [&str; 3]) {
    let _ = writeln!(
        out,
        "| {} | {} | {} |\n| --- | --- | --- |",
        heading[0], heading[1], heading[2]
    );
    let mut rows = Vec::new();
    let mut collect = |branch: &Value, outer: Option<&str>| {
        let name = branch["properties"][tag]["const"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let description = branch["description"].as_str().or(outer).unwrap_or_default();
        rows.push((name, fields(branch, tag), cell(description)));
    };
    for branch in def["oneOf"].as_array().into_iter().flatten() {
        match branch["oneOf"].as_array() {
            Some(inner) => {
                for b in inner {
                    collect(b, branch["description"].as_str());
                }
            }
            None => collect(branch, None),
        }
    }
    for (name, fields, description) in rows {
        let _ = writeln!(out, "| `{name}` | {fields} | {description} |");
    }
}

pub fn protocol_tables() -> (String, String) {
    let defs = definitions();
    let mut tools =
        String::from("| Tool | Method | Access | Arguments |\n| --- | --- | --- | --- |\n");
    for e in catalog() {
        let access = match (e.tool.read_only, e.tool.draft) {
            (true, _) => "read",
            (false, true) => "edit (draft)",
            (false, false) => "file/session (not in drafts)",
        };
        let _ = writeln!(
            tools,
            "| `{}` | `{}` | {access} | {} |",
            e.tool.name,
            e.tool.method,
            fields(&e.schema, "")
        );
    }
    tools.push_str("\n#### Sketch operations (`edit_sketch` `op`)\n\n");
    union_table(
        &mut tools,
        &defs["SketchOp"],
        "op",
        ["`op`", "Fields", "Notes"],
    );
    tools.push_str("\n#### Sketch relations and dimensions (`constraint`)\n\n");
    union_table(
        &mut tools,
        &defs["ConstraintKind"],
        "type",
        ["`type`", "Fields", "Notes"],
    );
    tools.push_str("\n#### Geometry references (`Ref`)\n\n");
    union_table(
        &mut tools,
        &defs["Ref"],
        "type",
        ["`type`", "Fields", "Notes"],
    );
    let mut features = String::new();
    union_table(
        &mut features,
        &defs["Feature"],
        "type",
        ["`type`", "Fields", "Notes"],
    );
    (tools, features)
}

/// Replace the text between `<!-- generated:NAME -->` and `<!-- /generated:NAME -->`.
fn splice(doc: &str, name: &str, body: &str) -> String {
    let (open, close) = (
        format!("<!-- generated:{name} -->"),
        format!("<!-- /generated:{name} -->"),
    );
    let start = doc
        .find(&open)
        .unwrap_or_else(|| panic!("{PROTOCOL} lacks {open}"))
        + open.len();
    let end = doc
        .find(&close)
        .unwrap_or_else(|| panic!("{PROTOCOL} lacks {close}"));
    format!(
        "{}\n<!-- {HEADER} -->\n\n{body}\n{}",
        &doc[..start],
        &doc[end..]
    )
    .replace("\n\n\n", "\n\n")
}

/// Error and warning codes with their meaning and repair hint.
pub fn error_table() -> String {
    let defs = definitions();
    let describe = |def: &str, name: &str| {
        defs[def]["oneOf"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|b| b["const"] == name)
            .and_then(|b| b["description"].as_str())
            .map(cell)
            .unwrap_or_default()
    };
    let mut out = String::from("| Code | Meaning | Repair |\n| --- | --- | --- |\n");
    for code in ErrorCode::ALL {
        let name = code.name();
        let _ = writeln!(
            out,
            "| `{name}` | {} | {} |",
            describe("ErrorCode", &name),
            cell(code.repair())
        );
    }
    out.push_str("\n| Warning | Meaning |\n| --- | --- |\n");
    for branch in defs["WarningCode"]["oneOf"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let name = branch["const"].as_str().unwrap_or_default();
        let _ = writeln!(out, "| `{name}` | {} |", describe("WarningCode", name));
    }
    out
}

pub fn protocol(current: &str) -> String {
    let (tools, features) = protocol_tables();
    let doc = splice(&splice(current, "tools", &tools), "features", &features);
    splice(&doc, "errors", &error_table())
}

/// Write the generated files under the repository `root`, or with `check`
/// only report which are stale. Returns the stale (or rewritten) paths.
pub fn run(root: &std::path::Path, check: bool) -> std::io::Result<Vec<String>> {
    let protocol_now = std::fs::read_to_string(root.join(PROTOCOL))?;
    let outputs = [
        (TYPESCRIPT, typescript()),
        (PROTOCOL, protocol(&protocol_now)),
    ];
    let mut stale = Vec::new();
    for (path, want) in outputs {
        let have = std::fs::read_to_string(root.join(path)).unwrap_or_default();
        if have != want {
            stale.push(path.to_owned());
            if !check {
                std::fs::write(root.join(path), &want)?;
            }
        }
    }
    Ok(stale)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn every_error_code_is_documented() {
        let table = super::error_table();
        for code in of_protocol::ErrorCode::ALL {
            let row = table
                .lines()
                .find(|l| l.starts_with(&format!("| `{}`", code.name())));
            let cells: Vec<&str> = row.expect("documented").split(" | ").collect();
            assert!(cells.iter().all(|c| !c.trim().is_empty()), "{row:?}");
        }
    }

    #[test]
    fn generated_files_are_current() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let stale = super::run(&root, true).unwrap();
        assert!(
            stale.is_empty(),
            "{stale:?} are stale: run `cargo run -p openforge -- generate`"
        );
    }
}

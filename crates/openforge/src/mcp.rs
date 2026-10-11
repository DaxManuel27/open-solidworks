//! A Model Context Protocol server over stdio, so AI assistants can build
//! and edit parts. Each tool in [`crate::tools`] forwards to a [`Session`]
//! method after its arguments pass the tool's generated schema.

use std::io::{BufRead, Write};

use of_protocol::{Response, RpcError, parse_request};
use serde_json::{Value, json};

use crate::session::Session;
use crate::tools;

const PROTOCOL_VERSION: &str = "2025-06-18";

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
        "tools/list" => Ok(tools::mcp_tools()),
        "tools/call" => {
            let name = params["name"].as_str().unwrap_or_default();
            let entry = tools::by_name(name)
                .ok_or_else(|| RpcError::params(format!("unknown tool {name}")))?;
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let result = entry
                .validate(&args)
                .and_then(|()| session.handle(entry.tool.method, args));
            let (text, is_error) = match result {
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

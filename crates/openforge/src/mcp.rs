//! A Model Context Protocol server over stdio, so AI assistants can build
//! and edit parts. Each tool in [`crate::tools`] forwards to a [`Session`]
//! method after its arguments pass the tool's generated schema.

use std::io::{BufRead, Write};

use of_protocol::{Response, RpcError, parse_request};
use serde_json::{Value, json};

use crate::session::Session;
use crate::tools;

const PROTOCOL_VERSION: &str = "2025-06-18";

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
            let envelope = tools::envelope::call(session, entry, args, None);
            let value = envelope.to_value();
            let text = format!(
                "{}\n{}",
                envelope.summary(),
                serde_json::to_string(&value).unwrap()
            );
            // Text carries the summary and the same JSON for text-only clients.
            Ok(json!({
                "content": [{ "type": "text", "text": text }],
                "structuredContent": value,
                "isError": !envelope.ok,
            }))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_results_mirror_ok_in_is_error() {
        let mut s = Session::new();
        let call = |s: &mut Session, name: &str, arguments: Value| {
            handle(
                s,
                "tools/call",
                json!({ "name": name, "arguments": arguments }),
            )
            .unwrap()
        };
        let r = call(
            &mut s,
            "new_sketch",
            json!({ "plane": { "type": "front_plane" } }),
        );
        assert_eq!(
            (r["isError"].clone(), r["structuredContent"]["ok"].clone()),
            (json!(false), json!(true))
        );
        let r = call(
            &mut s,
            "add_feature",
            json!({ "feature": {
            "type": "extrude", "sketch": 1, "end1": { "type": "blind", "depth": 5 } } }),
        );
        assert_eq!(r["isError"], true, "an empty sketch cannot rebuild");
        assert_eq!(r["structuredContent"]["error"]["code"], "E_REBUILD_FAILED");
        assert!(
            r["content"][0]["text"]
                .as_str()
                .unwrap()
                .starts_with("Failed (E_REBUILD_FAILED)")
        );
        let r = call(
            &mut s,
            "add_feature",
            json!({ "feature": { "type": "fillet", "radios": 1 } }),
        );
        assert_eq!(r["structuredContent"]["error"]["code"], "E_INVALID_ARGS");
        let list = handle(&mut s, "tools/list", json!({})).unwrap();
        assert!(
            list["tools"]
                .as_array()
                .unwrap()
                .iter()
                .all(|t| t["outputSchema"]["type"] == "object")
        );
    }
}

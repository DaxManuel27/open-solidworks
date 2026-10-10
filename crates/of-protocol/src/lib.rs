//! Messages exchanged between the core and its clients (the desktop app,
//! the CLI and the MCP server), carried as JSON-RPC 2.0, one message per line.
//!
//! The method list and payload shapes are documented in `docs/protocol.md`
//! and mirrored in `apps/desktop/src/renderer/src/protocol.ts`.

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u32 = 2;
pub const SCENE_VERSION: u32 = 2;

/// Capabilities describe implemented behavior, never planned features.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoreInfo {
    pub protocol_version: u32,
    pub scene_version: u32,
    pub part_file_version: u32,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    /// Absent for notifications.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl RpcError {
    pub const PARSE: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    /// The request was valid but the operation failed (bad geometry etc.).
    pub const FAILED: i64 = 1;

    pub fn new(code: i64, message: impl Into<String>) -> RpcError {
        RpcError {
            code,
            message: message.into(),
        }
    }

    pub fn failed(message: impl Into<String>) -> RpcError {
        RpcError::new(Self::FAILED, message)
    }

    pub fn params(message: impl Into<String>) -> RpcError {
        RpcError::new(Self::INVALID_PARAMS, message)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl Response {
    pub fn ok(id: Value, result: Value) -> Response {
        Response {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: Value, error: RpcError) -> Response {
        Response {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

/// Parse one line into a request, or the error response to send back.
pub fn parse_request(line: &str) -> Result<Request, Response> {
    let value: Value = serde_json::from_str(line)
        .map_err(|e| Response::err(Value::Null, RpcError::new(RpcError::PARSE, e.to_string())))?;
    let id = value.get("id").cloned().unwrap_or(Value::Null);
    serde_json::from_value(value)
        .map_err(|e| Response::err(id, RpcError::new(RpcError::INVALID_REQUEST, e.to_string())))
}

/// Little-endian bytes of `f32`s, base64-encoded (decoded in the app as a
/// `Float32Array`).
pub fn b64_f32(v: &[f32]) -> String {
    let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Little-endian bytes of `u32`s, base64-encoded.
pub fn b64_u32(v: &[u32]) -> String {
    let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Triangles and edge lines for the viewport, as base64 typed arrays.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MeshPayload {
    pub positions: String,
    pub normals: String,
    pub indices: String,
    /// Face i owns triangles `face_ranges[i]..face_ranges[i + 1]`.
    pub face_ranges: String,
    pub edge_points: String,
    /// Edge i owns points `edge_ranges[i]..edge_ranges[i + 1]`.
    pub edge_ranges: String,
    /// Vertex positions, for picking vertices.
    pub vertices: String,
    pub face_count: u32,
    pub edge_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_requests_and_reports_errors() {
        let r = parse_request(r#"{"jsonrpc":"2.0","id":3,"method":"doc.info"}"#).unwrap();
        assert_eq!(r.method, "doc.info");
        assert_eq!(r.id, Some(Value::from(3)));
        let e = parse_request("{oops").unwrap_err();
        assert_eq!(e.error.unwrap().code, RpcError::PARSE);
        let e = parse_request(r#"{"id":7}"#).unwrap_err();
        assert_eq!(e.id, Value::from(7));
    }

    #[test]
    fn encodes_little_endian() {
        assert_eq!(b64_u32(&[1]), "AQAAAA==");
        assert_eq!(b64_f32(&[1.0]), "AACAPw==");
    }
}

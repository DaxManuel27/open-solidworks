//! Messages exchanged between the core and its clients (the desktop app,
//! the CLI and the MCP server), carried as JSON-RPC 2.0, one message per line.
//!
//! The method list and payload shapes are documented in `docs/protocol.md`
//! and mirrored in `apps/desktop/src/renderer/src/protocol.ts`.

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u32 = 3;
pub const SCENE_VERSION: u32 = 3;

/// Capabilities describe implemented behavior, never planned features.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoreInfo {
    pub protocol_version: u32,
    pub scene_version: u32,
    pub part_file_version: u32,
    #[serde(default)]
    pub assembly_file_version: u32,
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
    /// Machine-readable detail: a stable [`ErrorCode`], the JSON path of the
    /// offending argument and the values allowed there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Box<ErrorData>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ErrorData {
    pub code: ErrorCode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed: Option<Vec<String>>,
}

/// Stable error codes reported to tool clients. Messages may change; codes do not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum ErrorCode {
    /// Arguments do not match the tool's input schema.
    #[serde(rename = "E_INVALID_ARGS")]
    InvalidArgs,
    /// No feature has this ID.
    #[serde(rename = "E_UNKNOWN_FEATURE")]
    UnknownFeature,
    /// The feature exists but is not a sketch.
    #[serde(rename = "E_NOT_A_SKETCH")]
    NotASketch,
    /// No sketch entity or relation has this ID, or it is the wrong kind.
    #[serde(rename = "E_UNKNOWN_ENTITY")]
    UnknownEntity,
    /// A face, edge, plane or other reference does not resolve.
    #[serde(rename = "E_UNKNOWN_REF")]
    UnknownRef,
    /// No registered parameter has this key.
    #[serde(rename = "E_UNKNOWN_PARAMETER")]
    UnknownParameter,
    /// No configuration has this ID.
    #[serde(rename = "E_UNKNOWN_CONFIGURATION")]
    UnknownConfiguration,
    /// No body has this ID.
    #[serde(rename = "E_UNKNOWN_BODY")]
    UnknownBody,
    /// A variable, equation or configuration is invalid.
    #[serde(rename = "E_INVALID_FORMULA")]
    InvalidFormula,
    /// The call added or changed a feature that failed to rebuild.
    #[serde(rename = "E_REBUILD_FAILED")]
    RebuildFailed,
    /// A material feature rebuilt but added or removed no material.
    #[serde(rename = "E_NO_EFFECT")]
    NoEffect,
    /// The document changed since the request's revision.
    #[serde(rename = "E_STALE_REVISION")]
    StaleRevision,
    /// A manual feature or sketch edit is open.
    #[serde(rename = "E_EDIT_IN_PROGRESS")]
    EditInProgress,
    /// The draft does not exist or expired.
    #[serde(rename = "E_DRAFT_MISSING")]
    DraftMissing,
    /// The operation is not available here (for example, a file operation in a draft).
    #[serde(rename = "E_NOT_ALLOWED")]
    NotAllowed,
    /// Any other failure.
    #[serde(rename = "E_FAILED")]
    Failed,
}

impl ErrorCode {
    pub const ALL: [ErrorCode; 16] = [
        Self::InvalidArgs,
        Self::UnknownFeature,
        Self::NotASketch,
        Self::UnknownEntity,
        Self::UnknownRef,
        Self::UnknownParameter,
        Self::UnknownConfiguration,
        Self::UnknownBody,
        Self::InvalidFormula,
        Self::RebuildFailed,
        Self::NoEffect,
        Self::StaleRevision,
        Self::EditInProgress,
        Self::DraftMissing,
        Self::NotAllowed,
        Self::Failed,
    ];

    pub fn name(self) -> String {
        serde_json::to_value(self)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned()
    }

    /// What a client should do next.
    pub fn repair(self) -> &'static str {
        match self {
            Self::InvalidArgs => {
                "Fix the argument at `path`; `allowed` lists valid values. The tool schema and its examples show the shape."
            }
            Self::UnknownFeature => {
                "Read feature IDs from part_info `features`; do not guess them."
            }
            Self::NotASketch => "Pass a sketch feature ID (type sketch in part_info `features`).",
            Self::UnknownEntity => {
                "Read entity IDs from get_sketch or the `created` list of the edit that made them."
            }
            Self::UnknownRef => {
                "Take face and edge refs from topology or the user's selection, and plane refs from part_info."
            }
            Self::UnknownParameter => "Use a key from part_info `parameters`.",
            Self::UnknownConfiguration => "Use an ID from part_info `configurations.items`.",
            Self::UnknownBody => "Use a body ID from part_info `bodies`.",
            Self::InvalidFormula => {
                "Check variable names, units (lengths need mm, in, …) and that every key exists in part_info `parameters`."
            }
            Self::RebuildFailed => {
                "Read the feature with get_feature, then fix it with update_feature (smaller radius, other direction, valid refs) or delete it."
            }
            Self::NoEffect => {
                "Fix the feature with update_feature: a cut sketched on a face usually needs `reverse: true`; check the sketch lies on or inside the part."
            }
            Self::StaleRevision => {
                "The part changed; read part_info again and start over from the current state."
            }
            Self::EditInProgress => "Wait until the user finishes the open feature or sketch edit.",
            Self::DraftMissing => "Start a new draft; drafts expire after ten minutes.",
            Self::NotAllowed => {
                "Use a tool from the catalog; file operations and undo stay with the user."
            }
            Self::Failed => {
                "Read the message, inspect the part with part_info, and try a different approach."
            }
        }
    }
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
            data: None,
        }
    }

    /// An error with a stable code. Argument errors use JSON-RPC
    /// invalid-params; everything else is an operation failure.
    pub fn coded(code: ErrorCode, message: impl Into<String>) -> RpcError {
        let rpc = match code {
            ErrorCode::InvalidArgs => Self::INVALID_PARAMS,
            _ => Self::FAILED,
        };
        RpcError {
            data: Some(Box::new(ErrorData {
                code,
                path: None,
                allowed: None,
            })),
            ..RpcError::new(rpc, message)
        }
    }

    /// The stable code, falling back by JSON-RPC code for untagged errors.
    pub fn error_code(&self) -> ErrorCode {
        match &self.data {
            Some(d) => d.code,
            None if self.code == Self::INVALID_PARAMS => ErrorCode::InvalidArgs,
            None => ErrorCode::Failed,
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
pub fn parse_request(line: &str) -> Result<Request, Box<Response>> {
    let value: Value = serde_json::from_str(line).map_err(|e| {
        Box::new(Response::err(
            Value::Null,
            RpcError::new(RpcError::PARSE, e.to_string()),
        ))
    })?;
    let id = value.get("id").cloned().unwrap_or(Value::Null);
    serde_json::from_value(value).map_err(|e| {
        Box::new(Response::err(
            id,
            RpcError::new(RpcError::INVALID_REQUEST, e.to_string()),
        ))
    })
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
    fn error_codes_are_stable_and_documented() {
        let names: Vec<String> = ErrorCode::ALL.iter().map(|c| c.name()).collect();
        assert!(names.iter().all(|n| n.starts_with("E_")));
        assert_eq!(
            names.len(),
            names
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
        );
        assert!(ErrorCode::ALL.iter().all(|c| !c.repair().is_empty()));
        let e = RpcError::coded(ErrorCode::NoEffect, "nothing removed");
        assert_eq!(
            (e.code, e.error_code()),
            (RpcError::FAILED, ErrorCode::NoEffect)
        );
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["data"]["code"], "E_NO_EFFECT");
        assert_eq!(RpcError::params("x").error_code(), ErrorCode::InvalidArgs);
        assert!(
            serde_json::to_value(RpcError::failed("x"))
                .unwrap()
                .get("data")
                .is_none()
        );
    }

    #[test]
    fn encodes_little_endian() {
        assert_eq!(b64_u32(&[1]), "AQAAAA==");
        assert_eq!(b64_f32(&[1.0]), "AACAPw==");
    }
}

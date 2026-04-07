//! JSON-RPC 2.0 protocol types for Deno plugin communication.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A JSON-RPC 2.0 request sent to the Deno plugin subprocess.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    pub id: u64,
}

/// A JSON-RPC 2.0 response received from the Deno plugin subprocess.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: u64,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Lifecycle hook names used as JSON-RPC method names.
pub const METHOD_INIT: &str = "on_init";
pub const METHOD_BUILD_START: &str = "on_build_start";
pub const METHOD_CONTENT_PARSED: &str = "on_content_parsed";
pub const METHOD_GRAPH_UPDATED: &str = "on_graph_updated";
pub const METHOD_VALIDATION_COMPLETE: &str = "on_validation_complete";
pub const METHOD_PAGE_RENDER: &str = "on_page_render";
pub const METHOD_BUILD_COMPLETE: &str = "on_build_complete";
pub const METHOD_FILE_CHANGED: &str = "on_file_changed";
pub const METHOD_NAME: &str = "name";
pub const METHOD_SHUTDOWN: &str = "shutdown";

impl JsonRpcRequest {
    /// Create a new JSON-RPC request.
    pub fn new(method: &str, params: Option<serde_json::Value>, id: u64) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id,
        }
    }
}

impl JsonRpcResponse {
    /// Check if this response is an error.
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    /// Extract the error message, if any.
    pub fn error_message(&self) -> Option<&str> {
        self.error.as_ref().map(|e| e.message.as_str())
    }
}

/// Parameters sent with the `on_init` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitParams {
    pub base_url: String,
    pub title: String,
    pub options: HashMap<String, toml::Value>,
}

/// Parameters sent with the `on_content_parsed` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentParsedParams {
    pub page: geoff_plugin::context::PageData,
}

/// Parameters sent with the `on_page_render` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageRenderParams {
    pub page: geoff_plugin::context::PageData,
    pub extra_vars: HashMap<String, serde_json::Value>,
}

/// Result returned from `on_page_render`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageRenderResult {
    pub page: geoff_plugin::context::PageData,
    pub extra_vars: HashMap<String, serde_json::Value>,
}

/// Result returned from `on_content_parsed`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentParsedResult {
    pub page: geoff_plugin::context::PageData,
}

/// Parameters sent with the `on_validation_complete` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationParams {
    pub conforms: bool,
    pub violations: usize,
}

/// Parameters sent with the `on_file_changed` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangedParams {
    pub changed_path: String,
}

// framework/src/disco/models.rs
use serde::{Deserialize, Serialize};
use serde_json::Value;

// JSON-RPC 2.0 Request
#[derive(Serialize, Deserialize, Debug)]
pub struct Request {
    pub jsonrpc: String,
    pub id: Option<Value>, // Can be string, number, or null for notifications
    pub method: String,
    pub params: Option<Value>,
}

// JSON-RPC 2.0 Response
#[derive(Serialize, Deserialize, Debug)]
pub struct Response {
    pub jsonrpc: String,
    pub id: Value, // Must match the request id
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorObject>,
}

// JSON-RPC 2.0 Error Object
#[derive(Serialize, Deserialize, Debug)]
pub struct ErrorObject {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// --- MCP Specific Payloads ---

// For the 'initialize' method parameters
#[derive(Serialize, Deserialize, Debug)]
pub struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: Value, // For now, we'll just accept any capabilities
    #[serde(rename = "clientInfo")]
    pub client_info: ClientInfo,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

// For the 'initialize' method result
#[derive(Serialize, Deserialize, Debug)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: Capabilities,
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Capabilities {
    pub tools: ToolCapability,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ToolCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
}

// For the 'tools/list' method result
#[derive(Serialize, Deserialize, Debug)]
pub struct ToolsListResult {
    pub tools: Vec<ToolDefinition>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

// For the 'tools/call' method result
#[derive(Serialize, Deserialize, Debug)]
pub struct ToolCallResult {
    pub content: Vec<Content>,
    #[serde(rename = "isError")]
    pub is_error: bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum Content {
    #[serde(rename = "text")]
    Text { text: String },
}
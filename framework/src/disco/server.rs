use std::io::{self, BufRead};
use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use crate::disco::interactive_tools::{parser, runner::ToolRunner, session::SessionManager};
use crate::disco::models::{
    Capabilities, Content, ErrorObject, InitializeParams, InitializeResult, Request, Response,
    ServerInfo, ToolCallResult, ToolCapability, ToolDefinition, ToolsListResult,
};

enum ServerState {
    Uninitialized,
    Initializing,
    Initialized,
}

pub async fn run_disco_server() -> std::io::Result<()> {
    let mut state = ServerState::Uninitialized;
    let tools_dir = Path::new("src/disco/interactive_tools/tools_yaml");
    let interactive_tools = parser::load_tools(tools_dir).unwrap();
    let session_manager = SessionManager::new();
    let tool_runner = Arc::new(ToolRunner::new(interactive_tools, session_manager));

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<Request>(&line) {
            Ok(request) => {
                match state {
                    ServerState::Uninitialized => {
                        if request.method == "initialize" {
                            if let Some(id) = request.id {
                                if let Some(params_value) = request.params {
                                    if let Ok(params) =
                                        serde_json::from_value::<InitializeParams>(params_value)
                                    {
                                        let result = InitializeResult {
                                            protocol_version: params.protocol_version,
                                            server_info: ServerInfo {
                                                name: "Noventa MCP Server".to_string(),
                                                version: "0.1.0".to_string(),
                                            },
                                            capabilities: Capabilities {
                                                tools: ToolCapability {
                                                    list_changed: false,
                                                },
                                            },
                                        };
                                        let response = Response {
                                            jsonrpc: "2.0".to_string(),
                                            id,
                                            result: Some(serde_json::to_value(result).unwrap()),
                                            error: None,
                                        };
                                        if let Ok(response_json) = serde_json::to_string(&response)
                                        {
                                            println!("{}", response_json);
                                            state = ServerState::Initializing;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    ServerState::Initializing => {
                        if request.method == "notifications/initialized" {
                            state = ServerState::Initialized;
                        }
                    }
                    ServerState::Initialized => {
                        if let Some(id) = request.id {
                            let response = match request.method.as_str() {
                                "tools/list" => {
                                    let mut tools = vec![ToolDefinition {
                                        name: "read_file".to_string(),
                                        description: "Reads the contents of a file.".to_string(),
                                        input_schema: serde_json::json!({
                                            "type": "object",
                                            "properties": {
                                                "path": {
                                                    "type": "string",
                                                    "description": "The path to the file."
                                                }
                                            },
                                            "required": ["path"]
                                        }),
                                    }];
                                    let tool_runner_clone = tool_runner.clone();
                                    for tool in tool_runner_clone.tools.values() {
                                        tools.push(ToolDefinition {
                                            name: tool.name.clone(),
                                            description: tool.description.clone(),
                                            input_schema: serde_json::json!({
                                                "type": "object",
                                                "properties": {
                                                    "user_id": { "type": "string" },
                                                    "user_input": { "type": "integer" }
                                                },
                                                "required": ["user_id"]
                                            }),
                                        });
                                    }
                                    let result = ToolsListResult { tools };
                                    Response {
                                        jsonrpc: "2.0".to_string(),
                                        id,
                                        result: Some(serde_json::to_value(result).unwrap()),
                                        error: None,
                                    }
                                }
                                "tools/call" => {
                                    if let Some(params) = request.params {
                                        let tool_name = params
                                            .get("name")
                                            .and_then(Value::as_str)
                                            .unwrap_or_default();
                                        let arguments = params.get("arguments").unwrap_or(&Value::Null);
                                        let tool_runner_clone = tool_runner.clone();
                                        let result =
                                            if tool_runner_clone.tools.contains_key(tool_name) {
                                                crate::disco::tools::run_interactive_tool(
                                                    &tool_runner_clone,
                                                    tool_name,
                                                    arguments,
                                                )
                                            } else if tool_name == "read_file" {
                                                crate::disco::tools::read_file(arguments)
                                            } else {
                                                Err("Unknown tool".to_string())
                                            };
                                        let tool_result = match result {
                                            Ok(value) => ToolCallResult {
                                                content: vec![Content::Text {
                                                    text: value
                                                        .as_str()
                                                        .unwrap_or_default()
                                                        .to_string(),
                                                }],
                                                is_error: false,
                                            },
                                            Err(e) => ToolCallResult {
                                                content: vec![Content::Text { text: e }],
                                                is_error: true,
                                            },
                                        };
                                        Response {
                                            jsonrpc: "2.0".to_string(),
                                            id,
                                            result: Some(
                                                serde_json::to_value(tool_result).unwrap(),
                                            ),
                                            error: None,
                                        }
                                    } else {
                                        Response {
                                            jsonrpc: "2.0".to_string(),
                                            id,
                                            result: None,
                                            error: Some(ErrorObject {
                                                code: -32602,
                                                message: "Invalid params".to_string(),
                                                data: None,
                                            }),
                                        }
                                    }
                                }
                                _ => Response {
                                    jsonrpc: "2.0".to_string(),
                                    id,
                                    result: None,
                                    error: Some(ErrorObject {
                                        code: -32601,
                                        message: "Method not found".to_string(),
                                        data: None,
                                    }),
                                },
                            };
                            if let Ok(response_json) = serde_json::to_string(&response) {
                                println!("{}", response_json);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let error_response = Response {
                    jsonrpc: "2.0".to_string(),
                    id: Value::Null,
                    result: None,
                    error: Some(ErrorObject {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                        data: None,
                    }),
                };
                if let Ok(response_json) = serde_json::to_string(&error_response) {
                    println!("{}", response_json);
                }
            }
        }
    }
    Ok(())
}
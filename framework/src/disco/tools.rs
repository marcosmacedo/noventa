// framework/src/disco/tools.rs
use serde_json::Value;
use std::fs;

pub fn read_file(args: &Value) -> Result<Value, String> {
    if let Some(path) = args.get("path").and_then(Value::as_str) {
        match fs::read_to_string(path) {
            Ok(contents) => Ok(Value::String(contents)),
            Err(e) => Err(format!("Failed to read file: {}", e)),
        }
    } else {
        Err("Missing or invalid 'path' argument".to_string())
    }
}

use crate::disco::interactive_tools::runner::ToolRunner;

pub fn run_interactive_tool(tool_runner: &ToolRunner, tool_name: &str, args: &Value) -> Result<Value, String> {
    let user_input = args.get("user_input").and_then(Value::as_u64).map(|u| u as usize);

    let response = tool_runner.run_tool(tool_name, user_input);
    Ok(Value::String(response))
}
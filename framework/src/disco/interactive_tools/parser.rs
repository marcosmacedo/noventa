// framework/src/disco/interactive_tools/parser.rs
use crate::disco::interactive_tools::models::InteractiveTool;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub fn load_tools(dir: &Path) -> Result<HashMap<String, InteractiveTool>, String> {
    let mut tools = HashMap::new();
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.path().extension().and_then(|s| s.to_str()) == Some("yaml") {
            let content = fs::read_to_string(entry.path())
                .map_err(|e| format!("Failed to read tool file: {}", e))?;
            let tool: InteractiveTool = serde_yaml::from_str(&content)
                .map_err(|e| format!("Failed to parse tool file: {}", e))?;
            tools.insert(tool.name.clone(), tool);
        }
    }
    Ok(tools)
}
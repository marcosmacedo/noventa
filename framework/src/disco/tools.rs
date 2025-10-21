// framework/src/disco/tools.rs
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

pub trait Tool: Send + Sync {
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn input_schema(&self) -> Value;
    fn run(&self, args: &Value) -> Result<Value, String>;
}

struct ReadFileTool;

impl Tool for ReadFileTool {
    fn name(&self) -> String {
        "read_file".to_string()
    }

    fn description(&self) -> String {
        "Use this tool to read the contents of a file. It will also provide helpful information about the file's purpose in a Noventa project, such as identifying it as a component, a page, or a layout.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file."
                }
            },
            "required": ["path"]
        })
    }

    fn run(&self, args: &Value) -> Result<Value, String> {
        let path_str = args
            .get("path")
            .and_then(Value::as_str)
            .ok_or("Missing or invalid 'path' argument".to_string())?;

        let path = std::path::Path::new(path_str);
        let contents = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        let metadata = get_file_metadata(path, true);

        let response = format!(
            "```\n{}\n```\n{}",
            contents,
            metadata.unwrap_or_default()
        );
        Ok(Value::String(response))
    }
}

enum PathType {
    ComponentLogic(String),
    ComponentTemplate(String),
    ComponentModel(String),
    PageTemplate(String),
    PageLayout,
    File,
    Directory,
}

fn get_path_type(path: &std::path::Path) -> PathType {
    let path_str = path.to_str().unwrap_or_default();

    if path.is_dir() {
        return PathType::Directory;
    }

    if path_str.contains("components/") {
        let file_name = path.file_name().unwrap_or_default().to_str().unwrap_or_default();
        let parent_folder = path.parent().unwrap_or(path).file_name().unwrap_or_default().to_str().unwrap_or_default().to_string();

        if file_name.ends_with("_logic.py") {
            return PathType::ComponentLogic(parent_folder);
        } else if file_name.ends_with("_template.html") {
            return PathType::ComponentTemplate(parent_folder);
        } else if file_name.ends_with("_models.py") {
            return PathType::ComponentModel(parent_folder);
        }
    } else if let Some(pages_index) = path_str.find("/pages/") {
        if path_str.ends_with(".html") {
            let route_part = &path_str[pages_index + "/pages/".len()..];
            let route = route_part.strip_suffix(".html").unwrap_or(route_part);
            let route = route.strip_suffix("index").unwrap_or(route);
            let route = if route.is_empty() { "/" } else { route };
            let route = if !route.starts_with('/') {
                format!("/{}", route)
            } else {
                route.to_string()
            };
            return PathType::PageTemplate(route);
        }
    } else if path_str.contains("/layouts/") && path_str.ends_with(".html") {
        return PathType::PageLayout;
    }

    PathType::File
}

fn get_file_metadata(path: &std::path::Path, full_metadata: bool) -> Option<String> {
    let path_type = get_path_type(path);

    match path_type {
        PathType::ComponentLogic(parent) => Some(if full_metadata {
            format!("Metadata of the file:\nPython component logic that returns context to the jinja view for component '{}'", parent)
        } else {
            "Component Logic".to_string()
        }),
        PathType::ComponentTemplate(parent) => Some(if full_metadata {
            format!("Metadata of the file:\nJinja template for component '{}'", parent)
        } else {
            "Component Template".to_string()
        }),
        PathType::ComponentModel(parent) => Some(if full_metadata {
            format!("Metadata of the file:\nSQLAlchemy model for component '{}'", parent)
        } else {
            "Component Model".to_string()
        }),
        PathType::PageTemplate(route) => Some(if full_metadata {
            format!("Metadata of the file:\nPage template that generates the route: {}", route)
        } else {
            "Page Template".to_string()
        }),
        PathType::PageLayout => Some(if full_metadata {
            "Metadata of the file:\nJinja layout used across pages".to_string()
        } else {
            "Page Layout".to_string()
        }),
        PathType::File | PathType::Directory => None,
    }
}

struct ListDirectoryTool;

impl Tool for ListDirectoryTool {
    fn name(&self) -> String {
        "list_directory".to_string()
    }

    fn description(&self) -> String {
        "Use this tool to see what's inside a directory. It will show you a list of files and folders, and it will tell you if they are special Noventa files like components or pages.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the directory to list."
                }
            },
            "required": ["path"]
        })
    }

    fn run(&self, args: &Value) -> Result<Value, String> {
        let path_str = args
            .get("path")
            .and_then(Value::as_str)
            .ok_or("Missing or invalid 'path' argument".to_string())?;

        let base_path = std::path::Path::new(path_str);
        let entries = fs::read_dir(base_path)
            .map_err(|e| format!("Failed to read directory: {}", e))?;

        let mut output_table = Vec::new();
        output_table.push(vec!["Path".to_string(), "Type".to_string()]);

        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                let relative_path = path.strip_prefix(base_path).unwrap_or(&path);
                let path_str = relative_path.to_str().unwrap_or_default().to_string();

                let path_type = get_path_type(&path);
                let type_str = match path_type {
                    PathType::ComponentLogic(_) => "Component Logic",
                    PathType::ComponentTemplate(_) => "Component Template",
                    PathType::ComponentModel(_) => "Component Model",
                    PathType::PageTemplate(_) => "Page Template",
                    PathType::PageLayout => "PageLayout",
                    PathType::File => "File",
                    PathType::Directory => "Directory",
                };

                output_table.push(vec![path_str, type_str.to_string()]);
            }
        }

        let mut col_widths = vec![0; 2];
        for row in &output_table {
            for (i, cell) in row.iter().enumerate() {
                if cell.len() > col_widths[i] {
                    col_widths[i] = cell.len();
                }
            }
        }

        let mut result = String::new();
        for (r_idx, row) in output_table.iter().enumerate() {
            for (c_idx, cell) in row.iter().enumerate() {
                result.push_str(&format!("{:<width$}", cell, width = col_widths[c_idx] + 2));
            }
            result.push('\n');
            if r_idx == 0 {
                for (c_idx, width) in col_widths.iter().enumerate() {
                    result.push_str(&"-".repeat(*width + 2));
                }
                result.push('\n');
            }
        }

        Ok(Value::String(result))
    }
}

struct CreateDirectoryTool;

impl Tool for CreateDirectoryTool {
    fn name(&self) -> String {
        "create_directory".to_string()
    }

    fn description(&self) -> String {
        "Use this tool to create a new directory.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path of the directory to create."
                }
            },
            "required": ["path"]
        })
    }

    fn run(&self, args: &Value) -> Result<Value, String> {
        let path_str = args
            .get("path")
            .and_then(Value::as_str)
            .ok_or("Missing or invalid 'path' argument".to_string())?;

        let path = std::path::Path::new(path_str);
        fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create directory: {}", e))?;

        let parent = path.parent().unwrap_or(path);
        let parent_path_str = parent.to_str().unwrap_or_default();

        let description = if parent_path_str.contains("/pages") {
            "within the pages directory, which may be part of a new route".to_string()
        } else if parent_path_str.contains("/layouts") {
            "within the layouts directory".to_string()
        } else if parent_path_str.contains("/components") {
            "within the components directory, likely for a new component".to_string()
        } else {
            format!("inside '{}'", parent.display())
        };

        Ok(Value::String(format!(
            "Successfully created directory '{}' {}.",
            path.display(),
            description
        )))
    }
}

struct WriteFileTool;

impl Tool for WriteFileTool {
    fn name(&self) -> String {
        "write_file".to_string()
    }

    fn description(&self) -> String {
        "Use this tool to create a new file with content or override an existing file with new content.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path of the file to write."
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file."
                }
            },
            "required": ["path", "content"]
        })
    }

    fn run(&self, args: &Value) -> Result<Value, String> {
        let path_str = args
            .get("path")
            .and_then(Value::as_str)
            .ok_or("Missing or invalid 'path' argument".to_string())?;
        let content = args
            .get("content")
            .and_then(Value::as_str)
            .ok_or("Missing or invalid 'content' argument".to_string())?;

        let path = std::path::Path::new(path_str);

        // Security check: Ensure the path is within the current working directory.
        let current_dir = std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            current_dir.join(path)
        };
        let absolute_path = absolute_path.canonicalize().unwrap_or(absolute_path);

        if !absolute_path.starts_with(&current_dir) {
            return Err("Error: Writing to paths outside the current working directory is not allowed.".to_string());
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create parent directories: {}", e))?;
        }

        fs::write(path, content)
            .map_err(|e| format!("Failed to write to file: {}", e))?;

        let path_type = get_path_type(path);
        let parent_path = path.parent().unwrap_or(path);
        let parent_path_str = parent_path.to_str().unwrap_or_default();

        let message = if parent_path_str.contains("/components") {
            let is_valid_component_file = path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.ends_with("_logic.py") || name.ends_with("_template.html") || name.ends_with("_models.py"))
                .unwrap_or(false);

            let is_in_subdirectory = parent_path
                .strip_prefix("/components")
                .map(|p| p.components().count() > 0)
                .unwrap_or(false);

            if is_valid_component_file && is_in_subdirectory {
                let component_name = parent_path.file_name().unwrap_or_default().to_str().unwrap_or_default();
                format!("Successfully wrote component file for component '{}'.", component_name)
            } else {
                "WARNING: Files inside '/components/' must be placed in a subdirectory (e.g., '/components/my_component/') and follow component naming conventions (_logic.py, _template.html, _models.py).".to_string()
            }
        } else {
            match path_type {
                PathType::ComponentLogic(comp) | PathType::ComponentTemplate(comp) | PathType::ComponentModel(comp) => {
                     format!("WARNING: You wrote a component file for '{}' outside the '/components' directory. It should be in a subdirectory like '/components/{}' to be recognized.", comp, comp)
                }
                PathType::PageTemplate(route) => {
                    if parent_path_str.contains("/pages") {
                        format!("Successfully wrote page template, which creates the route: {}", route)
                    } else {
                        "WARNING: You wrote an HTML file outside the '/pages' and '/layouts' directories. If this is a page, it should be in '/pages' to generate a route. If it's a reusable layout, consider placing it in '/layouts'.".to_string()
                    }
                }
                PathType::PageLayout => {
                    if parent_path_str.contains("/layouts") {
                        "Successfully wrote layout file.".to_string()
                    } else {
                        "WARNING: You wrote an HTML file outside the '/pages' and '/layouts' directories. If this is a page, it should be in '/pages' to generate a route. If it's a reusable layout, consider placing it in '/layouts'.".to_string()
                    }
                }
                PathType::File => format!("Successfully wrote file to '{}'.", path_str),
                PathType::Directory => "This tool is for writing files, not directories.".to_string(),
            }
        };

        Ok(Value::String(message))
    }
}


use crate::disco::interactive_tools::runner::ToolRunner;

struct DeleteDirectoryTool;

impl Tool for DeleteDirectoryTool {
    fn name(&self) -> String {
        "delete_directory".to_string()
    }

    fn description(&self) -> String {
        "Use this tool to delete a directory and everything inside it.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path of the directory to delete."
                }
            },
            "required": ["path"]
        })
    }

    fn run(&self, args: &Value) -> Result<Value, String> {
        let path_str = args
            .get("path")
            .and_then(Value::as_str)
            .ok_or("Missing or invalid 'path' argument".to_string())?;

        let path = std::path::Path::new(path_str);

        // Security check: Ensure the path is within the current working directory.
        let current_dir = std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
        let absolute_path = path.canonicalize().map_err(|e| format!("Failed to resolve path: {}", e))?;
        if !absolute_path.starts_with(&current_dir) {
            return Err("Error: Deletion of paths outside the current working directory is not allowed.".to_string());
        }

        // Guard against deleting protected directories.
        let protected_dirs = ["web/components", "web/pages", "web/layouts"];
        if protected_dirs.iter().any(|&dir| path_str == dir) {
            return Err(format!("Error: The directory '{}' is protected and cannot be deleted.", path_str));
        }

        fs::remove_dir_all(path)
            .map_err(|e| format!("Failed to delete directory: {}", e))?;

        Ok(Value::String(format!("Successfully deleted directory '{}'.", path_str)))
    }
}

struct DeleteFileTool;

impl Tool for DeleteFileTool {
    fn name(&self) -> String {
        "delete_file".to_string()
    }

    fn description(&self) -> String {
        "Use this tool to delete a file.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path of the file to delete."
                }
            },
            "required": ["path"]
        })
    }

    fn run(&self, args: &Value) -> Result<Value, String> {
        let path_str = args
            .get("path")
            .and_then(Value::as_str)
            .ok_or("Missing or invalid 'path' argument".to_string())?;

        let path = std::path::Path::new(path_str);

        // Security check: Ensure the path is within the current working directory.
        let current_dir = std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
        let absolute_path = path.canonicalize().map_err(|e| format!("Failed to resolve path: {}", e))?;
        if !absolute_path.starts_with(&current_dir) {
            return Err("Error: Deletion of paths outside the current working directory is not allowed.".to_string());
        }

        fs::remove_file(path)
            .map_err(|e| format!("Failed to delete file: {}", e))?;

        Ok(Value::String(format!("Successfully deleted file '{}'.", path_str)))
    }
}

pub struct ToolManager {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolManager {
    pub fn new() -> Self {
        let mut manager = Self {
            tools: HashMap::new(),
        };
        manager.register_tool(Arc::new(ReadFileTool));
        manager.register_tool(Arc::new(ListDirectoryTool));
        manager.register_tool(Arc::new(CreateDirectoryTool));
        manager.register_tool(Arc::new(WriteFileTool));
        manager.register_tool(Arc::new(DeleteDirectoryTool));
        manager.register_tool(Arc::new(DeleteFileTool));
        manager
    }

    pub fn register_tool(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name(), tool);
    }

    pub fn get_tool(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn get_all_tools(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.values().cloned().collect()
    }
}

pub fn run_interactive_tool(
    tool_runner: &ToolRunner,
    tool_name: &str,
    args: &Value,
) -> Result<Value, String> {
    let user_input = args
        .get("user_input")
        .and_then(Value::as_u64)
        .map(|u| u as usize);

    let response = tool_runner.run_tool(tool_name, user_input);
    Ok(Value::String(response))
}
use crate::actors::interpreter::PythonError;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

lazy_static! {
    pub static ref ERROR_CHANNEL: broadcast::Sender<String> = broadcast::channel(100).0;
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DetailedError {
    pub message: String,
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub end_line: Option<u32>,
    pub end_column: Option<u32>,
    pub error_source: Option<ErrorSource>,
    pub component: Option<ComponentInfo>,
    pub page: Option<TemplateInfo>,
    pub route: Option<String>,
}

impl DetailedError {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ErrorSource {
    Python(PythonError),
    Template(TemplateInfo),
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ComponentInfo {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TemplateInfo {
    pub name: String,
    pub line: usize,
    pub source: Option<String>,
    pub source_code: Option<String>,
    pub detail: String,
    pub traceback: Option<String>,
}

impl Default for DetailedError {
    fn default() -> Self {
        Self {
            message: "".to_string(),
            file_path: "".to_string(),
            line: 0,
            column: 0,
            end_line: None,
            end_column: None,
            error_source: None,
            component: None,
            page: None,
            route: None,
        }
    }
}

impl std::fmt::Display for DetailedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for DetailedError {}

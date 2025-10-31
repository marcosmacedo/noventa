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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detailed_error_to_json() {
        let error = DetailedError {
            message: "Test error".to_string(),
            file_path: "test.rs".to_string(),
            line: 10,
            column: 5,
            end_line: Some(10),
            end_column: Some(15),
            error_source: None,
            component: None,
            page: None,
            route: Some("/test".to_string()),
        };
        let json = error.to_json();
        assert!(json.contains("Test error"));
        assert!(json.contains("test.rs"));
    }

    #[test]
    fn test_detailed_error_default() {
        let error = DetailedError::default();
        assert_eq!(error.message, "");
        assert_eq!(error.file_path, "");
        assert_eq!(error.line, 0);
        assert_eq!(error.column, 0);
        assert!(error.end_line.is_none());
        assert!(error.end_column.is_none());
        assert!(error.error_source.is_none());
        assert!(error.component.is_none());
        assert!(error.page.is_none());
        assert!(error.route.is_none());
    }

    #[test]
    fn test_detailed_error_display() {
        let error = DetailedError {
            message: "Display test".to_string(),
            ..Default::default()
        };
        assert_eq!(format!("{}", error), "Display test");
    }

    #[test]
    fn test_error_source_python() {
        let python_error = PythonError {
            message: "Python error".to_string(),
            traceback: "trace".to_string(),
            line_number: Some(1),
            column_number: Some(0),
            end_line_number: Some(1),
            end_column_number: Some(10),
            filename: Some("test.py".to_string()),
            source_code: Some("code".to_string()),
        };
        let source = ErrorSource::Python(python_error);
        // Just test creation, since it's an enum
        match source {
            ErrorSource::Python(_) => {},
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_error_source_template() {
        let template_info = TemplateInfo {
            name: "test.html".to_string(),
            line: 5,
            source: Some("source".to_string()),
            source_code: Some("code".to_string()),
            detail: "detail".to_string(),
            traceback: Some("trace".to_string()),
        };
        let source = ErrorSource::Template(template_info);
        match source {
            ErrorSource::Template(_) => {}
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_component_info_default() {
        let info = ComponentInfo::default();
        assert_eq!(info.name, "");
    }

    #[test]
    fn test_template_info_default() {
        let info = TemplateInfo::default();
        assert_eq!(info.name, "");
        assert_eq!(info.line, 0);
        assert!(info.source.is_none());
        assert!(info.source_code.is_none());
        assert_eq!(info.detail, "");
        assert!(info.traceback.is_none());
    }
}

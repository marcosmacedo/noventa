use crate::actors::interpreter::PythonError;
use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde::Serialize;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use once_cell::sync::Lazy;

// Global runtime flag set from `main.rs` so error rendering can respect the
// application's dev mode without reading environment variables here.
static DEV_MODE: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

/// Set the global dev-mode flag. Call this from `main.rs` with the runtime
/// `dev_mode` value so template rendering and error pages can behave
/// consistently across the application.
pub fn set_dev_mode(value: bool) {
    DEV_MODE.store(value, Ordering::SeqCst);
}

#[derive(Debug, Serialize, Clone)]
pub struct TemplateInfo {
    pub name: String,
    pub line: usize,
    pub source: Option<String>,
    pub detail: String,
    pub traceback: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ComponentInfo {
    pub name: String,
}

#[derive(Debug, Serialize, Clone)]
pub enum ErrorSource {
    Python(PythonError),
    Template(TemplateInfo),
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct DetailedError {
    pub route: Option<String>,
    pub page: Option<TemplateInfo>,
    pub component: Option<ComponentInfo>,
    pub error_source: Option<ErrorSource>,
}

impl DetailedError {
    pub fn flatten(&self) -> Vec<ErrorSource> {
        let mut sources = vec![];
        if let Some(error_source) = &self.error_source {
            sources.push(error_source.clone());
        }
        sources
    }
}
impl fmt::Display for DetailedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Create a simple string representation for the detailed error.
        // This can be improved to be more descriptive.
        write!(f, "A detailed error occurred.")
    }
}

impl std::error::Error for DetailedError {}







impl ResponseError for DetailedError {
    fn status_code(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }

    fn error_response(&self) -> HttpResponse {
        let dev_mode = DEV_MODE.load(Ordering::SeqCst);

        if dev_mode {
            let final_html = crate::templates::render_structured_debug_error(self);
            return HttpResponse::build(self.status_code())
                .content_type("text/html")
                .body(final_html);
        }

        HttpResponse::build(self.status_code())
            .content_type("text/html")
            .body("<h1>Internal Server Error</h1>")
    }
}




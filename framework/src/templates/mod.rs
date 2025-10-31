use crate::errors::{DetailedError, ERROR_CHANNEL};
use minijinja::Environment;
use once_cell::sync::Lazy;

static DEBUG_ERROR_TEMPLATE: &str = include_str!("debug_error.html");

static JINJA_ENV: Lazy<Environment<'static>> = Lazy::new(|| {
    let mut env = Environment::new();
    env.add_template("debug_error.html", DEBUG_ERROR_TEMPLATE)
        .unwrap();
    env
});

pub fn render_structured_debug_error(detailed_error: &DetailedError) -> String {
    log_detailed_error(detailed_error);
    let tmpl = JINJA_ENV.get_template("debug_error.html").unwrap();

    let mut context = std::collections::HashMap::new();
    context.insert("error", minijinja::Value::from_serialize(detailed_error));

    if let Some(error_source) = &detailed_error.error_source {

        let (source_code, line_number) = match error_source {
            crate::errors::ErrorSource::Python(py_err) => (py_err.source_code.as_ref(), py_err.line_number),
            crate::errors::ErrorSource::Template(tmpl_err) => (tmpl_err.source_code.as_ref(), Some(tmpl_err.line)),
        };

        if let (Some(code), Some(line_num)) = (source_code, line_number) {
            let lines: Vec<_> = code.lines().collect();
            let start_line = (line_num as isize - 7).max(0) as usize;
            
            let numbered_lines: Vec<_> = lines.iter().enumerate().map(|(i, line)| {
                let num = start_line + i + 1;
                let is_highlighted = num == line_num;
                minijinja::context! {
                    number => num,
                    content => line,
                    highlight => is_highlighted,
                }
            }).collect();
            context.insert("code_snippet", minijinja::Value::from(numbered_lines));
        }
    }

    let mut rendered = tmpl
        .render(minijinja::Value::from(context))
        .unwrap_or_else(|e| {
            log::error!("Failed to render structured debug error page: {}", e);
            "<h1>Internal Server Error</h1><p>Could not render the debug error page.</p>".to_string()
        });

    // Common marker and script injection logic
    add_marker_and_scripts(&mut rendered);
    rendered
}
pub fn render_production_error(detailed_error: &DetailedError) -> String {
    log_production_error(detailed_error);
    "<h1>Internal Server Error</h1><p>An unexpected error occurred.</p>".to_string()
}


pub fn log_production_error(detailed_error: &DetailedError) {
    log::error!("An error occurred on route: {}", detailed_error.route.as_deref().unwrap_or("unknown"));
    if let Some(error_source) = &detailed_error.error_source {
        match error_source {
            crate::errors::ErrorSource::Python(py_err) => {
                log::error!("Type: Python Error");
                log::error!("Message: {}", py_err.message);
                log::error!("File: {}", detailed_error.file_path);
            }
            crate::errors::ErrorSource::Template(tmpl_err) => {
                log::error!("Type: Template Error");
                log::error!("Message: {}", tmpl_err.detail);
                log::error!("File: {}", tmpl_err.name);
            }
        }
    }
}
pub fn log_detailed_error(detailed_error: &DetailedError) {
    let mut error_clone = detailed_error.clone();

    let normalized_path = std::fs::canonicalize(&error_clone.file_path)
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
        .unwrap_or(error_clone.file_path.clone());

    error_clone.file_path = normalized_path;

    if let Err(e) = ERROR_CHANNEL.send(error_clone.to_json()) {
        log::error!("Failed to send error to ERROR_CHANNEL: {}", e);
    }

    const RED: &str = "\x1b[31m";
    const RESET: &str = "\x1b[0m";

    log::error!("{}[ERROR]{}", RED, RESET);
    if let Some(route) = &error_clone.route {
        log::error!("{}  Page: {}{}", RED, route, RESET);
    }
    if let Some(template) = &error_clone.page {
        log::error!("{}  Template: {}{}", RED, template.name, RESET);
    }
    if let Some(component) = &error_clone.component {
        log::error!("{}  Component: {}{}", RED, component.name, RESET);
    }

    if let Some(error_source) = &error_clone.error_source {
        match error_source {
            crate::errors::ErrorSource::Python(py_err) => {
                log::error!("{}  Error: {}{}", RED, py_err.message, RESET);
                log::error!("{}  File: {} @ line {}{}", RED, error_clone.file_path, py_err.line_number.unwrap_or(0), RESET);
                log::error!("");
                log::error!("{}  Code:{}", RED, RESET);

                if let (Some(code), Some(line_num)) = (py_err.source_code.as_ref(), py_err.line_number) {
                    let lines: Vec<_> = code.lines().collect();
                    let start_line = (line_num as isize - 3).max(0) as usize;
                    let end_line = (line_num + 3).min(lines.len());

                    for i in start_line..end_line {
                        let line = lines[i];
                        let num = i + 1;
                        let is_highlighted = num == line_num;
                        if is_highlighted {
                            log::error!("{}   > {:>4} | {}{}", RED, num, line, RESET);
                        } else {
                            log::error!("{}     {:>4} | {}{}", RED, num, line, RESET);
                        }
                    }
                }

                log::error!("");
                log::error!("{}  Traceback:{}", RED, RESET);
                for line in py_err.traceback.lines() {
                    log::error!("{}  {}{}", RED, line, RESET);
                }
            }
            crate::errors::ErrorSource::Template(tmpl_err) => {
                log::error!("{}  Error: {}{}", RED, tmpl_err.detail, RESET);
                log::error!("{}  File: {} @ line {}{}", RED, tmpl_err.name, tmpl_err.line, RESET);
                log::error!("");
                log::error!("{}  Code:{}", RED, RESET);

                if let Some(code) = &tmpl_err.source_code {
                    let lines: Vec<_> = code.lines().collect();
                    let line_num = tmpl_err.line;
                    let start_line = (line_num as isize - 3).max(0) as usize;
                    let end_line = (line_num + 3).min(lines.len());

                    for i in start_line..end_line {
                        let line = lines[i];
                        let num = i + 1;
                        let is_highlighted = num == line_num;
                        if is_highlighted {
                            log::error!("{}   > {:>4} | {}{}", RED, num, line, RESET);
                        } else {
                            log::error!("{}     {:>4} | {}{}", RED, num, line, RESET);
                        }
                    }
                }
                if let Some(traceback) = &tmpl_err.traceback {
                    log::error!("");
                    log::error!("{}  Traceback:{}", RED, RESET);
                    for line in traceback.lines() {
                        log::error!("{}  {}{}", RED, line, RESET);
                    }
                }
            }
        }
    }
}


fn add_marker_and_scripts(rendered: &mut String) {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let marker = format!("<!-- debug_rendered: {} -->", timestamp);

    if let Some(body_end_pos) = rendered.rfind("</body>") {
        let morphdom_script = format!("<script>{}</script>\n", include_str!("../scripts/morphdom-2.6.1-umd.min.js"));
        let devws_script = format!("<script>{}</script>", include_str!("../scripts/devws.js"));
        rendered.insert_str(body_end_pos, &format!("\n{}\n{}\n", morphdom_script, devws_script));
    } else {
        rendered.push_str(&marker);
    }
}
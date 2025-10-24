use crate::errors::DetailedError;
use minijinja::{context, Environment};
use once_cell::sync::Lazy;

static DEBUG_ERROR_TEMPLATE: &str = include_str!("debug_error.html");

static JINJA_ENV: Lazy<Environment<'static>> = Lazy::new(|| {
    let mut env = Environment::new();
    env.add_template("debug_error.html", DEBUG_ERROR_TEMPLATE)
        .unwrap();
    env
});

pub fn render_structured_debug_error(detailed_error: &DetailedError) -> String {
    log::debug!("Rendering structured error: {:?}", detailed_error);
    let tmpl = JINJA_ENV.get_template("debug_error.html").unwrap();
    let error_chain = detailed_error.flatten();
    let mut rendered = tmpl
        .render(context! {
            error_chain => error_chain,
            error => detailed_error,
        })
        .unwrap_or_else(|e| {
            log::error!("Failed to render structured debug error page: {}", e);
            "<h1>Internal Server Error</h1><p>Could not render the debug error page.</p>".to_string()
        });

    // Common marker and script injection logic
    add_marker_and_scripts(&mut rendered);
    rendered
}

// pub fn render_debug_error(
//     error_message: &str,
//     traceback: &str,
//     filename: &str,
//     line_number: usize,
// ) -> String {
//     log::debug!("render_debug_error called for {}:{}", filename, line_number);

//     let tmpl = JINJA_ENV.get_template("debug_error.html").unwrap();
//     let mut rendered = tmpl
//         .render(context! {
//             error_message => error_message,
//             traceback => traceback,
//             filename => filename,
//             line_number => line_number,
//         })
//         .unwrap_or_else(|e| {
//             log::error!("Failed to render debug error page: {}", e);
//             "<h1>Internal Server Error</h1><p>Could not render the debug error page.</p>"
//                 .to_string()
//         });

//     add_marker_and_scripts(&mut rendered);
//     rendered
// }

fn add_marker_and_scripts(rendered: &mut String) {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let marker = format!("<!-- debug_rendered: {} -->", timestamp);

    if let Some(body_end_pos) = rendered.rfind("</body>") {
        let devws_script =
            format!("<script>{}</script>", include_str!("../scripts/devws.js"));
        rendered.insert_str(body_end_pos, &format!("\n{}\n{}\n", devws_script, marker));
    } else {
        rendered.push_str(&marker);
    }
}
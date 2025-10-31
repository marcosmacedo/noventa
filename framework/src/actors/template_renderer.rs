use crate::actors::health::{HealthActor, ReportTemplateLatency, ReportPythonLatency};
use crate::actors::interpreter::{ExecuteFunction, PythonInterpreterActor};
use crate::actors::page_renderer::{HttpRequestInfo, RenderOutput};
use crate::actors::session_manager::SessionManagerActor;
use crate::components::Component;
use crate::config;
use crate::errors::{ComponentInfo, DetailedError, ErrorSource};
use actix::prelude::*;
use minijinja::{Environment, State, value::Kwargs, Value};
use regex::Regex;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, RwLock};

static FORM_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(<form[^>]*>)").unwrap());
static COMPONENT_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{\{\s*component\s*\(([^)]+)\)\s*\}\}").unwrap());
static EXTENDS_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r#"\{%\s*extends\s*"([^"]+)"\s*%\}
"#).unwrap());

// Actor for rendering templates
pub struct TemplateRendererActor {
    env: Arc<Environment<'static>>,
    interpreter: Addr<PythonInterpreterActor>,
    health_actor: Addr<HealthActor>,
    dev_mode: bool,
    components: Arc<RwLock<Vec<Component>>>,
}

#[derive(Debug, Clone)]
struct ComponentCall {
    name: String,
    kwargs: HashMap<String, Value>,
}

impl TemplateRendererActor {
    pub fn new(
        interpreter: Addr<PythonInterpreterActor>,
        health_actor: Addr<HealthActor>,
        dev_mode: bool,
        components: Vec<Component>,
    ) -> Self {
        let mut env = Environment::new();
        minijinja_contrib::add_to_environment(&mut env);
        env.set_loader(minijinja::path_loader(config::BASE_PATH.to_str().unwrap()));

        Self {
            env: Arc::new(env),
            interpreter,
            health_actor,
            dev_mode,
            components: Arc::new(RwLock::new(components)),
        }
    }

    fn handle_post_request(&mut self, msg: RenderTemplate) -> Result<RenderOutput, DetailedError> {
        // Phase 1: Scan - Recursively find all `component()` calls in the templates
        // to build a complete blueprint of the page's component tree.
        let mut component_calls = Vec::new();
        let template = self.env.get_template(&msg.template_name).map_err(|e| DetailedError {
            page: Some(crate::errors::TemplateInfo {
                name: msg.template_name.clone(),
                line: e.line().unwrap_or(0),
                source: None,
                source_code: None,
                detail: e.detail().unwrap_or("").to_string(),
                traceback: Some(format!("{:?}", e)),
            }),
            file_path: msg.template_name.clone(),
            ..Default::default()
        })?;
        self.recursive_scan(&msg.template_name, template.source(), &mut component_calls).map_err(|e| DetailedError {
            page: Some(crate::errors::TemplateInfo {
                name: msg.template_name.clone(),
                line: 0,
                source: None,
                source_code: None,
                detail: e.to_string(),
                traceback: Some(format!("{:?}", e)),
            }),
            file_path: msg.template_name.clone(),
            ..Default::default()
        })?;

        // Extract form data to identify which component action was triggered.
        let form_data: HashMap<String, String> =
            serde_json::from_value(serde_json::Value::Object(msg.request_info.form_data.clone())).map_err(|e| DetailedError {
                message: format!("Failed to parse form data: {}", e),
                ..Default::default()
            })?;
        let form_component_id = form_data.get("component_id").cloned().unwrap_or_default();
        let action = form_data.get("action").cloned().unwrap_or_default();

        log::debug!("Handling POST request for component '{}', action '{}'", form_component_id, action);

    // Phase 2: Act & Cache - Execute the action handler for the target component *before* rendering.
        // The unique context returned by the action is cached to be used in the final render.
        let mut action_context = None;

        log::debug!("--- Debugging POST Request ---");
        log::debug!("Form Component ID: '{}'", form_component_id);
        log::debug!("Component Calls Found:");
        for call in &component_calls {
            log::debug!("  - Name: {}, Kwargs: {:?}", call.name, call.kwargs);
        }

        let found_component = component_calls.iter().find(|c| c.name == form_component_id);

        if let Some(action_component_call) = found_component {
            log::debug!("Successfully found component to handle action: '{}'", action_component_call.name);
            if !action.is_empty() {
                let mut form_data_value = HashMap::new();
                for (k, v) in form_data {
                    form_data_value.insert(k.clone(), Value::from(v.clone()));
                }

                let mut kwargs_map_post = action_component_call.kwargs.clone();
                kwargs_map_post.extend(form_data_value);

                let components = self.components.read().map_err(|_| DetailedError {
                    message: "Component lock is poisoned".to_string(),
                    ..Default::default()
                })?;
                let component = components.iter().find(|c| c.id == action_component_call.name).ok_or_else(|| DetailedError {
                    message: format!("Component '{}' not found", action_component_call.name),
                    ..Default::default()
                })?;
                if let Some(logic_path) = &component.logic_path {
                    let module_path = path_to_module(logic_path).map_err(|e| DetailedError {
                        message: format!("Invalid module path: {}", e),
                        ..Default::default()
                    })?;

                    let execute_fn_msg = ExecuteFunction {
                        module_path,
                        function_name: format!("action_{}", action),
                        request: msg.request_info.clone(),
                        args: Some(kwargs_map_post),
                        session_manager: msg.session_manager.clone(),
                    };

                    let result = futures::executor::block_on(self.interpreter.send(execute_fn_msg));
                    match result {
                        Ok(Ok(result)) => {
                            if let Ok(redirect_url) = result.context.get_attr("_redirect") {
                                if !redirect_url.is_undefined() && !redirect_url.is_none() {
                                    if let Some(url_str) = redirect_url.as_str() {
                                        return Ok(RenderOutput::Redirect(url_str.to_string()));
                                    }
                                }
                            }
                            action_context = Some(result.context);
                        }
                        Ok(Err(py_err)) => {
                            return Err(DetailedError {
                                component: Some(ComponentInfo {
                                    name: action_component_call.name.clone(),
                                }),
                                error_source: Some(ErrorSource::Python(py_err.clone())),
                                message: py_err.message.clone(),
                                file_path: py_err.filename.clone().unwrap_or_default(),
                                line: py_err.line_number.unwrap_or(0) as u32,
                                column: py_err.column_number.unwrap_or(0) as u32,
                                end_line: py_err.end_line_number.map(|l| l as u32),
                                end_column: py_err.end_column_number.map(|c| c as u32),
                                ..Default::default()
                            });
                        }
                        Err(e) => {
                            log::error!("A mailbox error occurred: {}. This might indicate a problem with the server's internal communication.", e);
                            return Err(DetailedError {
                                error_source: Some(ErrorSource::Python(
                                    crate::actors::interpreter::PythonError {
                                        message: e.to_string(),
                                        traceback: format!("{:?}", e),
                                        line_number: None,
                                        column_number: None,
                                        end_line_number: None,
                                        end_column_number: None,
                                        filename: None,
                                        source_code: None,
                                    },
                                )),
                                ..Default::default()
                            });
                        }
                    }
                }
            }else{
                return Err(DetailedError {
                    component: Some(ComponentInfo {
                        name: action_component_call.name.clone(),
                    }),
                    error_source: Some(ErrorSource::Template(crate::errors::TemplateInfo {
                        name: msg.template_name.clone(),
                        ..Default::default()
                    })),
                    message: "This component requires an action to be specified in the template".to_string(),
                    file_path: msg.template_name.clone(),
                    ..Default::default()
                });
            }
        }else {
            return Err(DetailedError {
                error_source: Some(ErrorSource::Template(crate::errors::TemplateInfo {
                    name: msg.template_name.clone(),
                    ..Default::default()
                })),
                message: "No component found for the given component_id in the POST data".to_string(),
                file_path: msg.template_name.clone(),
                ..Default::default()
            });
        }

        // Phase 3: Render - Render the full page.
        let mut env = (*self.env).clone();
        if self.dev_mode {
            env.set_loader(minijinja::path_loader("."));
        }

    let interpreter_clone = self.interpreter.clone();
        let health_actor_clone = self.health_actor.clone();
        let request_info_clone = msg.request_info.clone();
        let session_manager_clone = msg.session_manager.clone();
        let components_clone = Arc::clone(&self.components);
        let action_context = Arc::new(action_context);
        let form_component_id = form_component_id.clone();

        env.add_function(
            "component",
            move |state: &State, name: String, kwargs: Kwargs| -> Result<Value, minijinja::Error> {
                let name = name.replace(".", "/");
                let kwargs_map: HashMap<String, Value> = kwargs
                    .args()
                    .filter_map(|k| kwargs.get::<Value>(k).ok().map(|v| (k.to_string(), v)))
                    .collect();

                let components = components_clone.read().unwrap();
                let component = components.iter().find(|c| c.id == name).unwrap();
                let context_result = if let Some(logic_path) = &component.logic_path {
                    let module_path = path_to_module(logic_path).unwrap();
                    let execute_fn_msg = ExecuteFunction {
                        module_path,
                        function_name: "load_template_context".to_string(),
                        request: request_info_clone.clone(),
                        args: Some(kwargs_map),
                        session_manager: session_manager_clone.clone(),
                    };

                    let python_start_time = std::time::Instant::now();
                    let future = interpreter_clone.send(execute_fn_msg);
                    let result = futures::executor::block_on(future);
                    let python_duration_ms = python_start_time.elapsed().as_secs_f64() * 1000.0;
                    health_actor_clone.do_send(ReportPythonLatency(python_duration_ms));

                    match result {
                        Ok(Ok(res)) => Ok(res.context),
                        Ok(Err(py_err)) => {
                            let detailed_error = DetailedError {
                                component: Some(ComponentInfo { name: name.clone() }),
                                error_source: Some(ErrorSource::Python(py_err.clone())),
                                message: py_err.message.clone(),
                                file_path: py_err.filename.clone().unwrap_or_default(),
                                line: py_err.line_number.unwrap_or(0) as u32,
                                column: py_err.column_number.unwrap_or(0) as u32,
                                end_line: py_err.end_line_number.map(|l| l as u32),
                                end_column: py_err.end_column_number.map(|c| c as u32),
                                ..Default::default()
                            };
                            let err = minijinja::Error::new(
                                minijinja::ErrorKind::InvalidOperation,
                                "Python function crashed",
                            );
                            Err(err.with_source(detailed_error))
                        }
                        Err(e) => {
                            log::error!("A mailbox error occurred: {}. This might indicate a problem with the server's internal communication.", e);
                            Err(minijinja::Error::new(minijinja::ErrorKind::InvalidOperation, "Mailbox error").with_source(e))
                        }
                    }
                } else {
                    // If there's no logic_path, there's no context to load.
                    Ok(Value::from_serialize(serde_json::json!({})))
                };

                match context_result {
                    Ok(context) => {
                        let mut final_context = context;
                        // If this is the component that handled the POST request, merge the action context.
                        if name == form_component_id {
                            if let Some(action_ctx) = action_context.as_ref().as_ref() {
                                let get_ctx_result = serde_json::to_value(&final_context);
                                let action_ctx_result = serde_json::to_value(action_ctx);

                                let mut get_ctx_map: serde_json::Value = match get_ctx_result {
                                    Ok(val) => val,
                                    Err(e) => return Err(minijinja::Error::new(minijinja::ErrorKind::InvalidOperation, "Failed to serialize context").with_source(e)),
                                };

                                let action_ctx_map: serde_json::Value = match action_ctx_result {
                                    Ok(val) => val,
                                    Err(e) => return Err(minijinja::Error::new(minijinja::ErrorKind::InvalidOperation, "Failed to serialize action context").with_source(e)),
                                };

                                if let (Some(get_map), Some(action_map)) = (get_ctx_map.as_object_mut(), action_ctx_map.as_object()) {
                                    for (k, v) in action_map.iter() {
                                        get_map.insert(k.clone(), v.clone());
                                    }
                                }
                                final_context = Value::from_serialize(get_ctx_map);
                            }
                        }

                        let components = components_clone.read().unwrap();
                        let component = components.iter().find(|c| c.id == name).ok_or_else(|| {
                            minijinja::Error::new(minijinja::ErrorKind::TemplateNotFound, "Component not found")
                        })?;
                        let mut template_path = component.template_path.clone();
                        if template_path.starts_with("./") {
                            template_path = template_path[2..].to_string();
                        }
                        let tmpl = state.env().get_template(&template_path)?;
                        let mut result = tmpl.render(final_context)?;

                        let re = Regex::new(r"(<form[^>]*>)").unwrap();
                        let replacement = format!(r#"$1<input type="hidden" name="component_id" value="{}">"#, name);
                        result = re.replace_all(&result, replacement).to_string();

                        Ok(Value::from_safe_string(result))
                    }
                    Err(e) => Err(e),
                }
            },
        );

        let rendered_page = self.render_page(&env, &msg.template_name).map_err(|e| {
            if let Some(detailed_error) = e.source().and_then(|s| s.downcast_ref::<DetailedError>()) {
                return detailed_error.clone();
            }
            let template_info = crate::errors::TemplateInfo {
                name: e.name().unwrap_or(&msg.template_name).to_string(),
                line: e.line().unwrap_or(0),
                source: None,
                source_code: {
                    let filename = e.name().unwrap_or(&msg.template_name);
                    if let Ok(contents) = std::fs::read_to_string(filename) {
                        if let Some(ln) = e.line() {
                            let start = (ln as isize - 7).max(0) as usize;
                            let end = (ln + 6).min(contents.lines().count());
                            Some(contents.lines().skip(start).take(end - start).collect::<Vec<_>>().join("\n"))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                detail: e.detail().unwrap_or("").to_string(),
                traceback: Some(format!("{:?}", e)),
            };
            DetailedError {
                page: Some(template_info.clone()),
                error_source: Some(ErrorSource::Template(template_info.clone())),
                file_path: e.name().unwrap_or(&msg.template_name).to_string(),
                line: template_info.line as u32,
                ..Default::default()
            }
        })?;
        Ok(RenderOutput::Html(rendered_page))
    }

    // Recursively scans template files to find all `{{ component(...) }}` calls.
    // This builds a complete tree of all components on a page and their arguments,
    // without executing any of them.
    fn recursive_scan(&self, template_name: &str, template_content: &str, calls: &mut Vec<ComponentCall>) -> Result<(), minijinja::Error> {
        log::debug!("Scanning template: {}", template_name);

        // First, check for an `extends` tag and scan the parent template.
        if let Some(caps) = EXTENDS_REGEX.captures(template_content) {
            if let Some(parent_template_name) = caps.get(1) {
                let parent_name = parent_template_name.as_str();
                log::debug!("Found extends tag, scanning parent: {}", parent_name);
                let parent_template = self.env.get_template(parent_name)?;
                self.recursive_scan(parent_name, parent_template.source(), calls)?;
            }
        }

        // Now, scan the current template for component calls.
        for cap in COMPONENT_REGEX.captures_iter(template_content) {
            let args_str = &cap[1];
            // Manual parsing of arguments from the template string.
            let mut parts = args_str.split(',');
            let name = parts.next().unwrap_or("").trim().replace("'", "").replace("\"", "");
            let name = name.replace(".", "/");
            let mut kwargs_map = HashMap::new();
            for part in parts {
                let mut kv = part.splitn(2, '=');
                if let (Some(key), Some(val)) = (kv.next(), kv.next()) {
                    let key = key.trim().to_string();
                    let val_str = val.trim().to_string();
                    // This is a simplification; it doesn't handle complex values like variables.
                    // For now, we'll assume string literals.
                    let value = Value::from(val_str.replace("'", "").replace("\"", ""));
                    kwargs_map.insert(key, value);
                }
            }

            let components = self.components.read().unwrap();
            let component = components.iter().find(|c| c.id == name).ok_or_else(|| {
                minijinja::Error::new(minijinja::ErrorKind::TemplateNotFound, "Component not found")
            })?;

            // Recurse into the component's own template to find nested components.
            self.recursive_scan(&component.id, &component.template_content, calls)?;
            calls.push(ComponentCall { name, kwargs: kwargs_map });
        }

        Ok(())
    }

    fn render_page(&self, env: &Environment, template_name: &str) -> Result<String, minijinja::Error> {
        let tmpl = env.get_template(template_name)?;
        let start_time = std::time::Instant::now();
        let mut result = tmpl.render(minijinja::context! {})?;
        let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;
        self.health_actor.do_send(ReportTemplateLatency(duration_ms));

        if let Some(body_end_pos) = result.rfind("</body>") {
            let mut scripts = String::new();
            scripts.push_str(&format!("<script>{}</script>\n", include_str!("../scripts/idiomorph.min.js")));
            scripts.push_str(&format!("<script>{}</script>\n", include_str!("../scripts/frontend.js")));
            if self.dev_mode {
                scripts.push_str(&format!("<script>{}</script>\n", include_str!("../scripts/devws.js")));
            }
            result.insert_str(body_end_pos, &scripts);
        }

        Ok(result)
    }

}

impl Actor for TemplateRendererActor {
    type Context = SyncContext<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {}
}

// Message for rendering a template
#[derive(Message)]
#[rtype(result = "Result<RenderOutput, DetailedError>")]
pub struct RenderTemplate {
    pub template_name: String,
    pub request_info: Arc<HttpRequestInfo>,
    pub session_manager: Addr<SessionManagerActor>,
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct UpdateComponents(pub Vec<Component>);


impl Handler<RenderTemplate> for TemplateRendererActor {
    type Result = Result<RenderOutput, DetailedError>;

    fn handle(&mut self, msg: RenderTemplate, _ctx: &mut Self::Context) -> Self::Result {
        if msg.request_info.method == "POST" {
            return self.handle_post_request(msg);
        }

        let mut env = (*self.env).clone();
        if self.dev_mode {
            env.set_loader(minijinja::path_loader("."));
        }

        let interpreter_clone = self.interpreter.clone();
        let health_actor_clone = self.health_actor.clone();
        let request_info_clone = msg.request_info.clone();
        let session_manager_clone = msg.session_manager.clone();
        let components_clone = Arc::clone(&self.components);

        env.add_function(
            "component",
            move |state: &State, name: String, kwargs: Kwargs| -> Result<Value, minijinja::Error> {
                let name = name.replace(".", "/");
                let kwargs_map: HashMap<String, Value> = kwargs
                    .args()
                    .filter_map(|k| kwargs.get::<Value>(k).ok().map(|v| (k.to_string(), v)))
                    .collect();

                let components = components_clone.read().unwrap();
                let component = components.iter().find(|c| c.id == name).ok_or_else(|| {
                    minijinja::Error::new(minijinja::ErrorKind::TemplateNotFound, "Component not found")
                })?;
                if let Some(logic_path) = &component.logic_path {
                    let module_path = path_to_module(logic_path).unwrap();
                    let execute_fn_msg = ExecuteFunction {
                        module_path,
                        function_name: "load_template_context".to_string(),
                        request: request_info_clone.clone(),
                        args: Some(kwargs_map),
                        session_manager: session_manager_clone.clone(),
                    };

                    let python_start_time = std::time::Instant::now();
                    let future = interpreter_clone.send(execute_fn_msg);
                    let result = futures::executor::block_on(future);
                    let python_duration_ms = python_start_time.elapsed().as_secs_f64() * 1000.0;
                    health_actor_clone.do_send(ReportPythonLatency(python_duration_ms));

                    match result {
                        Ok(Ok(result)) => {
                            if let Ok(redirect_url) = result.context.get_attr("_redirect") {
                                if !redirect_url.is_undefined() && !redirect_url.is_none() {
                                    if let Some(url_str) = redirect_url.as_str() {
                                        let redirect_marker = format!("<!-- REDIRECT:{} -->", url_str);
                                        return Ok(Value::from_safe_string(redirect_marker));
                                    }
                                }
                            }
                            let components = components_clone.read().unwrap();
                            let component =
                                components.iter().find(|c| c.id == name).ok_or_else(|| {
                                    minijinja::Error::new(
                                        minijinja::ErrorKind::TemplateNotFound,
                                        "Component not found",
                                    )
                                })?;
                            let mut template_path = component.template_path.clone();
                            if template_path.starts_with("./") {
                                template_path = template_path[2..].to_string();
                            }
                            let tmpl = state.env().get_template(&template_path)?;
                            let mut rendered_component = tmpl.render(result.context)?;

                            let replacement = format!(
                                r#"$1<input type="hidden" name="component_id" value="{}">"#,
                                name
                            );
                            rendered_component = FORM_REGEX
                                .replace_all(&rendered_component, replacement)
                                .to_string();

                            Ok(Value::from_safe_string(rendered_component))
                        }
                        Ok(Err(py_err)) => {
                            let detailed_error = DetailedError {
                                component: Some(ComponentInfo { name: name.clone() }),
                                error_source: Some(ErrorSource::Python(py_err.clone())),
                                message: py_err.message.clone(),
                                file_path: py_err.filename.clone().unwrap_or_default(),
                                line: py_err.line_number.unwrap_or(0) as u32,
                                column: py_err.column_number.unwrap_or(0) as u32,
                                end_line: py_err.end_line_number.map(|l| l as u32),
                                end_column: py_err.end_column_number.map(|c| c as u32),
                                ..Default::default()
                            };
                            let err = minijinja::Error::new(
                                minijinja::ErrorKind::InvalidOperation,
                                "Python function crashed",
                            );
                            Err(err.with_source(detailed_error))
                        }
                        Err(e) => {
                            log::error!("A mailbox error occurred: {}. This might indicate a problem with the server's internal communication.", e);
                            Err(minijinja::Error::new(
                                minijinja::ErrorKind::InvalidOperation,
                                "Mailbox error",
                            )
                            .with_source(e))
                        }
                    }
                } else {
                    // If there's no logic_path, just render the template without context.
                    let components = components_clone.read().unwrap();
                    let component =
                        components.iter().find(|c| c.id == name).ok_or_else(|| {
                            minijinja::Error::new(
                                minijinja::ErrorKind::TemplateNotFound,
                                "Component not found",
                            )
                        })?;
                    let mut template_path = component.template_path.clone();
                    if template_path.starts_with("./") {
                        template_path = template_path[2..].to_string();
                    }
                    let tmpl = state.env().get_template(&template_path)?;
                    let mut rendered_component =
                        tmpl.render(Value::from_serialize(serde_json::json!({})))?;

                    let replacement = format!(
                        r#"$1<input type="hidden" name="component_id" value="{}">"#,
                        name
                    );
                    rendered_component = FORM_REGEX
                        .replace_all(&rendered_component, replacement)
                        .to_string();

                    Ok(Value::from_safe_string(rendered_component))
                }
            },
        );

        let rendered_page = self.render_page(&env, &msg.template_name).map_err(|e| {
            if let Some(detailed_error) = e.source().and_then(|s| s.downcast_ref::<DetailedError>()) {
                return detailed_error.clone();
            }
            let template_info = crate::errors::TemplateInfo {
                name: e.name().unwrap_or(&msg.template_name).to_string(),
                line: e.line().unwrap_or(0),
                source: None,
                source_code: {
                    let filename = e.name().unwrap_or(&msg.template_name);
                    if let Ok(contents) = std::fs::read_to_string(filename) {
                        if let Some(ln) = e.line() {
                            let start = (ln as isize - 7).max(0) as usize;
                            let end = (ln + 6).min(contents.lines().count());
                            Some(contents.lines().skip(start).take(end - start).collect::<Vec<_>>().join("\n"))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                detail: e.detail().unwrap_or("").to_string(),
                traceback: Some(format!("{:?}", e)),
            };
            DetailedError {
                page: Some(template_info.clone()),
                error_source: Some(ErrorSource::Template(template_info.clone())),
                file_path: e.name().unwrap_or(&msg.template_name).to_string(),
                line: template_info.line as u32,
                ..Default::default()
            }
        })?;

        if rendered_page.contains("<!-- REDIRECT:") {
            if let Some(caps) = Regex::new(r"<!-- REDIRECT:(.*?) -->").unwrap().captures(&rendered_page) {
                if let Some(url) = caps.get(1) {
                    return Ok(RenderOutput::Redirect(url.as_str().to_string()));
                }
            }
        }

        Ok(RenderOutput::Html(rendered_page))
    }
}

impl Handler<UpdateComponents> for TemplateRendererActor {
    type Result = ();

    fn handle(&mut self, msg: UpdateComponents, _ctx: &mut Self::Context) -> Self::Result {
        let mut components = self.components.write().unwrap();
        *components = msg.0;
    }
}


fn path_to_module(path_str: &str) -> Result<String, std::io::Error> {
    let path = std::path::Path::new(path_str);

    // Clean the path to remove "./"
    let cleaned_path = path.strip_prefix("./").unwrap_or(path);

    // Convert to string and remove the .py extension
    let module_str = cleaned_path.to_str().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "Path contains invalid UTF-8"))?;
    let module_str_no_ext = module_str.strip_suffix(".py").unwrap_or(module_str);

    // Replace slashes with dots for Python import syntax
    let module_path = module_str_no_ext.replace("/", ".");

    Ok(module_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_to_module() {
        // Test basic conversion
        assert_eq!(path_to_module("utils.py").unwrap(), "utils");
        assert_eq!(path_to_module("path/to/module.py").unwrap(), "path.to.module");
        
        // Test with leading ./
        assert_eq!(path_to_module("./utils.py").unwrap(), "utils");
        assert_eq!(path_to_module("./path/to/module.py").unwrap(), "path.to.module");
        
        // Test without .py extension
        assert_eq!(path_to_module("utils").unwrap(), "utils");
        assert_eq!(path_to_module("path/to/module").unwrap(), "path.to.module");
        
        // Test edge cases
        assert_eq!(path_to_module("single").unwrap(), "single");
        assert_eq!(path_to_module("a/b/c.py").unwrap(), "a.b.c");
    }
}
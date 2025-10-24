use crate::actors::health::{HealthActor, ReportTemplateLatency, ReportPythonLatency};
use crate::actors::interpreter::{ExecuteFunction, PythonInterpreterActor};
use crate::actors::page_renderer::HttpRequestInfo;
use crate::actors::session_manager::SessionManagerActor;
use crate::components::Component;
use crate::errors::{DetailedError, ErrorSource, ComponentInfo};
use actix::prelude::*;
use std::error::Error;
use minijinja::{Environment, State, value::Kwargs, Value};
use std::sync::Arc;
use std::collections::HashMap;
use regex::Regex;
use once_cell::sync::Lazy;

static FORM_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(<form[^>]*>)").unwrap());
static COMPONENT_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{\{\s*component\s*\(([^)]+)\)\s*\}\}").unwrap());

// Actor for rendering templates
pub struct TemplateRendererActor {
    env: Arc<Environment<'static>>,
    interpreter: Addr<PythonInterpreterActor>,
    health_actor: Addr<HealthActor>,
    dev_mode: bool,
    components: Vec<Component>,
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
        env.set_loader(minijinja::path_loader("."));

        Self {
            env: Arc::new(env),
            interpreter,
            health_actor,
            dev_mode,
            components,
        }
    }

    fn handle_post_request(&mut self, msg: RenderTemplate) -> Result<String, DetailedError> {
        // Phase 1: Scan - Recursively find all `component()` calls in the templates
        // to build a complete blueprint of the page's component tree.
        let mut component_calls = Vec::new();
        let template = self.env.get_template(&msg.template_name).map_err(|e| DetailedError {
            page: Some(crate::errors::TemplateInfo {
                name: msg.template_name.clone(),
                line: e.line().unwrap_or(0),
                source: None,
                detail: e.detail().unwrap_or("").to_string(),
                traceback: Some(format!("{:?}", e)),
            }),
            ..Default::default()
        })?;
        self.recursive_scan(template.source(), &mut component_calls).map_err(|e| DetailedError {
            page: Some(crate::errors::TemplateInfo {
                name: msg.template_name.clone(),
                line: 0,
                source: None,
                detail: e.to_string(),
                traceback: Some(format!("{:?}", e)),
            }),
            ..Default::default()
        })?;

        // Extract form data to identify which component action was triggered.
        let form_data: HashMap<String, String> =
            serde_json::from_value(serde_json::Value::Object(msg.request_info.form_data.clone())).unwrap();
        let form_component_id = form_data.get("component_id").cloned().unwrap_or_default();
        let action = form_data.get("action").cloned().unwrap_or_default();

    // Phase 2: Act & Cache - Execute the action handler for the target component *before* rendering.
        // The unique context returned by the action is cached to be used in the final render.
        let mut action_context = None;
        if let Some(action_component_call) = component_calls.iter().find(|c| c.name == form_component_id) {
            if !action.is_empty() {
                let mut form_data_value = HashMap::new();
                for (k, v) in form_data {
                    form_data_value.insert(k.clone(), Value::from(v.clone()));
                }

                let mut kwargs_map_post = action_component_call.kwargs.clone();
                kwargs_map_post.extend(form_data_value);

                let component = self.components.iter().find(|c| c.id == action_component_call.name).unwrap();
                let module_path = path_to_module(component.logic_path.as_ref().unwrap()).unwrap();

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
                            action_context = Some(result.context);
                        }
                        Ok(Err(py_err)) => {
                            return Err(DetailedError {
                                component: Some(ComponentInfo {
                                    name: action_component_call.name.clone(),
                                }),
                                error_source: Some(ErrorSource::Python(py_err)),
                                ..Default::default()
                            });
                        }
                    Err(e) => {
                        log::error!("A mailbox error occurred: {}. This might indicate a problem with the server's internal communication.", e);
                        return Err(DetailedError {
                            error_source: Some(ErrorSource::Python(crate::actors::interpreter::PythonError {
                                message: e.to_string(),
                                traceback: format!("{:?}", e),
                                line_number: None,
                                filename: None,
                            })),
                            ..Default::default()
                        });
                    }
                }
            }
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
        let components_clone = self.components.clone();
        let action_context = Arc::new(action_context);

        env.add_function(
            "component",
            move |state: &State, name: String, kwargs: Kwargs| -> Result<Value, minijinja::Error> {
                let kwargs_map: HashMap<String, Value> = kwargs
                    .args()
                    .filter_map(|k| kwargs.get::<Value>(k).ok().map(|v| (k.to_string(), v)))
                    .collect();

                let context_result = if name == form_component_id {
                    Ok(action_context.as_ref().as_ref().unwrap().clone())
                } else {
                    let component = components_clone.iter().find(|c| c.id == name).unwrap();
                    let module_path = path_to_module(component.logic_path.as_ref().unwrap()).unwrap();

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
                                error_source: Some(ErrorSource::Python(py_err)),
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
                };

                match context_result {
                    Ok(context) => {
                        let component = components_clone.iter().find(|c| c.id == name).ok_or_else(|| {
                            minijinja::Error::new(minijinja::ErrorKind::TemplateNotFound, "Component not found")
                        })?;
                        let mut template_path = component.template_path.clone();
                        if template_path.starts_with("./") {
                            template_path = template_path[2..].to_string();
                        }
                        let tmpl = state.env().get_template(&template_path)?;
                        let mut result = tmpl.render(context)?;

                        let re = Regex::new(r"(<form[^>]*>)").unwrap();
                        let replacement = format!(r#"$1<input type="hidden" name="component_id" value="{}">"#, name);
                        result = re.replace_all(&result, replacement).to_string();

                        Ok(Value::from_safe_string(result))
                    }
                    Err(e) => Err(e),
                }
            },
        );

        self.render_page(&env, &msg.template_name).map_err(|e| {
            if let Some(detailed_error) = e.source().and_then(|s| s.downcast_ref::<DetailedError>()) {
                return detailed_error.clone();
            }
            let template_info = crate::errors::TemplateInfo {
                name: e.name().unwrap_or(&msg.template_name).to_string(),
                line: e.line().unwrap_or(0),
                source: None,
                detail: e.detail().unwrap_or("").to_string(),
                traceback: Some(format!("{:?}", e)),
            };
            DetailedError {
                page: Some(template_info.clone()),
                error_source: Some(ErrorSource::Template(template_info)),
                ..Default::default()
            }
        })
    }

    // Recursively scans template files to find all `{{ component(...) }}` calls.
    // This builds a complete tree of all components on a page and their arguments,
    // without executing any of them.
    fn recursive_scan(&self, template_content: &str, calls: &mut Vec<ComponentCall>) -> Result<(), minijinja::Error> {
        for cap in COMPONENT_REGEX.captures_iter(template_content) {
            let args_str = &cap[1];
            // Manual parsing of arguments from the template string.
            let mut parts = args_str.split(',');
            let name = parts.next().unwrap_or("").trim().replace("'", "").replace("\"", "");
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

            let component = self.components.iter().find(|c| c.id == name).ok_or_else(|| {
                minijinja::Error::new(minijinja::ErrorKind::TemplateNotFound, "Component not found")
            })?;

            // Recurse into the component's own template to find nested components.
            self.recursive_scan(&component.template_content, calls)?;
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
            scripts.push_str(&format!("<script>{}</script>\n", include_str!("../scripts/morphdom-2.6.1-umd.min.js")));
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
#[rtype(result = "Result<String, DetailedError>")]
pub struct RenderTemplate {
    pub template_name: String,
    pub request_info: Arc<HttpRequestInfo>,
    pub session_manager: Addr<SessionManagerActor>,
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct UpdateComponents(pub Vec<Component>);

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct UpdateSingleComponent(pub Component);

impl Handler<RenderTemplate> for TemplateRendererActor {
    type Result = Result<String, DetailedError>;

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
        let components_clone = self.components.clone();

        env.add_function(
            "component",
            move |state: &State, name: String, kwargs: Kwargs| -> Result<Value, minijinja::Error> {
                let kwargs_map: HashMap<String, Value> = kwargs
                    .args()
                    .filter_map(|k| kwargs.get::<Value>(k).ok().map(|v| (k.to_string(), v)))
                    .collect();

                let component = components_clone.iter().find(|c| c.id == name).unwrap();
                let module_path = path_to_module(component.logic_path.as_ref().unwrap()).unwrap();

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
                        let component = components_clone.iter().find(|c| c.id == name).ok_or_else(|| {
                            minijinja::Error::new(minijinja::ErrorKind::TemplateNotFound, "Component not found")
                        })?;
                        let mut template_path = component.template_path.clone();
                        if template_path.starts_with("./") {
                            template_path = template_path[2..].to_string();
                        }
                        let tmpl = state.env().get_template(&template_path)?;
                        let mut rendered_component = tmpl.render(result.context)?;

                        let replacement = format!(r#"$1<input type="hidden" name="component_id" value="{}">"#, name);
                        rendered_component = FORM_REGEX.replace_all(&rendered_component, replacement).to_string();

                        Ok(Value::from_safe_string(rendered_component))
                    }
                    Ok(Err(py_err)) => {
                        let detailed_error = DetailedError {
                            component: Some(ComponentInfo { name: name.clone() }),
                            error_source: Some(ErrorSource::Python(py_err)),
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
            },
        );

        self.render_page(&env, &msg.template_name).map_err(|e| {
            if let Some(detailed_error) = e.source().and_then(|s| s.downcast_ref::<DetailedError>()) {
                return detailed_error.clone();
            }
            let template_info = crate::errors::TemplateInfo {
                name: e.name().unwrap_or(&msg.template_name).to_string(),
                line: e.line().unwrap_or(0),
                source: None,
                detail: e.detail().unwrap_or("").to_string(),
                traceback: Some(format!("{:?}", e)),
            };
            DetailedError {
                page: Some(template_info.clone()),
                error_source: Some(ErrorSource::Template(template_info)),
                ..Default::default()
            }
        })
    }
}

impl Handler<UpdateComponents> for TemplateRendererActor {
    type Result = ();

    fn handle(&mut self, msg: UpdateComponents, _ctx: &mut Self::Context) -> Self::Result {
        self.components = msg.0;
    }
}

impl Handler<UpdateSingleComponent> for TemplateRendererActor {
    type Result = ();

    fn handle(&mut self, msg: UpdateSingleComponent, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(component) = self.components.iter_mut().find(|c| c.id == msg.0.id) {
            *component = msg.0;
        } else {
            self.components.push(msg.0);
        }
    }
}

fn path_to_module(path_str: &str) -> Result<String, std::io::Error> {
    let path = std::path::Path::new(path_str);
    
    // Clean the path to remove "./"
    let cleaned_path = if path.starts_with("./") {
        path.strip_prefix("./").unwrap()
    } else {
        path
    };

    // Convert to string and remove the .py extension
    let module_str = cleaned_path.to_str().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "Path contains invalid UTF-8"))?;
    let module_str_no_ext = module_str.strip_suffix(".py").unwrap_or(module_str);

    // Replace slashes with dots for Python import syntax
    let module_path = module_str_no_ext.replace("/", ".");

    Ok(module_path)
}
use crate::actors::health::{HealthActor, ReportTemplateLatency, ReportPythonLatency};
use crate::actors::interpreter::{ExecutePythonFunction, PythonInterpreterActor};
use crate::actors::page_renderer::HttpRequestInfo;
use crate::components::Component;
use actix::prelude::*;
use minijinja::{Environment, Error, State, value::Kwargs, Value};
use std::sync::Arc;
use std::collections::HashMap;
use regex::Regex;

// Actor for rendering templates
pub struct TemplateRendererActor {
    env: Arc<Environment<'static>>,
    interpreter: Addr<PythonInterpreterActor>,
    health_actor: Addr<HealthActor>,
    dev_mode: bool,
    components: Vec<Component>,
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
    fn scan_components(&mut self) {
        if self.dev_mode {
            log::info!("Re-scanning components...");
            let components = crate::components::scan_components(std::path::Path::new("./components")).unwrap_or_else(|e| {
                log::error!("Failed to discover components: {}", e);
                vec![]
            });
            log::info!("Found {} components.", components.len());
            self.components = components;
        }
    }
}

impl Actor for TemplateRendererActor {
    type Context = SyncContext<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        self.scan_components();
    }
}

// Message for rendering a template
#[derive(Message)]
#[rtype(result = "Result<String, Error>")]
pub struct RenderTemplate {
    pub template_name: String,
    pub request_info: Arc<HttpRequestInfo>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct RescanComponents;

impl Handler<RenderTemplate> for TemplateRendererActor {
    type Result = Result<String, Error>;

    fn handle(&mut self, msg: RenderTemplate, _ctx: &mut Self::Context) -> Self::Result {
        let mut env = (*self.env).clone();
        if self.dev_mode {
            env.set_loader(minijinja::path_loader("."));
        }

        let interpreter_clone = self.interpreter.clone();
        let health_actor_clone = self.health_actor.clone();
        let request_info_clone = msg.request_info.clone();
        let components_clone = self.components.clone();

        let form_component_id = if request_info_clone.method == "POST" {
            let form_data: HashMap<String, String> =
                serde_json::from_value(serde_json::Value::Object(request_info_clone.form_data.clone())).unwrap();
            form_data.get("component_id").cloned().unwrap_or_default()
        } else {
            String::new()
        };

        env.add_function(
            "component",
            move |state: &State, name: String, kwargs: Kwargs| -> Result<Value, Error> {
                let kwargs_map: HashMap<String, Value> = kwargs
                    .args()
                    .filter_map(|k| kwargs.get::<Value>(k).ok().map(|v| (k.to_string(), v)))
                    .collect();

                let component_id = &name;

                let execute_fn_msg = if request_info_clone.method == "POST" && &form_component_id == component_id {
                    let form_data: HashMap<String, String> =
                        serde_json::from_value(serde_json::Value::Object(request_info_clone.form_data.clone())).unwrap();
                    let action = form_data.get("action").cloned().unwrap_or_default();

                    if action.is_empty() {
                         ExecutePythonFunction {
                            component_name: name.clone(),
                            function_name: "load_template_context".to_string(),
                            request: request_info_clone.clone(),
                            args: Some(kwargs_map),
                        }
                    } else {
                        let mut form_data_value = HashMap::new();
                        for (k, v) in form_data {
                            form_data_value.insert(k.clone(), Value::from(v.clone()));
                        }

                        let mut kwargs_map_post = kwargs_map.clone();
                        kwargs_map_post.extend(form_data_value);

                        ExecutePythonFunction {
                            component_name: name.clone(),
                            function_name: format!("action_{}", action),
                            request: request_info_clone.clone(),
                            args: Some(kwargs_map_post),
                        }
                    }
                } else {
                    ExecutePythonFunction {
                        component_name: name.clone(),
                        function_name: "load_template_context".to_string(),
                        request: request_info_clone.clone(),
                        args: Some(kwargs_map),
                    }
                };

                let python_start_time = std::time::Instant::now();
                let future = interpreter_clone.send(execute_fn_msg);
                let result = futures::executor::block_on(future);
                let python_duration_ms = python_start_time.elapsed().as_secs_f64() * 1000.0;
                health_actor_clone.do_send(ReportPythonLatency(python_duration_ms));

                match result {
                    Ok(Ok(context)) => {
                        let component = components_clone.iter().find(|c| c.id == name).ok_or_else(|| {
                            Error::new(minijinja::ErrorKind::TemplateNotFound, "Component not found")
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
                    Ok(Err(e)) => {
                        log::error!("Error executing python function: {}", e);
                        Err(minijinja::Error::new(minijinja::ErrorKind::InvalidOperation, e.to_string()))
                    }
                    Err(e) => {
                        log::error!("Mailbox error: {}", e);
                        Err(minijinja::Error::new(minijinja::ErrorKind::InvalidOperation, e.to_string()))
                    }
                }
            },
        );

        let tmpl = env.get_template(&msg.template_name)?;
        let start_time = std::time::Instant::now();
        let mut result = tmpl.render(minijinja::context! {})?;
        let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;
        self.health_actor.do_send(ReportTemplateLatency(duration_ms));

        const MDOM_SCRIPT_CONTENT: &str = include_str!("../scripts/idiomorph.js");
        if let Some(body_end_pos) = result.rfind("</body>") {
            let script_tag = format!("<script>{}</script>\n", MDOM_SCRIPT_CONTENT);
            result.insert_str(body_end_pos, &script_tag);
        }

        const SCRIPT_CONTENT: &str = include_str!("../scripts/frontend.js");
        if let Some(body_end_pos) = result.rfind("</body>") {
            let script_tag = format!("<script>{}</script>\n", SCRIPT_CONTENT);
            result.insert_str(body_end_pos, &script_tag);
        }

        if self.dev_mode {
            const DEV_SCRIPT_CONTENT: &str = include_str!("../scripts/devws.js");
            if let Some(body_end_pos) = result.rfind("</body>") {
                let script_tag = format!("<script>{}</script>\n", DEV_SCRIPT_CONTENT);
                result.insert_str(body_end_pos, &script_tag);
            }
        }

        if let Some(body_end_pos) = result.rfind("</body>") {
            let script_tag = "<script src=\"https://cdn.jsdelivr.net/npm/@unocss/runtime/uno.global.js\"></script>";
            result.insert_str(body_end_pos, &script_tag);
        }

        Ok(result)
    }
}

impl Handler<RescanComponents> for TemplateRendererActor {
    type Result = ();

    fn handle(&mut self, _msg: RescanComponents, _ctx: &mut Self::Context) -> Self::Result {
        self.scan_components();
    }
}
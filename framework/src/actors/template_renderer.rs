use crate::actors::component_renderer::{ComponentRendererActor, HandleRender};
use crate::actors::health::{HealthActor, ReportTemplateLatency};
use crate::actors::page_renderer::HttpRequestInfo;
use crate::components::Component;
use actix::prelude::*;
use minijinja::{Environment, Error, State, value::Kwargs};
use std::sync::Arc;

// Actor for rendering templates
pub struct TemplateRendererActor {
    env: Arc<Environment<'static>>,
    component_renderer: Addr<ComponentRendererActor>,
    health_actor: Addr<HealthActor>,
    dev_mode: bool,
    components: Vec<Component>,
}

impl TemplateRendererActor {
    pub fn new(
        component_renderer: Addr<ComponentRendererActor>,
        health_actor: Addr<HealthActor>,
        dev_mode: bool,
        components: Vec<Component>,
    ) -> Self {
        let mut env = Environment::new();
        env.set_loader(minijinja::path_loader("."));

        Self {
            env: Arc::new(env),
            component_renderer,
            health_actor,
            dev_mode,
            components,
        }
    }
}

impl Actor for TemplateRendererActor {
    type Context = SyncContext<Self>;
}

// Message for rendering a template
#[derive(Message)]
#[rtype(result = "Result<String, Error>")]
pub struct RenderTemplate {
    pub template_name: String,
    pub request_info: Arc<HttpRequestInfo>,
}

impl Handler<RenderTemplate> for TemplateRendererActor {
    type Result = Result<String, Error>;

    fn handle(&mut self, msg: RenderTemplate, _ctx: &mut Self::Context) -> Self::Result {
        let mut env = (*self.env).clone();
        if self.dev_mode {
            env.set_loader(minijinja::path_loader("."));
        }

        let component_renderer_clone = self.component_renderer.clone();
        let _health_actor_clone = self.health_actor.clone();
        let request_info_clone = msg.request_info.clone();
        let components_clone = self.components.clone();

        env.add_function(
            "component",
            move |state: &State, name: String, kwargs: Kwargs| -> Result<minijinja::Value, Error> {
                let component_renderer_clone = component_renderer_clone.clone();
                let request_info_clone = request_info_clone.clone();
                let components_clone = components_clone.clone();

                let kwargs_map: std::collections::HashMap<String, minijinja::Value> = kwargs
                    .args()
                    .filter_map(|k| {
                        kwargs
                            .get::<minijinja::Value>(k)
                            .ok()
                            .map(|v| (k.to_string(), v))
                    })
                    .collect();

                let handle_render_msg = HandleRender {
                    name: name.clone(),
                    req: request_info_clone,
                    kwargs: Some(kwargs_map),
                };

                let fut = async move {
                    match component_renderer_clone.send(handle_render_msg).await {
                        Ok(Ok(context)) => {
                            let component = components_clone.iter().find(|c| c.id == name).ok_or_else(|| {
                                Error::new(minijinja::ErrorKind::TemplateNotFound, "Component not found")
                            })?;
                            let mut template_path = component.template_path.clone();
                            if template_path.starts_with("./") {
                                template_path = template_path[2..].to_string();
                            }

                            let tmpl = state.env().get_template(&template_path)?;
                            let result = tmpl.render(context)?;
                            Ok(minijinja::Value::from_safe_string(result))
                        }
                        Ok(Err(e)) => {
                            log::error!("Error rendering component: {}", e);
                            Err(Error::new(
                                minijinja::ErrorKind::InvalidOperation,
                                "Failed to get component context",
                            ))
                        }
                        Err(e) => {
                            log::error!("Mailbox error: {}", e);
                            Err(Error::new(
                                minijinja::ErrorKind::InvalidOperation,
                                "Failed to get component context",
                            ))
                        }
                    }
                };

                futures::executor::block_on(fut)
            },
        );

        let tmpl = env.get_template(&msg.template_name)?;
        let start_time = std::time::Instant::now();
        let mut result = tmpl.render(minijinja::context! {})?;
        let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;
        self.health_actor
            .do_send(ReportTemplateLatency(duration_ms));


        const MDOM_SCRIPT_CONTENT: &str = include_str!("../scripts/morphdom-umd.min.js");
        if let Some(body_end_pos) = result.rfind("</body>") {
            let script_tag = format!("<script>{}</script>\n", MDOM_SCRIPT_CONTENT);
            result.insert_str(body_end_pos, &script_tag);
        }

        const SCRIPT_CONTENT: &str = include_str!("../scripts/frontend.js");
        if let Some(body_end_pos) = result.rfind("</body>") {
            let script_tag = format!("<script>{}</script>\n", SCRIPT_CONTENT);
            result.insert_str(body_end_pos, &script_tag);
        }

        if let Some(body_end_pos) = result.rfind("</body>") {
            let script_tag = "<script src=\"https://cdn.jsdelivr.net/npm/@unocss/runtime/uno.global.js\"></script>";
            result.insert_str(body_end_pos, &script_tag);
        }

        Ok(result)
    }
}
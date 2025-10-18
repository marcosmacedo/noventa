use crate::actors::component_renderer::{ComponentRendererActor, HandleRender};
use crate::actors::page_renderer::HttpRequestInfo;
use actix::prelude::*;
use minijinja::{Environment, Error, State};
use std::sync::Arc;

// Actor for rendering templates
pub struct TemplateRendererActor {
    env: Arc<Environment<'static>>,
    component_renderer: Addr<ComponentRendererActor>,
}


impl TemplateRendererActor {
    pub fn new(
        component_renderer: Addr<ComponentRendererActor>,
    ) -> Self {
        let mut env = Environment::new();

        // Add page templates
        let pages_dir = std::path::Path::new("../web/pages");
        for entry in walkdir::WalkDir::new(pages_dir)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().is_file())
        {
            let path = entry.into_path();
            let name = path.file_name().unwrap().to_str().unwrap().to_owned();
            let template = std::fs::read_to_string(path).unwrap();
            env.add_template_owned(name, template).unwrap();
        }

        // Add component templates
        let components_dir = std::path::Path::new("../web/components");
        for entry in walkdir::WalkDir::new(components_dir)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().is_file() && e.path().extension().and_then(|s| s.to_str()) == Some("html"))
        {
            let path = entry.path();
            let name = path.parent().unwrap().file_name().unwrap().to_str().unwrap().to_owned();
            let template = std::fs::read_to_string(path).unwrap();
            env.add_template_owned(name, template).unwrap();
        }

        Self {
            env: Arc::new(env),
            component_renderer,
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
        let component_renderer_clone = self.component_renderer.clone();
        let request_info_clone = msg.request_info.clone();

        env.add_function(
            "component",
            move |state: &State, name: String| -> Result<minijinja::Value, Error> {
                let component_renderer_clone = component_renderer_clone.clone();
                let request_info_clone = request_info_clone.clone();
                let handle_render_msg = HandleRender {
                    name: name.clone(),
                    req: request_info_clone,
                };

                let fut = async move {
                    match component_renderer_clone.send(handle_render_msg).await {
                        Ok(Ok(context)) => {
                            let tmpl = state.env().get_template(&name).unwrap();
                            Ok(minijinja::Value::from_safe_string(
                                tmpl.render(context).unwrap(),
                            ))
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

                actix::System::new().block_on(fut)
            },
        );

        let tmpl = env.get_template(&msg.template_name)?;
        let result = tmpl.render(minijinja::context! {});

        if let Err(e) = &result {
            log::error!("Error rendering template: {}", e);
        }

        result
    }
}
use actix::{Actor, Context, Handler, Message};
use crate::components::scan_components;
use minijinja::{Environment, Error, State};
use std::collections::HashMap;
use std::sync::Arc;

pub struct RendererActor {
    env: Arc<Environment<'static>>,
}

impl RendererActor {
    pub fn new() -> Self {
        let components_path = std::path::Path::new("../web/components");
        let components = scan_components(components_path).unwrap();
        let mut env = Environment::new();

        for (name, component) in &components {
            let template = std::fs::read_to_string(&component.template_path).unwrap();
            env.add_template_owned(name.clone(), template).unwrap();
        }

        env.add_function(
            "component",
            move |state: &State, name: String| -> Result<String, Error> {
                let tmpl = state.env().get_template(&name)?;
                let context = state.lookup(".").unwrap_or(minijinja::Value::from_serialize(
                    &HashMap::<String, String>::new(),
                ));
                tmpl.render(context)
            },
        );

        env.set_loader(|name| {
            let path = std::path::Path::new("../web/pages").join(name);
            match std::fs::read_to_string(path) {
                Ok(s) => Ok(Some(s)),
                Err(_) => Ok(None),
            }
        });

        Self {
            env: Arc::new(env),
        }
    }
}

impl Actor for RendererActor {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result = "Result<String, minijinja::Error>")]
pub struct RenderMessage {
    pub template_name: String,
    pub context: HashMap<String, serde_json::Value>,
}

impl Handler<RenderMessage> for RendererActor {
    type Result = Result<String, minijinja::Error>;

    fn handle(&mut self, msg: RenderMessage, _ctx: &mut Context<Self>) -> Self::Result {
        let tmpl = self.env.get_template(&msg.template_name)?;
        tmpl.render(&msg.context)
    }
}
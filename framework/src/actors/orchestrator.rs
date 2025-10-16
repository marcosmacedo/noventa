use crate::actors::interpreter::RenderComponent;
use crate::actors::manager::InterpreterManager;
use crate::actors::renderer::{RenderMessage, RendererActor};
use actix::prelude::*;
use std::io::{Error, ErrorKind};

pub struct HttpOrchestratorActor {
    interpreter: Addr<InterpreterManager>,
    renderer: Addr<RendererActor>,
}

impl HttpOrchestratorActor {
    pub fn new(interpreter: Addr<InterpreterManager>, renderer: Addr<RendererActor>) -> Self {
        Self {
            interpreter,
            renderer,
        }
    }
}

impl Actor for HttpOrchestratorActor {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result = "Result<String, Error>")]
pub struct HandleRequest {
    pub component_name: String,
    pub template_name: String,
}

impl Handler<HandleRequest> for HttpOrchestratorActor {
    type Result = ResponseFuture<Result<String, Error>>;

    fn handle(&mut self, msg: HandleRequest, _ctx: &mut Self::Context) -> Self::Result {
        let interpreter = self.interpreter.clone();
        let renderer = self.renderer.clone();

        Box::pin(async move {
            let render_component_msg = RenderComponent {
                name: msg.component_name,
            };

            let context = match interpreter.send(render_component_msg).await {
                Ok(Ok(context)) => context,
                Ok(Err(e)) => {
                    log::error!("Interpreter actor error: {}", e);
                    return Err(e);
                }
                Err(e) => {
                    log::error!("Mailbox error calling interpreter actor: {}", e);
                    return Err(Error::new(ErrorKind::Other, e.to_string()));
                }
            };

            let render_msg = RenderMessage {
                template_name: msg.template_name,
                context,
            };

            match renderer.send(render_msg).await {
                Ok(Ok(rendered)) => Ok(rendered),
                Ok(Err(e)) => {
                    log::error!("Renderer actor error: {}", e);
                    Err(Error::new(ErrorKind::Other, e.to_string()))
                }
                Err(e) => {
                    log::error!("Mailbox error calling renderer actor: {}", e);
                    Err(Error::new(ErrorKind::Other, e.to_string()))
                }
            }
        })
    }
}
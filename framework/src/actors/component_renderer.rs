use crate::actors::interpreter::{ExecutePythonFunction, PythonInterpreterActor};
use crate::actors::page_renderer::HttpRequestInfo;
use actix::prelude::*;
use serde::Deserialize;
use std::sync::Arc;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Error, ErrorKind};

pub struct ComponentRendererActor {
    interpreter: Addr<PythonInterpreterActor>,
}

impl ComponentRendererActor {
    pub fn new(interpreter: Addr<PythonInterpreterActor>) -> Self {
        Self { interpreter }
    }
}

impl Actor for ComponentRendererActor {
    type Context = Context<Self>;
}


#[derive(Message)]
#[rtype(result = "Result<HashMap<String, Value>, Error>")]
pub struct HandleRender {
    pub name: String,
    pub req: Arc<HttpRequestInfo>,
}

impl Handler<HandleRender> for ComponentRendererActor {
    type Result = ResponseFuture<Result<HashMap<String, Value>, Error>>;

    fn handle(&mut self, msg: HandleRender, _ctx: &mut Self::Context) -> Self::Result {
        let interpreter = self.interpreter.clone();
        let component_name = msg.name.clone();

        Box::pin(async move {
            let req = msg.req;
            let req_str = serde_json::to_string(&*req).unwrap();

            let mut args = HashMap::new();
            args.insert("request".to_string(), req_str);

            let execute_fn_msg = if req.method == "GET" {
                ExecutePythonFunction {
                    component_name,
                    function_name: "load_template_context".to_string(),
                    args: Some(args),
                }
            } else {
                let form_data: HashMap<String, String> =
                    serde_json::from_value(serde_json::Value::Object(req.form_data.clone())).unwrap();
                let action = form_data.get("action").cloned().unwrap_or_default();

                if action.is_empty() {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
                        "Action is required for POST requests",
                    ));
                }

                args.extend(form_data);

                ExecutePythonFunction {
                    component_name,
                    function_name: format!("action_{}", action),
                    args: Some(args),
                }
            };

            match interpreter.send(execute_fn_msg).await {
                Ok(Ok(context)) => Ok(context),
                Ok(Err(e)) => {
                    log::error!("Error executing python function: {}", e);
                    Err(e)
                }
                Err(e) => {
                    log::error!("Mailbox error: {}", e);
                    Err(Error::new(ErrorKind::Other, e.to_string()))
                }
            }
        })
    }
}
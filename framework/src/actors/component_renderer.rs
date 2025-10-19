use crate::actors::health::{HealthActor, ReportPythonLatency};
use crate::actors::interpreter::{ExecutePythonFunction, PythonInterpreterActor};
use crate::actors::page_renderer::HttpRequestInfo;
use actix::prelude::*;
use minijinja::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::io::{Error, ErrorKind};
use std::time::Duration;
use actix_web::rt::time::timeout;

pub struct ComponentRendererActor {
    interpreter: Addr<PythonInterpreterActor>,
    health_actor: Addr<HealthActor>,
}

impl ComponentRendererActor {
    pub fn new(interpreter: Addr<PythonInterpreterActor>, health_actor: Addr<HealthActor>) -> Self {
        Self {
            interpreter,
            health_actor,
        }
    }
}

impl Actor for ComponentRendererActor {
    type Context = Context<Self>;
}


#[derive(Message)]
#[rtype(result = "Result<Value, Error>")]
pub struct HandleRender {
    pub name: String,
    pub req: Arc<HttpRequestInfo>,
}

impl Handler<HandleRender> for ComponentRendererActor {
    type Result = ResponseFuture<Result<Value, Error>>;

    fn handle(&mut self, msg: HandleRender, _ctx: &mut Self::Context) -> Self::Result {
        let interpreter = self.interpreter.clone();
        let health_actor = self.health_actor.clone();
        let component_name = msg.name.clone();

        Box::pin(async move {
            let actor_start_time = std::time::Instant::now();
            let req = msg.req;
            let mut args = HashMap::new();

            let execute_fn_msg = if req.method == "GET" {
                ExecutePythonFunction {
                    component_name,
                    function_name: "load_template_context".to_string(),
                    request: req,
                    args: None,
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
                    request: req,
                    args: Some(args),
                }
            };

            let python_start_time = std::time::Instant::now();
            let future = interpreter.send(execute_fn_msg);
            let result = timeout(Duration::from_secs(5), future).await;
            let python_duration_ms = python_start_time.elapsed().as_secs_f64() * 1000.0;
            health_actor.do_send(ReportPythonLatency(python_duration_ms));


            match result {
                Ok(Ok(Ok(context))) => Ok(context),
                Ok(Ok(Err(e))) => {
                    log::error!("Error executing python function: {}", e);
                    Err(e)
                }
                Ok(Err(e)) => {
                    log::error!("Mailbox error: {}", e);
                    Err(Error::new(ErrorKind::Other, e.to_string()))
                }
                Err(_) => {
                    log::error!("Timeout error waiting for python interpreter");
                    Err(Error::new(
                        ErrorKind::TimedOut,
                        "Timeout waiting for python interpreter",
                    ))
                }
            }
        })
    }
}
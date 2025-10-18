use crate::actors::template_renderer::{RenderTemplate, TemplateRendererActor};
use actix::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Serialize, Deserialize)]
pub enum FileData {
    InMemory(Vec<u8>),
    OnDisk(PathBuf),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FilePart {
    pub filename: String,
    pub headers: HashMap<String, String>,
    pub data: FileData,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct HttpRequestInfo {
    pub path: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub form_data: serde_json::Map<String, serde_json::Value>,
    pub files: HashMap<String, FilePart>,
    pub query_params: HashMap<String, String>,
    pub path_params: HashMap<String, String>,
}

pub struct PageRendererActor {
    template_renderer: Addr<TemplateRendererActor>,
}

impl PageRendererActor {
    pub fn new(template_renderer: Addr<TemplateRendererActor>) -> Self {
        Self { template_renderer }
    }
}

impl Actor for PageRendererActor {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result = "Result<String, minijinja::Error>")]
pub struct RenderMessage {
    pub template_path: String,
    pub request_info: Arc<HttpRequestInfo>,
}

impl Handler<RenderMessage> for PageRendererActor {
    type Result = ResponseFuture<Result<String, minijinja::Error>>;

    fn handle(&mut self, msg: RenderMessage, _ctx: &mut Context<Self>) -> Self::Result {
        let template_renderer = self.template_renderer.clone();
        Box::pin(async move {
            let render_msg = RenderTemplate {
                template_name: msg.template_path,
                request_info: msg.request_info.clone(),
            };

            match template_renderer.send(render_msg).await {
                Ok(Ok(rendered)) => Ok(rendered),
                Ok(Err(e)) => {
                    log::error!("Error rendering template: {}", e);
                    Err(e)
                }
                Err(e) => {
                    log::error!("Mailbox error: {}", e);
                    Err(minijinja::Error::new(
                        minijinja::ErrorKind::InvalidOperation,
                        "Mailbox error",
                    ))
                }
            }
        })
    }
}
use crate::actors::health::{HealthActor, ReportTemplateLatency};
use crate::actors::session_manager::SessionManagerActor;
use crate::actors::template_renderer::{RenderTemplate, TemplateRendererActor};
use actix::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use actix_web::rt::time::timeout;

#[derive(Clone, Serialize, Deserialize)]
pub enum FileData {
    InMemory(Vec<u8>),
    OnDisk(PathBuf),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FilePart {
    pub filename: String,
    pub content_type: String,
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
    pub scheme: String,
    pub host: String,
    pub remote_addr: Option<String>,
    pub url: String,
    pub base_url: String,
    pub host_url: String,
    pub url_root: String,
    pub full_path: String,
    pub query_string: Vec<u8>,
    pub cookies: HashMap<String, String>,
    pub user_agent: Option<String>,
    pub content_type: Option<String>,
    pub content_length: Option<usize>,
    pub is_secure: bool,
    pub is_xhr: bool,
    pub accept_charsets: Vec<String>,
    pub accept_encodings: Vec<String>,
    pub accept_languages: Vec<String>,
    pub accept_mimetypes: Vec<String>,
    pub access_route: Vec<String>,
    pub authorization: Option<String>,
    pub cache_control: Option<String>,
    pub content_encoding: Option<String>,
    pub content_md5: Option<String>,
    pub date: Option<String>,
    pub if_match: Vec<String>,
    pub if_modified_since: Option<String>,
    pub if_none_match: Vec<String>,
    pub if_range: Option<String>,
    pub if_unmodified_since: Option<String>,
    pub max_forwards: Option<String>,
    pub pragma: Option<String>,
    pub range: Option<String>,
    pub referrer: Option<String>,
    pub remote_user: Option<String>,
}

pub struct PageRendererActor {
    template_renderer: Addr<TemplateRendererActor>,
    health_actor: Addr<HealthActor>,
}

impl PageRendererActor {
    pub fn new(template_renderer: Addr<TemplateRendererActor>, health_actor: Addr<HealthActor>) -> Self {
        Self {
            template_renderer,
            health_actor,
        }
    }
}

impl Actor for PageRendererActor {
    type Context = Context<Self>;
}

#[derive(Clone)]
pub enum RenderOutput {
    Html(String),
    Redirect(String),
}

#[derive(Message, Clone)]
#[rtype(result = "Result<RenderOutput, crate::errors::DetailedError>")]
pub struct RenderMessage {
    pub template_path: String,
    pub request_info: Arc<HttpRequestInfo>,
    pub session_manager: Addr<SessionManagerActor>,
}

impl Handler<RenderMessage> for PageRendererActor {
    type Result = ResponseFuture<Result<RenderOutput, crate::errors::DetailedError>>;

    fn handle(&mut self, msg: RenderMessage, _ctx: &mut Context<Self>) -> Self::Result {
        let template_renderer = self.template_renderer.clone();
        let health_actor = self.health_actor.clone();
        Box::pin(async move {
            let render_msg = RenderTemplate {
                template_name: msg.template_path,
                request_info: msg.request_info.clone(),
                session_manager: msg.session_manager,
            };

            let start_time = std::time::Instant::now();
            let future = template_renderer.send(render_msg);
            let result = timeout(Duration::from_secs(5), future).await;
            let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;
            health_actor.do_send(ReportTemplateLatency(duration_ms));

            match result {
                Ok(inner) => match inner {
                    Ok(render_res) => match render_res {
                        Ok(rendered) => Ok(rendered),
                        Err(e) => Err(e),
                    },
                    Err(mailbox_err) => {
                        log::error!("Template renderer mailbox error: {}", mailbox_err);
                        Err(crate::errors::DetailedError {
                            error_source: None,
                            ..Default::default()
                        })
                    }
                },
                Err(_) => {
                    log::error!("The template renderer timed out. The server is taking too long to respond.");
                    Err(crate::errors::DetailedError {
                        error_source: Some(crate::errors::ErrorSource::Python(crate::actors::interpreter::PythonError {
                            message: "Timeout".to_string(),
                            traceback: "".to_string(),
                            line_number: None,
                            column_number: None,
                            end_line_number: None,
                            end_column_number: None,
                            filename: None,
                            source_code: None,
                        })),
                        ..Default::default()
                    })
                }
            }
        })
    }
}
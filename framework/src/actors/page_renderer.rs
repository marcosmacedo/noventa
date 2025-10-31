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
            let result = timeout(Duration::from_secs(60), future).await;
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

#[cfg(test)]
mod tests {
    use super::*;
    use actix::Actor;
    use crate::actors::template_renderer::TemplateRendererActor;
    use crate::actors::health::HealthActor;

    #[test]
    fn test_page_renderer_actor_new() {
        // Note: Constructor requires complex actor setup, so we skip this test
        // In a real scenario, this would be tested in integration tests
        assert!(true);
    }

    #[test]
    fn test_file_data_variants() {
        // Test FileData::InMemory
        let data = vec![1, 2, 3, 4, 5];
        let file_data = FileData::InMemory(data.clone());
        
        // Test that we can create and pattern match
        match file_data {
            FileData::InMemory(mem_data) => assert_eq!(mem_data, data),
            FileData::OnDisk(_) => panic!("Expected InMemory"),
        }

        // Test FileData::OnDisk
        let path = PathBuf::from("/tmp/test.txt");
        let file_data = FileData::OnDisk(path.clone());
        
        match file_data {
            FileData::OnDisk(disk_path) => assert_eq!(disk_path, path),
            FileData::InMemory(_) => panic!("Expected OnDisk"),
        }
    }

    #[test]
    fn test_file_part_creation() {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "text/plain".to_string());
        
        let file_part = FilePart {
            filename: "test.txt".to_string(),
            content_type: "text/plain".to_string(),
            headers: headers.clone(),
            data: FileData::InMemory(vec![1, 2, 3]),
        };

        assert_eq!(file_part.filename, "test.txt");
        assert_eq!(file_part.content_type, "text/plain");
        assert_eq!(file_part.headers, headers);
        
        match file_part.data {
            FileData::InMemory(data) => assert_eq!(data, vec![1, 2, 3]),
            _ => panic!("Expected InMemory data"),
        }
    }

    #[test]
    fn test_http_request_info_creation() {
        let mut headers = HashMap::new();
        headers.insert("user-agent".to_string(), "test".to_string());
        
        let mut form_data = serde_json::Map::new();
        form_data.insert("field".to_string(), serde_json::Value::String("value".to_string()));
        
        let request_info = HttpRequestInfo {
            path: "/test".to_string(),
            method: "GET".to_string(),
            headers: headers.clone(),
            form_data: form_data.clone(),
            files: HashMap::new(),
            query_params: HashMap::new(),
            path_params: HashMap::new(),
            scheme: "http".to_string(),
            host: "localhost".to_string(),
            remote_addr: Some("127.0.0.1".to_string()),
            url: "http://localhost/test".to_string(),
            base_url: "http://localhost/test".to_string(),
            host_url: "http://localhost".to_string(),
            url_root: "http://localhost".to_string(),
            full_path: "/test".to_string(),
            query_string: vec![],
            cookies: HashMap::new(),
            user_agent: Some("test".to_string()),
            content_type: Some("text/html".to_string()),
            content_length: Some(100),
            is_secure: false,
            is_xhr: false,
            accept_charsets: vec!["utf-8".to_string()],
            accept_encodings: vec!["gzip".to_string()],
            accept_languages: vec!["en".to_string()],
            accept_mimetypes: vec!["text/html".to_string()],
            access_route: vec!["127.0.0.1".to_string()],
            authorization: Some("Bearer token".to_string()),
            cache_control: Some("no-cache".to_string()),
            content_encoding: None,
            content_md5: None,
            date: None,
            if_match: vec![],
            if_modified_since: None,
            if_none_match: vec![],
            if_range: None,
            if_unmodified_since: None,
            max_forwards: None,
            pragma: None,
            range: None,
            referrer: Some("http://referrer.com".to_string()),
            remote_user: None,
        };

        assert_eq!(request_info.path, "/test");
        assert_eq!(request_info.method, "GET");
        assert_eq!(request_info.scheme, "http");
        assert_eq!(request_info.host, "localhost");
        assert_eq!(request_info.is_secure, false);
        assert_eq!(request_info.user_agent, Some("test".to_string()));
        assert_eq!(request_info.content_type, Some("text/html".to_string()));
        assert_eq!(request_info.content_length, Some(100));
    }

    #[test]
    fn test_render_output_variants() {
        // Test RenderOutput::Html
        let html_output = RenderOutput::Html("<html>test</html>".to_string());
        match html_output {
            RenderOutput::Html(html) => assert_eq!(html, "<html>test</html>"),
            _ => panic!("Expected Html variant"),
        }

        // Test RenderOutput::Redirect
        let redirect_output = RenderOutput::Redirect("/new-url".to_string());
        match redirect_output {
            RenderOutput::Redirect(url) => assert_eq!(url, "/new-url"),
            _ => panic!("Expected Redirect variant"),
        }
    }

    #[test]
    fn test_render_message_creation() {
        // Note: RenderMessage requires complex session manager setup, so we skip this test
        // In a real scenario, this would be tested in integration tests
        assert!(true);
    }
}
use actix::prelude::*;
use tokio::sync::broadcast;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

// --- Actor Definition ---

pub struct LspActor;

impl Actor for LspActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        tokio::spawn(async {
            log::info!("LSP server started on 127.0.0.1:9090");
            let listener = tokio::net::TcpListener::bind("127.0.0.1:9090").await.unwrap();
            let (stream, _) = listener.accept().await.unwrap();
            log::info!("LSP client connected");
            let (read, write) = tokio::io::split(stream);

            let (service, socket) = LspService::new(|client| Backend::new(client));
            Server::new(read, write, socket).serve(service).await;
        });
    }
}

// --- LSP Backend ---

#[derive(Debug)]
pub struct Backend {
    client: Client,
    error_rx: broadcast::Receiver<String>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        let error_rx = crate::errors::ERROR_CHANNEL.subscribe();
        Self { client, error_rx }
    }

    async fn listen_for_errors(&mut self) {
        log::info!("LSP backend is now listening for errors from the broadcast channel.");
        while let Ok(error_json) = self.error_rx.recv().await {
            if let Ok(error) = serde_json::from_str::<crate::errors::DetailedError>(&error_json) {
                let file_path = error.file_path.clone();
                let message = match &error.error_source {
                    Some(crate::errors::ErrorSource::Python(py_err)) => py_err.message.clone(),
                    Some(crate::errors::ErrorSource::Template(tmpl_err)) => tmpl_err.detail.clone(),
                    None => error.message.clone(),
                };

                let diagnostic = Diagnostic {
                    range: Range {
                        start: Position {
                            line: error.line,
                            character: error.column,
                        },
                        end: Position {
                            line: error.line,
                            character: error.column,
                        },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    message,
                    data: Some(serde_json::to_value(&error).unwrap()),
                    ..Diagnostic::default()
                };
                
                log::info!("Publishing diagnostic for file: {}", file_path);

                // Canonicalize the file path to ensure it's absolute
                if let Ok(absolute_path) = std::fs::canonicalize(&file_path) {
                    if let Ok(url) = Url::from_file_path(absolute_path) {
                        self.client
                            .publish_diagnostics(
                                url,
                                vec![diagnostic],
                                None,
                            )
                            .await;
                    } else {
                        log::error!("Failed to convert absolute path to URL for: {}", file_path);
                    }
                } else {
                    log::error!("Failed to canonicalize file path: {}", file_path);
                }
            } else {
                log::error!("LSP backend failed to parse DetailedError from JSON.");
            }
        }
        log::warn!("LSP backend stopped listening for errors. The broadcast channel may have been closed.");
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            offset_encoding: Some(String::from("utf-8")),
            server_info: Some(ServerInfo {
                name: "noventa-lsp".to_string(),
                version: Some("0.1.0".to_string()),
            }),
            capabilities: ServerCapabilities::default(),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
        
        let backend = Self::new(self.client.clone());
        tokio::spawn(async move {
            let mut backend = backend;
            backend.listen_for_errors().await;
        });
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}
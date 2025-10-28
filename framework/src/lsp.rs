use actix::prelude::*;
use dashmap::DashMap;
use lazy_static::lazy_static;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

// --- Global State ---

struct FileInfo {
    uri: Url,
}

lazy_static! {
    static ref OPEN_FILES: DashMap<String, FileInfo> = DashMap::new();
}
use std::sync::atomic::{AtomicUsize, Ordering};

lazy_static! {
    static ref ALL_CLIENTS: DashMap<usize, Client> = DashMap::new();
}
static CLIENT_COUNTER: AtomicUsize = AtomicUsize::new(1);

// --- Actor Definition ---

pub struct LspActor;

impl Actor for LspActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        // Spawn the single, global error listener
        tokio::spawn(listen_for_errors());

        // Spawn the server to accept client connections
        tokio::spawn(async {
            log::info!("LSP server started on 127.0.0.1:9090");
            let listener = tokio::net::TcpListener::bind("127.0.0.1:9090").await.unwrap();
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                log::info!("LSP client connected");
                let (read, write) = tokio::io::split(stream);

                let (service, socket) = LspService::new(|client| {
                    let id = CLIENT_COUNTER.fetch_add(1, Ordering::SeqCst);
                    ALL_CLIENTS.insert(id, client.clone());
                    Backend::new(client, id)
                });

                tokio::spawn(Server::new(read, write, socket).serve(service));
            }
        });
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        log::info!("LSP server actor stopped");
    }
}

// --- LSP Backend ---

#[derive(Debug)]
pub struct Backend {
    client: Client,
    client_id: usize,
}

impl Backend {
    pub fn new(client: Client, client_id: usize) -> Self {
        Self { client, client_id }
    }
}

async fn listen_for_errors() {
    let mut error_rx = crate::errors::ERROR_CHANNEL.subscribe();
    while let Ok(error_json) = error_rx.recv().await {
        if let Ok(error) = serde_json::from_str::<crate::errors::DetailedError>(&error_json) {
            let file_path = error.file_path.clone();

            let normalized_path = std::fs::canonicalize(&file_path)
                .ok()
                .and_then(|p| p.to_str().map(|s| s.to_string()))
                .unwrap_or(file_path.clone());

            let message = match &error.error_source {
                Some(crate::errors::ErrorSource::Python(py_err)) => py_err.message.clone(),
                Some(crate::errors::ErrorSource::Template(tmpl_err)) => tmpl_err.detail.clone(),
                None => error.message.clone(),
            };

            let diagnostic = Diagnostic {
                range: Range {
                    start: Position {
                        line: error.line.saturating_sub(1),
                        character: error.column.saturating_sub(1),
                    },
                    end: Position {
                        line: error.end_line.unwrap_or(error.line).saturating_sub(1),
                        character: error.end_column.unwrap_or(error.column),
                    },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                message,
                data: Some(serde_json::to_value(&error).unwrap()),
                ..Diagnostic::default()
            };

            match Url::from_file_path(&normalized_path) {
                Ok(uri) => {
                    for client in ALL_CLIENTS.iter() {
                        client
                            .publish_diagnostics(uri.clone(), vec![diagnostic.clone()], None)
                            .await;
                    }
                }
                Err(e) => {
                    log::error!("Failed to create URI from path {}: {:?}", normalized_path, e);
                }
            }
        }
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
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        log::info!("LSP server shutting down");
        ALL_CLIENTS.remove(&self.client_id);
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        if let Ok(file_path) = uri.to_file_path() {
            if let Some(normalized_path) = file_path.canonicalize().ok().and_then(|p| p.to_str().map(|s| s.to_string())) {
                log::debug!("File opened globally: {} -> {}", file_path.display(), normalized_path);
                let info = FileInfo {
                    uri,
                };
                OPEN_FILES.insert(normalized_path, info);
            }
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        if let Ok(file_path) = uri.to_file_path() {
            if let Some(normalized_path) = file_path.canonicalize().ok().and_then(|p| p.to_str().map(|s| s.to_string())) {
                log::debug!("File closed globally: {}", normalized_path);
                OPEN_FILES.remove(&normalized_path);
            }
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.client
            .publish_diagnostics(params.text_document.uri, vec![], None)
            .await;
    }
}
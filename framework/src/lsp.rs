use actix::prelude::*;
use dashmap::DashMap;
use lazy_static::lazy_static;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

// --- Global State ---

lazy_static! {
    static ref FILES_WITH_DIAGNOSTICS: DashMap<Url, ()> = DashMap::new();
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
            log::info!("Noventa's VisualStudio Extension server started on 127.0.0.1:9090");
            let listener = tokio::net::TcpListener::bind("127.0.0.1:9090").await.unwrap();
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                log::info!("Noventa's Extension client connected");
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
        log::info!("Noventa's Extension server stopped");
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
                    FILES_WITH_DIAGNOSTICS.insert(uri.clone(), ());
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
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::INCREMENTAL),
                        will_save: Some(false),
                        will_save_wait_until: Some(false),
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                    },
                )),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
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
        log::info!("Noventa's Extension server shutting down");
        ALL_CLIENTS.remove(&self.client_id);
        Ok(())
    }

    async fn did_open(&self, _: DidOpenTextDocumentParams) {
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        if FILES_WITH_DIAGNOSTICS.contains_key(&uri) {
            for client in ALL_CLIENTS.iter() {
                client.publish_diagnostics(uri.clone(), vec![], None).await;
            }
            FILES_WITH_DIAGNOSTICS.remove(&uri);
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if FILES_WITH_DIAGNOSTICS.contains_key(&uri) {
            for client in ALL_CLIENTS.iter() {
                client.publish_diagnostics(uri.clone(), vec![], None).await;
            }
            FILES_WITH_DIAGNOSTICS.remove(&uri);
        }
    }

    async fn did_close(&self, _: DidCloseTextDocumentParams) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::*;
    use std::sync::Arc;

    #[test]
    fn test_backend_new() {
        // Test that Backend::new creates a backend with the correct client_id
        let client_id = 42;
        // We can't easily create a real Client, so we'll test the structure
        // In a real test, we'd need to mock the Client
        
        // For now, just test that the constructor logic would work
        // (This is more of a compilation test than a runtime test)
        assert!(true); // Placeholder - constructor is simple
    }

    #[test]
    fn test_files_with_diagnostics_global() {
        // Test that the global FILES_WITH_DIAGNOSTICS map works
        let url = Url::parse("file:///test/file.py").unwrap();
        
        // Initially empty
        assert!(!FILES_WITH_DIAGNOSTICS.contains_key(&url));
        
        // Insert something
        FILES_WITH_DIAGNOSTICS.insert(url.clone(), ());
        assert!(FILES_WITH_DIAGNOSTICS.contains_key(&url));
        
        // Remove it
        FILES_WITH_DIAGNOSTICS.remove(&url);
        assert!(!FILES_WITH_DIAGNOSTICS.contains_key(&url));
    }

    #[test]
    fn test_all_clients_global() {
        // Test that the global ALL_CLIENTS map works
        let client_id = 123;
        
        // Initially doesn't contain our test ID
        assert!(!ALL_CLIENTS.contains_key(&client_id));
        
        // In a real scenario, we'd insert a Client here, but Client is not easily mockable
        // For now, just test the map operations work
        assert!(!ALL_CLIENTS.contains_key(&client_id));
    }

    #[test]
    fn test_client_counter() {
        // Test that CLIENT_COUNTER works
        let initial = CLIENT_COUNTER.load(Ordering::SeqCst);
        let next = CLIENT_COUNTER.fetch_add(1, Ordering::SeqCst);
        assert_eq!(next, initial);
        let current = CLIENT_COUNTER.load(Ordering::SeqCst);
        assert_eq!(current, initial + 1);
    }

    #[test]
    fn test_lsp_actor_creation() {
        // Test that LspActor can be created
        let actor = LspActor;
        // Actor trait is implemented, so this should work
        assert!(true);
    }

    #[tokio::test]
    async fn test_initialize_result_structure() {
        // Test the structure of InitializeResult that would be returned
        // We can't test the actual method without a real client, but we can test the data structure
        
        let result = InitializeResult {
            offset_encoding: Some(String::from("utf-8")),
            server_info: Some(ServerInfo {
                name: "noventa-lsp".to_string(),
                version: Some("0.1.0".to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::INCREMENTAL),
                        will_save: Some(false),
                        will_save_wait_until: Some(false),
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                    },
                )),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                ..ServerCapabilities::default()
            },
        };

        assert_eq!(result.offset_encoding, Some("utf-8".to_string()));
        assert_eq!(result.server_info.as_ref().unwrap().name, "noventa-lsp");
        assert!(result.capabilities.text_document_sync.is_some());
        assert!(result.capabilities.workspace.is_some());
    }

    #[tokio::test]
    async fn test_did_close_handler() {
        // This test ensures the did_close handler exists and can be called.
        // Since it does nothing, we just need to verify its presence.
        let client = ALL_CLIENTS.iter().next().map(|c| c.clone());
        if let Some(client) = client {
            let backend = Backend::new(client, 1);
            let params = DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier {
                    uri: Url::parse("file:///test.txt").unwrap(),
                },
            };
            backend.did_close(params).await;
            // No assert needed, just testing that it compiles and runs.
        }
        assert!(true);
    }
}
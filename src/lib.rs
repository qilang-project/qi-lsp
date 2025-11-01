//! Qi Language Server Protocol (LSP) Implementation
//!
//! This crate provides language server functionality for the Qi programming language,
//! enabling features like syntax highlighting, code completion, diagnostics, and more
//! in editors and IDEs that support the Language Server Protocol.

#![deny(missing_docs)]
#![warn(clippy::all)]

pub mod server;
pub mod document;
pub mod diagnostics;
pub mod completion;
pub mod hover;
pub mod definition;
pub mod references;
pub mod formatting;
pub mod workspace_symbols;
pub mod document_symbols;
pub mod rename;

use anyhow::Result;
use log::{debug, info, warn, error};
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{
    InitializeParams, InitializeResult, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind, CompletionOptions, HoverProviderCapability,
    PositionEncodingKind, OneOf,
};
use tokio::sync::RwLock;

use document::DocumentManager;
use diagnostics::DiagnosticsManager;

/// Main language server implementation
pub struct QiLanguageServer {
    /// Connection to the client
    connection: Connection,
    /// Document management
    documents: RwLock<DocumentManager>,
    /// Diagnostics management
    diagnostics: RwLock<DiagnosticsManager>,
    /// Server configuration
    config: ServerConfig,
}

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Root URI of the workspace
    root_uri: Option<String>,
    /// Client capabilities
    client_capabilities: Option<InitializeParams>,
}

impl QiLanguageServer {
    /// Create a new language server instance
    pub fn new(connection: Connection) -> Self {
        info!("Creating new Qi Language Server instance");

        Self {
            connection,
            documents: RwLock::new(DocumentManager::new()),
            diagnostics: RwLock::new(DiagnosticsManager::new()),
            config: ServerConfig {
                root_uri: None,
                client_capabilities: None,
            },
        }
    }

    /// Run the language server main loop
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting Qi Language Server");

        // Handle initialization
        let initialize_params = self.handle_initialize().await?;
        self.config.root_uri = initialize_params.root_uri.as_ref().map(|uri| uri.to_string());
        self.config.client_capabilities = Some(initialize_params.clone());

        // Send initialized notification
        self.send_initialized_notification().await?;

        info!("Language server initialized successfully");

        // Main message loop
        self.main_loop().await?;

        Ok(())
    }

    /// Handle the initialize request
    async fn handle_initialize(&mut self) -> Result<InitializeParams> {
        debug!("Waiting for initialize request");

        let (id, params) = self.connection.initialize_start()?;
        let initialize_params: InitializeParams = serde_json::from_value(params)
            .map_err(|e| anyhow::anyhow!("Failed to parse initialize params: {}", e))?;

        let server_capabilities = ServerCapabilities {
            notebook_document_sync: None,
            position_encoding: Some(PositionEncodingKind::UTF16),
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            completion_provider: Some(CompletionOptions {
                resolve_provider: Some(false),
                trigger_characters: Some(vec![
                    ".".to_string(),
                    "(".to_string(),
                    " ".to_string(),
                    "可".to_string(), // Chinese keywords trigger
                    "函".to_string(),
                    "变".to_string(),
                ]),
                work_done_progress_options: Default::default(),
                all_commit_characters: None,
                completion_item: None,
            }),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            definition_provider: Some(OneOf::Left(true)),
            references_provider: Some(OneOf::Left(true)),
            document_formatting_provider: Some(OneOf::Left(true)),
            workspace_symbol_provider: Some(OneOf::Left(true)),
            workspace: None,
            execute_command_provider: None,
            selection_range_provider: None,
            signature_help_provider: None,
            document_highlight_provider: None,
            document_symbol_provider: Some(OneOf::Left(true)),
            rename_provider: Some(OneOf::Left(true)),
            code_action_provider: None,
            code_lens_provider: None,
            document_link_provider: None,
            color_provider: None,
            folding_range_provider: None,
            declaration_provider: None,
            implementation_provider: None,
            type_definition_provider: None,
            document_on_type_formatting_provider: None,
            semantic_tokens_provider: None,
            call_hierarchy_provider: None,
            linked_editing_range_provider: None,
            inline_value_provider: None,
            inlay_hint_provider: None,
            diagnostic_provider: None,
            document_range_formatting_provider: None,
            experimental: None,
            moniker_provider: None,
        };

        let initialize_result = InitializeResult {
            capabilities: server_capabilities,
            server_info: Some(lsp_types::ServerInfo {
                name: "Qi Language Server".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        };

        self.connection.initialize_finish(id, serde_json::to_value(initialize_result)?)?;

        debug!("Initialize request handled");
        Ok(initialize_params)
    }

    /// Send initialized notification
    async fn send_initialized_notification(&self) -> Result<()> {
        let params = serde_json::json!({});
        self.connection.sender.send(Message::Notification(lsp_server::Notification {
            method: "initialized".to_string(),
            params,
        }))?;
        debug!("Sent initialized notification");
        Ok(())
    }

    /// Main message handling loop
    async fn main_loop(&mut self) -> Result<()> {
        info!("Starting main message loop");

        while let Ok(msg) = self.connection.receiver.recv() {
            match msg {
                Message::Request(request) => {
                    if let Err(e) = self.handle_request(request).await {
                        error!("Error handling request: {}", e);
                    }
                }
                Message::Notification(notification) => {
                    if let Err(e) = self.handle_notification(notification).await {
                        error!("Error handling notification: {}", e);
                    }
                }
                Message::Response(_) => {
                    debug!("Received unexpected response message");
                }
            }
        }

        info!("Message loop ended");
        Ok(())
    }

    /// Handle incoming requests
    async fn handle_request(&mut self, request: Request) -> Result<()> {
        debug!("Handling request: {}", request.method);

        match request.method.as_str() {
            "textDocument/completion" => {
                self.handle_completion_request(request).await?;
            }
            "textDocument/hover" => {
                self.handle_hover_request(request).await?;
            }
            "textDocument/definition" => {
                self.handle_definition_request(request).await?;
            }
            "textDocument/references" => {
                self.handle_references_request(request).await?;
            }
            "textDocument/formatting" => {
                self.handle_formatting_request(request).await?;
            }
            "workspace/symbol" => {
                self.handle_workspace_symbol_request(request).await?;
            }
            "textDocument/documentSymbol" => {
                self.handle_document_symbol_request(request).await?;
            }
            "textDocument/rename" => {
                self.handle_rename_request(request).await?;
            }
            "shutdown" => {
                debug!("Received shutdown request");
                // Send empty response to acknowledge shutdown
                let response = Response {
                    id: request.id,
                    result: Some(serde_json::Value::Null),
                    error: None,
                };
                self.connection.sender.send(Message::Response(response))?;
            }
            _ => {
                warn!("Unhandled request method: {}", request.method);
                // Send method not found error
                let error = lsp_server::ResponseError {
                    code: lsp_server::ErrorCode::MethodNotFound as i32,
                    message: format!("Method not found: {}", request.method),
                    data: None,
                };
                let response = Response {
                    id: request.id,
                    result: None,
                    error: Some(error),
                };
                self.connection.sender.send(Message::Response(response))?;
            }
        }

        Ok(())
    }

    /// Handle incoming notifications
    async fn handle_notification(&mut self, notification: lsp_server::Notification) -> Result<()> {
        debug!("Handling notification: {}", notification.method);

        match notification.method.as_str() {
            "textDocument/didOpen" => {
                self.handle_text_document_did_open(notification).await?;
            }
            "textDocument/didChange" => {
                self.handle_text_document_did_change(notification).await?;
            }
            "textDocument/didClose" => {
                self.handle_text_document_did_close(notification).await?;
            }
            "textDocument/didSave" => {
                self.handle_text_document_did_save(notification).await?;
            }
            "exit" => {
                info!("Received exit notification, shutting down");
                std::process::exit(0);
            }
            _ => {
                debug!("Unhandled notification method: {}", notification.method);
            }
        }

        Ok(())
    }

    // Request handlers
    async fn handle_completion_request(&self, request: Request) -> Result<()> {
        let documents = self.documents.read().await;
        completion::handle_completion(&self.connection, request, &*documents).await
    }

    async fn handle_hover_request(&self, request: Request) -> Result<()> {
        let documents = self.documents.read().await;
        hover::handle_hover(&self.connection, request, &*documents).await
    }

    async fn handle_definition_request(&self, request: Request) -> Result<()> {
        let documents = self.documents.read().await;
        definition::handle_definition(&self.connection, request, &*documents).await
    }

    async fn handle_references_request(&self, request: Request) -> Result<()> {
        let documents = self.documents.read().await;
        references::handle_references(&self.connection, request, &*documents).await
    }

    async fn handle_formatting_request(&self, request: Request) -> Result<()> {
        let documents = self.documents.read().await;
        formatting::handle_formatting(&self.connection, request, &*documents).await
    }

    async fn handle_workspace_symbol_request(&self, request: Request) -> Result<()> {
        let documents = self.documents.read().await;
        workspace_symbols::handle_workspace_symbols(&self.connection, request, &*documents).await
    }

    async fn handle_document_symbol_request(&self, request: Request) -> Result<()> {
        let documents = self.documents.read().await;
        document_symbols::handle_document_symbols(&self.connection, request, &*documents).await
    }

    async fn handle_rename_request(&self, request: Request) -> Result<()> {
        let documents = self.documents.read().await;
        rename::handle_rename(&self.connection, request, &*documents).await
    }

    // Notification handlers
    async fn handle_text_document_did_open(&self, notification: lsp_server::Notification) -> Result<()> {
        let params: lsp_types::DidOpenTextDocumentParams = serde_json::from_value(notification.params)?;

        let uri = params.text_document.uri.to_string();
        let text = params.text_document.text;
        let language_id = params.text_document.language_id;

        if language_id == "qi" {
            self.documents.write().await.open_document(&uri, text.clone());
            {
                let documents = self.documents.read().await;
                diagnostics::update_diagnostics(&self.connection, &uri, &text, &*documents).await?;
            }
            debug!("Opened document: {}", uri);
        }

        Ok(())
    }

    async fn handle_text_document_did_change(&self, notification: lsp_server::Notification) -> Result<()> {
        let params: lsp_types::DidChangeTextDocumentParams = serde_json::from_value(notification.params)?;

        let uri = params.text_document.uri.to_string();

        if let Some(changes) = params.content_changes.get(0) {
            self.documents.write().await.update_document(&uri, &changes.text);

            // Re-analyze and update diagnostics
            if let Some(document) = self.documents.read().await.get_document(&uri) {
                let content = document.rope.to_string();
                let documents = self.documents.read().await;
                diagnostics::update_diagnostics(&self.connection, &uri, &content, &*documents).await?;
                debug!("Updated document: {}", uri);
            }
        }

        Ok(())
    }

    async fn handle_text_document_did_close(&self, notification: lsp_server::Notification) -> Result<()> {
        let params: lsp_types::DidCloseTextDocumentParams = serde_json::from_value(notification.params)?;

        let uri = params.text_document.uri.to_string();
        self.documents.write().await.close_document(&uri);
        debug!("Closed document: {}", uri);

        Ok(())
    }

    async fn handle_text_document_did_save(&self, notification: lsp_server::Notification) -> Result<()> {
        let params: lsp_types::DidSaveTextDocumentParams = serde_json::from_value(notification.params)?;

        let uri = params.text_document.uri.to_string();
        debug!("Saved document: {}", uri);

        // Trigger diagnostics update on save
        if let Some(document) = self.documents.read().await.get_document(&uri) {
            let content = document.rope.to_string();
            let documents = self.documents.read().await;
            diagnostics::update_diagnostics(&self.connection, &uri, &content, &*documents).await?;
        }

        Ok(())
    }
}
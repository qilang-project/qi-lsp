//! Qi Language Server Entry Point
//!
//! This is the main entry point for the Qi Language Server.
//! It handles setup and configuration before starting the language server.

use anyhow::Result;
use env_logger::Env;
use log::{info, error};
use lsp_server::Connection;
use qi_lsp::QiLanguageServer;
use std::env;
use std::process;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    init_logging();

    info!("Starting Qi Language Server v{}", env!("CARGO_PKG_VERSION"));

    // Create connection to client
    let (connection, io_threads) = Connection::stdio();

    // Create and run language server
    let mut server = QiLanguageServer::new(connection);

    // Run the server
    if let Err(e) = server.run().await {
        error!("Language server error: {}", e);
        process::exit(1);
    }

    // Wait for IO threads to finish
    io_threads.join()?;

    info!("Qi Language Server shutdown complete");
    Ok(())
}

/// Initialize logging configuration
fn init_logging() {
    // Set default log level based on environment
    let default_level = if env::var("QI_LSP_DEBUG").is_ok() {
        "debug"
    } else {
        "info"
    };

    // Initialize logger with environment
    env_logger::Builder::from_env(Env::default().default_filter_or(default_level))
        .format_timestamp_secs()
        .init();
}
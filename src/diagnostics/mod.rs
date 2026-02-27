//! Diagnostics management for the Qi Language Server
//!
//! This module provides functionality for generating and publishing
//! diagnostics (errors, warnings, hints) for Qi source code.

#![allow(dead_code)]

use anyhow::Result;
use log::{debug, warn};
use lsp_server::{Connection, Message};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, PublishDiagnosticsParams, Position, Range,
    NumberOrString,
};
use qi_compiler::lexer::tokens::Span;
use qi_compiler::parser::ParseError;
use std::collections::HashMap;
use tokio::sync::RwLock;

use crate::document::DocumentManager;

pub mod semantic;

/// Diagnostics manager
#[derive(Debug)]
pub struct DiagnosticsManager {
    /// Cache of diagnostics per document
    diagnostics_cache: RwLock<HashMap<String, Vec<Diagnostic>>>,
}

impl DiagnosticsManager {
    /// Create a new diagnostics manager
    pub fn new() -> Self {
        Self {
            diagnostics_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Clear diagnostics cache
    pub async fn clear_cache(&self) {
        self.diagnostics_cache.write().await.clear();
    }

    /// Clear diagnostics for a specific document
    pub async fn clear_document_diagnostics(&self, uri: &str) {
        self.diagnostics_cache.write().await.remove(uri);
    }
}

impl Default for DiagnosticsManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Update and publish diagnostics for a document
pub async fn update_diagnostics(
    connection: &Connection,
    uri: &str,
    content: &str,
    document_manager: &DocumentManager,
) -> Result<()> {
    debug!("Updating diagnostics for document: {}", uri);

    let mut diagnostics = Vec::new();

    // Get parse errors from document manager
    if let Some(parse_errors) = document_manager.get_document_errors(uri) {
        for parse_error in parse_errors.iter() {
            if let Some(diagnostic) = convert_parse_error_to_diagnostic(parse_error, content) {
                diagnostics.push(diagnostic);
            }
        }
    }

    // Perform semantic analysis
    if let Some(ast) = document_manager.get_document_ast(uri) {
        semantic::analyze_semantics(uri, &ast, &mut diagnostics, document_manager);
    }

    // Publish diagnostics
    let params = PublishDiagnosticsParams {
        uri: uri.parse::<lsp_types::Uri>().map_err(|e| {
            warn!("Invalid URI '{}': {}", uri, e);
            e
        })?,
        diagnostics: diagnostics.clone(),
        version: None,
    };

    connection.sender.send(Message::Notification(lsp_server::Notification {
        method: "textDocument/publishDiagnostics".to_string(),
        params: serde_json::to_value(params)?,
    }))?;

    debug!("Published {} diagnostics for {}", diagnostics.len(), uri);
    Ok(())
}

/// Convert a parse error to an LSP diagnostic
fn convert_parse_error_to_diagnostic(
    parse_error: &ParseError,
    _content: &str,
) -> Option<Diagnostic> {
    // Extract line and column numbers from the error (1-based → 0-based conversion)
    let (line, col, message) = match parse_error {
        ParseError::UnexpectedToken(token_kind, line, col) => {
            (*line as u32, *col as u32, format!("意外的标记: {:?}", token_kind))
        }
        ParseError::ExpectedToken(expected, found, line, col) => {
            (*line as u32, *col as u32, format!("期望 {:?} 但找到 {:?}", expected, found))
        }
        ParseError::InvalidSyntax(msg, line, col) => {
            (*line as u32, *col as u32, format!("语法错误: {}", msg))
        }
        ParseError::UnterminatedExpression(line, col) => {
            (*line as u32, *col as u32, "未终止的表达式".to_string())
        }
        ParseError::InvalidFunctionDeclaration(line, col) => {
            (*line as u32, *col as u32, "无效的函数声明".to_string())
        }
        ParseError::InvalidVariableDeclaration(line, col) => {
            (*line as u32, *col as u32, "无效的变量声明".to_string())
        }
        ParseError::UnexpectedEof => (0, 0, "意外的文件结束".to_string()),
        ParseError::General(msg) => (0, 0, format!("解析错误: {}", msg)),
        ParseError::ParseFailed => (0, 0, "解析失败：语法错误".to_string()),
    };

    // Convert from 1-based to 0-based, and clamp to avoid underflow
    let line_0 = line.saturating_sub(1);
    let col_0 = col.saturating_sub(1);

    let diagnostic = Diagnostic {
        range: Range {
            start: Position { line: line_0, character: col_0 },
            end: Position { line: line_0, character: col_0 + 1 },
        },
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String("parse-error".to_string())),
        code_description: None,
        source: Some("qi-compiler".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    };

    Some(diagnostic)
}

/// Convert a span to an LSP range
fn span_to_range(span: &Span, content: &str) -> Option<Range> {
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return None;
    }

    // Find line and character positions from byte offsets
    let mut byte_offset = 0;
    let mut start_line = 0;
    let mut start_char = 0;
    let mut end_line = 0;
    let mut end_char = 0;

    for (line_idx, line) in lines.iter().enumerate() {
        let line_start = byte_offset;
        let line_end = byte_offset + line.len() + 1; // +1 for newline

        if span.start >= line_start && span.start < line_end {
            start_line = line_idx;
            start_char = span.start - line_start;
        }

        if span.end >= line_start && span.end <= line_end {
            end_line = line_idx;
            end_char = span.end - line_start;
            break;
        }

        byte_offset = line_end;
    }

    Some(Range {
        start: Position {
            line: start_line as u32,
            character: start_char as u32,
        },
        end: Position {
            line: end_line as u32,
            character: end_char as u32,
        },
    })
}

/// Create a diagnostic for a semantic error
pub fn create_semantic_diagnostic(
    message: &str,
    range: Range,
    severity: DiagnosticSeverity,
    code: &str,
) -> Diagnostic {
    Diagnostic {
        range,
        severity: Some(severity),
        code: Some(NumberOrString::String(code.to_string())),
        code_description: None,
        source: Some("qi-lsp".to_string()),
        message: message.to_string(),
        related_information: None,
        tags: None,
        data: None,
    }
}

/// Create a diagnostic for a warning
pub fn create_warning_diagnostic(
    message: &str,
    range: Range,
    code: &str,
) -> Diagnostic {
    create_semantic_diagnostic(message, range, DiagnosticSeverity::WARNING, code)
}

/// Create a diagnostic for an error
pub fn create_error_diagnostic(
    message: &str,
    range: Range,
    code: &str,
) -> Diagnostic {
    create_semantic_diagnostic(message, range, DiagnosticSeverity::ERROR, code)
}

/// Create a diagnostic for an information message
pub fn create_info_diagnostic(
    message: &str,
    range: Range,
    code: &str,
) -> Diagnostic {
    create_semantic_diagnostic(message, range, DiagnosticSeverity::INFORMATION, code)
}

/// Create a diagnostic for a hint
pub fn create_hint_diagnostic(
    message: &str,
    range: Range,
    code: &str,
) -> Diagnostic {
    create_semantic_diagnostic(message, range, DiagnosticSeverity::HINT, code)
}
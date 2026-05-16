//! Document formatting functionality for the Qi Language Server
//!
//! This module provides code formatting capabilities for Qi source code,
//! ensuring consistent style and formatting according to defined conventions.

use anyhow::Result;
use log::{debug, warn};
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{
    DocumentFormattingParams, DocumentRangeFormattingParams, FormattingOptions, TextEdit, Range,
    Position,
};

use crate::document::DocumentManager;

/// Handle document formatting requests
pub async fn handle_formatting(
    connection: &Connection,
    request: Request,
    document_manager: &DocumentManager,
) -> Result<()> {
    debug!("Handling document formatting request");

    let params: DocumentFormattingParams = serde_json::from_value(request.params)?;
    let uri = params.text_document.uri.to_string();
    let options = params.options;

    // Get the current document content
    let content = match document_manager.get_document_content(&uri) {
        Some(content) => content,
        None => {
            warn!("Document not found for formatting: {}", uri);
            let response = Response {
                id: request.id,
                result: Some(serde_json::Value::Null),
                error: None,
            };
            connection.sender.send(Message::Response(response))?;
            return Ok(());
        }
    };

    // Format the content
    let formatted_content = format_qi_code(&content, &options);

    // Create text edit to replace entire document
    let text_edit = TextEdit::new(
        Range {
            start: Position { line: 0, character: 0 },
            end: Position {
                line: u32::MAX,
                character: u32::MAX,
            },
        },
        formatted_content,
    );

    let response = Response {
        id: request.id,
        result: Some(serde_json::to_value(vec![text_edit])?),
        error: None,
    };

    connection.sender.send(Message::Response(response))?;
    debug!("Sent formatting response");

    Ok(())
}

/// Format Qi source code according to style conventions
fn format_qi_code(content: &str, options: &FormattingOptions) -> String {
    debug!("Formatting Qi code using qi_tools formatter");

    // Use the official Qi tools formatter instead of duplicating logic
    let mut config = qi_tools::formatter::FormatConfig::default();
    config.indent_size = options.tab_size as usize;
    config.use_tabs = !options.insert_spaces;

    let formatter = qi_tools::formatter::Formatter::with_config(config);

    match formatter.format_file(content) {
        Ok(formatted) => {
            debug!("Formatting successful");
            formatted
        }
        Err(e) => {
            warn!("Formatting failed: {}, returning original content", e);
            content.to_string()
        }
    }
}

/// Handle document range formatting requests.
///
/// Strategy: the underlying qi_tools formatter is whole-file. To format a range
/// we extract the substring, format it standalone, and emit a TextEdit replacing
/// only that range. This keeps the user's selection intact rather than nuking
/// the whole document on range-format.
pub async fn handle_range_formatting(
    connection: &Connection,
    request: Request,
    document_manager: &DocumentManager,
) -> Result<()> {
    debug!("Handling document range formatting request");

    let params: DocumentRangeFormattingParams = serde_json::from_value(request.params)?;
    let uri = params.text_document.uri.to_string();
    let options = params.options;
    let range = params.range;

    let content = match document_manager.get_document_content(&uri) {
        Some(c) => c,
        None => {
            warn!("Document not found for range formatting: {}", uri);
            let response = Response {
                id: request.id,
                result: Some(serde_json::Value::Null),
                error: None,
            };
            connection.sender.send(Message::Response(response))?;
            return Ok(());
        }
    };

    let Some(sub) = slice_by_range(&content, range) else {
        let response = Response {
            id: request.id,
            result: Some(serde_json::to_value(Vec::<TextEdit>::new())?),
            error: None,
        };
        connection.sender.send(Message::Response(response))?;
        return Ok(());
    };

    let formatted = format_qi_code(&sub, &options);

    let text_edit = TextEdit::new(range, formatted);
    let response = Response {
        id: request.id,
        result: Some(serde_json::to_value(vec![text_edit])?),
        error: None,
    };
    connection.sender.send(Message::Response(response))?;
    Ok(())
}

fn slice_by_range(content: &str, range: Range) -> Option<String> {
    let start = position_to_byte_offset(content, range.start)?;
    let end = position_to_byte_offset(content, range.end)?;
    if end < start || end > content.len() {
        return None;
    }
    Some(content[start..end].to_string())
}

fn position_to_byte_offset(content: &str, pos: Position) -> Option<usize> {
    let mut line = 0u32;
    let mut col_bytes = 0u32;
    for (i, c) in content.char_indices() {
        if line == pos.line && col_bytes == pos.character {
            return Some(i);
        }
        if c == '\n' {
            line += 1;
            col_bytes = 0;
            if line == pos.line && col_bytes == pos.character {
                return Some(i + 1);
            }
        } else {
            col_bytes += c.len_utf8() as u32;
        }
    }
    if line == pos.line && col_bytes == pos.character {
        Some(content.len())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_extracts_simple_range() {
        let src = "包 主程序;\n变量 x: 整数 = 1;\n";
        let line1_bytes = "变量 x: 整数 = 1;".len();
        let r = Range {
            start: Position { line: 1, character: 0 },
            end: Position { line: 1, character: line1_bytes as u32 },
        };
        let s = slice_by_range(src, r).expect("slice");
        assert_eq!(s, "变量 x: 整数 = 1;");
    }

    #[test]
    fn slice_out_of_bounds_returns_none() {
        let src = "x\n";
        let r = Range {
            start: Position { line: 5, character: 0 },
            end: Position { line: 5, character: 1 },
        };
        assert!(slice_by_range(src, r).is_none());
    }

    #[test]
    fn position_to_byte_offset_handles_chinese() {
        let src = "变量\n";
        let off = position_to_byte_offset(src, Position { line: 0, character: 3 }).unwrap();
        assert_eq!(off, 3);
    }
}

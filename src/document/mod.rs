//! Document management for the Qi Language Server
//!
//! This module provides functionality for managing text documents,
//! including parsing, AST management, and document state tracking.

use dashmap::DashMap;
use log::{debug, warn};
use qi_compiler::parser::{Parser, Program, ParseError};
use ropey::Rope;
use std::sync::Arc;

/// Document manager that handles all open documents
pub struct DocumentManager {
    /// Map of document URI to document state
    documents: Arc<DashMap<String, Document>>,
    /// Shared parser instance
    parser: Arc<Parser>,
}

/// Represents a single document in the editor
#[derive(Debug, Clone)]
pub struct Document {
    /// Document URI
    pub uri: String,
    /// Document content as a rope for efficient editing
    pub rope: Rope,
    /// Parsed AST (cached)
    pub ast: Option<Arc<Program>>,
    /// Parse errors (cached)
    pub parse_errors: Option<Arc<Vec<ParseError>>>,
    /// Version of the document (for change tracking)
    pub version: i32,
    /// Language ID
    pub language_id: String,
}

/// Position in a document
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentPosition {
    /// Line number (0-based)
    pub line: usize,
    /// Character offset in line (0-based)
    pub character: usize,
}

/// Range in a document
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentRange {
    /// Start position
    pub start: DocumentPosition,
    /// End position
    pub end: DocumentPosition,
}

impl DocumentManager {
    /// Create a new document manager
    pub fn new() -> Self {
        Self {
            documents: Arc::new(DashMap::new()),
            parser: Arc::new(Parser::new()),
        }
    }

    /// Open a document and parse it
    pub fn open_document(&self, uri: &str, content: String) {
        debug!("Opening document: {}", uri);

        let rope = Rope::from_str(&content);
        let (ast, parse_errors) = self.parse_content(&content);

        let document = Document {
            uri: uri.to_string(),
            rope,
            ast,
            parse_errors,
            version: 1,
            language_id: "qi".to_string(),
        };

        self.documents.insert(uri.to_string(), document);
        debug!("Document opened and parsed: {}", uri);
    }

    /// Update document content
    pub fn update_document(&self, uri: &str, content: &str) {
        debug!("Updating document: {}", uri);

        if let Some(mut doc) = self.documents.get_mut(uri) {
            doc.rope = Rope::from_str(content);
            doc.version += 1;

            // Re-parse document
            let (ast, parse_errors) = self.parse_content(content);
            doc.ast = ast;
            doc.parse_errors = parse_errors;

            debug!("Document updated and re-parsed: {}", uri);
        } else {
            warn!("Attempted to update non-existent document: {}", uri);
        }
    }

    /// Close a document
    pub fn close_document(&self, uri: &str) {
        debug!("Closing document: {}", uri);
        self.documents.remove(uri);
    }

    /// Get a document by URI
    pub fn get_document(&self, uri: &str) -> Option<Document> {
        self.documents.get(uri).map(|doc| doc.clone())
    }

    /// Get document content as string
    pub fn get_document_content(&self, uri: &str) -> Option<String> {
        self.documents.get(uri).map(|doc| doc.rope.to_string())
    }

    /// Get document AST if available
    pub fn get_document_ast(&self, uri: &str) -> Option<Arc<Program>> {
        self.documents.get(uri).and_then(|doc| doc.ast.clone())
    }

    /// Get document parse errors if available
    pub fn get_document_errors(&self, uri: &str) -> Option<Arc<Vec<ParseError>>> {
        self.documents.get(uri).and_then(|doc| doc.parse_errors.clone())
    }

    /// Check if a document is managed
    pub fn has_document(&self, uri: &str) -> bool {
        self.documents.contains_key(uri)
    }

    /// Get all document URIs
    pub fn get_all_uris(&self) -> Vec<String> {
        self.documents.iter().map(|entry| entry.key().clone()).collect()
    }

    /// Parse content and return AST and errors
    fn parse_content(&self, content: &str) -> (Option<Arc<Program>>, Option<Arc<Vec<ParseError>>>) {
        match self.parser.parse_source(content) {
            Ok(program) => (Some(Arc::new(program)), None),
            Err(error) => {
                // For now, return the single error. In the future, we might want to collect
                // multiple errors during parsing
                (None, Some(Arc::new(vec![error])))
            }
        }
    }

    /// Convert line/column to byte offset
    pub fn position_to_offset(&self, uri: &str, position: DocumentPosition) -> Option<usize> {
        self.documents.get(uri).and_then(|doc| {
            if position.line < doc.rope.len_lines() {
                let line_start = doc.rope.line_to_char(position.line);
                let line_end = doc.rope.line_to_char(position.line + 1);
                let line_len = line_end - line_start;

                if position.character <= line_len {
                    Some(line_start + position.character)
                } else {
                    None
                }
            } else {
                None
            }
        })
    }

    /// Convert byte offset to line/column
    pub fn offset_to_position(&self, uri: &str, offset: usize) -> Option<DocumentPosition> {
        self.documents.get(uri).and_then(|doc| {
            if offset <= doc.rope.len_chars() {
                let line = doc.rope.char_to_line(offset);
                let line_start = doc.rope.line_to_char(line);
                let character = offset - line_start;

                Some(DocumentPosition { line, character })
            } else {
                None
            }
        })
    }

    /// Get line content
    pub fn get_line_content(&self, uri: &str, line: usize) -> Option<String> {
        self.documents.get(uri).and_then(|doc| {
            if line < doc.rope.len_lines() {
                Some(doc.rope.line(line).to_string())
            } else {
                None
            }
        })
    }

    /// Get text in a range
    pub fn get_range_text(&self, uri: &str, range: DocumentRange) -> Option<String> {
        self.documents.get(uri).and_then(|doc| {
            let start_offset = self.position_to_offset(uri, range.start)?;
            let end_offset = self.position_to_offset(uri, range.end)?;

            if start_offset <= end_offset && end_offset <= doc.rope.len_chars() {
                Some(doc.rope.slice(start_offset..end_offset).to_string())
            } else {
                None
            }
        })
    }
}

impl Default for DocumentManager {
    fn default() -> Self {
        Self::new()
    }
}

impl From<lsp_types::Position> for DocumentPosition {
    fn from(pos: lsp_types::Position) -> Self {
        Self {
            line: pos.line as usize,
            character: pos.character as usize,
        }
    }
}

impl From<DocumentPosition> for lsp_types::Position {
    fn from(pos: DocumentPosition) -> Self {
        Self {
            line: pos.line as u32,
            character: pos.character as u32,
        }
    }
}

impl From<lsp_types::Range> for DocumentRange {
    fn from(range: lsp_types::Range) -> Self {
        Self {
            start: range.start.into(),
            end: range.end.into(),
        }
    }
}

impl From<DocumentRange> for lsp_types::Range {
    fn from(range: DocumentRange) -> Self {
        Self {
            start: range.start.into(),
            end: range.end.into(),
        }
    }
}
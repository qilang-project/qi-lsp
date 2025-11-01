//! Document formatting functionality for the Qi Language Server
//!
//! This module provides code formatting capabilities for Qi source code,
//! ensuring consistent style and formatting according to defined conventions.

use anyhow::Result;
use log::{debug, warn};
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{
    DocumentFormattingParams, FormattingOptions, TextEdit, Range, Position,
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
    debug!("Formatting Qi code with {} spaces per indent", options.tab_size);

    let mut formatter = QiFormatter::new(options);
    formatter.format(content)
}

/// Qi code formatter
struct QiFormatter {
    /// Formatting options
    options: FormattingOptions,
    /// Current indentation level
    indent_level: usize,
    /// Output buffer
    output: String,
    /// Current line content
    current_line: String,
    /// Whether we're at the beginning of a line
    line_start: bool,
    /// Previous token type for context
    prev_token: TokenType,
    /// Current context (e.g., inside function, block, etc.)
    context: Vec<FormattingContext>,
}

/// Token types for formatting decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenType {
    Keyword,
    Identifier,
    Symbol,
    String,
    Comment,
    Whitespace,
    Newline,
    Unknown,
}

/// Formatting context
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormattingContext {
    TopLevel,
    Function,
    Block,
    Struct,
    Enum,
    ControlFlow,
}

impl QiFormatter {
    /// Create a new formatter
    fn new(options: &FormattingOptions) -> Self {
        Self {
            options: options.clone(),
            indent_level: 0,
            output: String::new(),
            current_line: String::new(),
            line_start: true,
            prev_token: TokenType::Unknown,
            context: vec![FormattingContext::TopLevel],
        }
    }

    /// Format the input content
    fn format(&mut self, content: &str) -> String {
        self.output.clear();
        self.current_line.clear();
        self.indent_level = 0;
        self.line_start = true;
        self.prev_token = TokenType::Unknown;
        self.context = vec![FormattingContext::TopLevel];

        // Tokenize and format
        self.tokenize_and_format(content);

        // Flush any remaining content
        self.flush_line();

        self.output.clone()
    }

    /// Tokenize the content and apply formatting rules
    fn tokenize_and_format(&mut self, content: &str) {
        let mut chars = content.chars().peekable();
        let mut current_token = String::new();

        while let Some(ch) = chars.next() {
            match ch {
                // Whitespace
                ' ' | '\t' => {
                    if !current_token.is_empty() {
                        self.process_token(&current_token);
                        current_token.clear();
                    }
                    self.handle_whitespace(ch);
                }
                // Newline
                '\n' => {
                    if !current_token.is_empty() {
                        self.process_token(&current_token);
                        current_token.clear();
                    }
                    self.handle_newline();
                }
                // String literals
                '"' => {
                    if !current_token.is_empty() {
                        self.process_token(&current_token);
                        current_token.clear();
                    }
                    current_token.push(ch);
                    self.parse_string(&mut chars, &mut current_token);
                    self.process_token(&current_token);
                    current_token.clear();
                }
                // Comments
                '/' => {
                    if let Some(&next_ch) = chars.peek() {
                        if next_ch == '/' {
                            if !current_token.is_empty() {
                                self.process_token(&current_token);
                                current_token.clear();
                            }
                            current_token.push(ch);
                            current_token.push(chars.next().unwrap());
                            self.parse_line_comment(&mut chars, &mut current_token);
                            self.process_token(&current_token);
                            current_token.clear();
                            continue;
                        } else if next_ch == '*' {
                            if !current_token.is_empty() {
                                self.process_token(&current_token);
                                current_token.clear();
                            }
                            current_token.push(ch);
                            current_token.push(chars.next().unwrap());
                            self.parse_block_comment(&mut chars, &mut current_token);
                            self.process_token(&current_token);
                            current_token.clear();
                            continue;
                        }
                    }
                    current_token.push(ch);
                }
                // Symbols that typically standalone
                ';' | '；' | '{' | '}' | '(' | ')' | '【' | '】' | '（' | '）' => {
                    if !current_token.is_empty() {
                        self.process_token(&current_token);
                        current_token.clear();
                    }
                    current_token.push(ch);
                    self.process_token(&current_token);
                    current_token.clear();
                }
                // Regular characters (part of identifiers or keywords)
                _ => {
                    current_token.push(ch);
                }
            }
        }

        // Process final token
        if !current_token.is_empty() {
            self.process_token(&current_token);
        }
    }

    /// Parse a string literal
    fn parse_string(&mut self, chars: &mut std::iter::Peekable<std::str::Chars>, current_token: &mut String) {
        while let Some(ch) = chars.next() {
            current_token.push(ch);
            if ch == '"' {
                break;
            } else if ch == '\\' {
                // Handle escaped characters
                if let Some(next_ch) = chars.next() {
                    current_token.push(next_ch);
                }
            }
        }
    }

    /// Parse a line comment
    fn parse_line_comment(&mut self, chars: &mut std::iter::Peekable<std::str::Chars>, current_token: &mut String) {
        while let Some(ch) = chars.next() {
            current_token.push(ch);
            if ch == '\n' {
                break;
            }
        }
    }

    /// Parse a block comment
    fn parse_block_comment(&mut self, chars: &mut std::iter::Peekable<std::str::Chars>, current_token: &mut String) {
        while let Some(ch) = chars.next() {
            current_token.push(ch);
            if ch == '*' {
                if let Some(&next_ch) = chars.peek() {
                    if next_ch == '/' {
                        current_token.push(chars.next().unwrap());
                        break;
                    }
                }
            }
        }
    }

    /// Process a token according to formatting rules
    fn process_token(&mut self, token: &str) {
        let token_type = self.classify_token(token);

        match token_type {
            TokenType::Keyword => {
                self.handle_keyword(token);
            }
            TokenType::Identifier => {
                self.handle_identifier(token);
            }
            TokenType::Symbol => {
                self.handle_symbol(token);
            }
            TokenType::String | TokenType::Comment => {
                self.handle_literal(token);
            }
            TokenType::Whitespace => {
                // Skip explicit whitespace handling (handled in tokenize_and_format)
            }
            TokenType::Newline => {
                self.handle_newline();
            }
            TokenType::Unknown => {
                self.handle_unknown(token);
            }
        }

        self.prev_token = token_type;
    }

    /// Classify a token type
    fn classify_token(&self, token: &str) -> TokenType {
        if token.is_empty() {
            return TokenType::Unknown;
        }

        match token.chars().next().unwrap() {
            ' ' | '\t' => TokenType::Whitespace,
            '\n' => TokenType::Newline,
            '"' => TokenType::String,
            '/' if token.starts_with("//") || token.starts_with("/*") => TokenType::Comment,
            ';' | '；' | '{' | '}' | '(' | ')' | '[' | ']' | '【' | '】' | '（' | '）' | ',' | '，' | ':' | '=' | '+' | '-' | '*' | '/' | '%' | '<' | '>' | '!' | '&' | '|' | '^' => TokenType::Symbol,
            _ => {
                if self.is_keyword(token) {
                    TokenType::Keyword
                } else if self.is_identifier(token) {
                    TokenType::Identifier
                } else {
                    TokenType::Unknown
                }
            }
        }
    }

    /// Check if a token is a Qi keyword
    fn is_keyword(&self, token: &str) -> bool {
        matches!(token,
            "包" | "函数" | "异步函数" | "变量" | "如果" | "否则" | "当" | "对于" | "循环" | "返回" |
            "结构体" | "枚举" | "公开" | "私有" | "可变" | "不可变" | "匹配" | "导入" | "打印" | "等待" |
            "整数" | "长整数" | "短整数" | "字节" | "浮点数" | "布尔" | "字符" | "字符串" | "空" |
            "数组" | "字典" | "列表" | "集合" | "指针" | "引用" | "可变引用"
        )
    }

    /// Check if a token is a valid identifier
    fn is_identifier(&self, token: &str) -> bool {
        !token.is_empty() && (token.chars().next().unwrap().is_alphabetic() || token.chars().next().unwrap() == '_') &&
        token.chars().all(|ch| ch.is_alphanumeric() || ch == '_' || (ch as u32 >= 0x4E00 && ch as u32 <= 0x9FFF))
    }

    /// Handle keywords
    fn handle_keyword(&mut self, keyword: &str) {
        match keyword {
            "包" => {
                self.append_to_line(keyword);
                self.append_to_line(" ");
            }
            "函数" | "异步函数" => {
                if self.line_start {
                    self.append_indent();
                }
                self.append_to_line(keyword);
                self.append_to_line(" ");
                self.context.push(FormattingContext::Function);
            }
            "如果" | "当" | "对于" | "循环" => {
                if self.line_start {
                    self.append_indent();
                }
                self.append_to_line(keyword);
                self.append_to_line(" ");
                self.context.push(FormattingContext::ControlFlow);
            }
            "否则" => {
                self.append_to_line(" ");
                self.append_to_line(keyword);
            }
            "{" | "【" | "（" => {
                self.append_to_line(&format!(" {}", keyword));
                self.increase_indent();
                self.newline();
            }
            "}" | "】" | "）" => {
                self.decrease_indent();
                if !self.line_start {
                    self.newline();
                }
                self.append_indent();
                self.append_to_line(keyword);
                self.context.pop();
            }
            ";" | "；" => {
                self.append_to_line(keyword);
                self.newline();
            }
            _ => {
                self.append_to_line(keyword);
                self.append_to_line(" ");
            }
        }
    }

    /// Handle identifiers
    fn handle_identifier(&mut self, identifier: &str) {
        if self.line_start {
            self.append_indent();
        }
        self.append_to_line(identifier);
    }

    /// Handle symbols
    fn handle_symbol(&mut self, symbol: &str) {
        match symbol {
            "(" | "（" => {
                self.append_to_line(symbol);
            }
            ")" | "）" => {
                self.append_to_line(symbol);
            }
            "{" | "【" => {
                self.append_to_line(symbol);
            }
            "}" | "】" => {
                self.append_to_line(symbol);
            }
            ":" => {
                self.append_to_line(": ");
            }
            "=" => {
                self.append_to_line(" = ");
            }
            ";" | "；" => {
                self.append_to_line(symbol);
                self.newline();
            }
            "," | "，" => {
                self.append_to_line(symbol);
                self.append_to_line(" ");
            }
            "." | "。" => {
                self.append_to_line(symbol);
            }
            _ => {
                self.append_to_line(symbol);
            }
        }
    }

    /// Handle literals (strings, comments)
    fn handle_literal(&mut self, literal: &str) {
        if self.line_start && literal.starts_with("//") {
            self.append_indent();
        }
        self.append_to_line(literal);
        if literal.starts_with("//") {
            self.newline();
        }
    }

    /// Handle unknown tokens
    fn handle_unknown(&mut self, token: &str) {
        if self.line_start {
            self.append_indent();
        }
        self.append_to_line(token);
    }

    /// Handle whitespace
    fn handle_whitespace(&mut self, ch: char) {
        if self.options.insert_spaces {
            // Convert tabs to spaces
            if ch == '\t' {
                self.append_to_line(&" ".repeat(self.options.tab_size as usize));
            } else {
                self.append_to_line(" ");
            }
        } else {
            self.append_to_line(&ch.to_string());
        }
    }

    /// Handle newline
    fn handle_newline(&mut self) {
        self.newline();
    }

    /// Append text to current line
    fn append_to_line(&mut self, text: &str) {
        self.current_line.push_str(text);
        self.line_start = false;
    }

    /// Append current indentation
    fn append_indent(&mut self) {
        if self.options.insert_spaces {
            let spaces = self.indent_level * self.options.tab_size as usize;
            self.current_line.push_str(&" ".repeat(spaces));
        } else {
            self.current_line.push_str(&"\t".repeat(self.indent_level));
        }
    }

    /// Start a new line
    fn newline(&mut self) {
        self.flush_line();
        self.line_start = true;
    }

    /// Flush current line to output
    fn flush_line(&mut self) {
        let trimmed = self.current_line.trim_end();
        if !trimmed.is_empty() || !self.output.is_empty() {
            self.output.push_str(trimmed);
            self.output.push('\n');
        }
        self.current_line.clear();
    }

    /// Increase indentation level
    fn increase_indent(&mut self) {
        self.indent_level += 1;
    }

    /// Decrease indentation level
    fn decrease_indent(&mut self) {
        if self.indent_level > 0 {
            self.indent_level -= 1;
        }
    }
}
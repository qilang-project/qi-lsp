//! Basic semantic tokens provider for Qi source.
//!
//! Scans the raw document and classifies each token into one of the
//! capability-declared types (KEYWORD/TYPE/FUNCTION/VARIABLE/STRING/NUMBER/COMMENT).
//! Token indices must match the order declared in `SemanticTokensLegend` at the
//! lib.rs server-capability site.

use anyhow::Result;
use log::{debug, warn};
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{SemanticTokens, SemanticTokensParams, SemanticTokensResult};

use crate::document::DocumentManager;

const TOKEN_KEYWORD: u32 = 0;
const TOKEN_TYPE: u32 = 1;
const TOKEN_FUNCTION: u32 = 2;
const TOKEN_VARIABLE: u32 = 3;
const TOKEN_STRING: u32 = 4;
const TOKEN_NUMBER: u32 = 5;
const TOKEN_COMMENT: u32 = 6;

/// Handle textDocument/semanticTokens/full.
pub async fn handle_semantic_tokens_full(
    connection: &Connection,
    request: Request,
    document_manager: &DocumentManager,
) -> Result<()> {
    debug!("Handling semanticTokens/full request");

    let params: SemanticTokensParams = serde_json::from_value(request.params)?;
    let uri = params.text_document.uri.to_string();

    let tokens = match document_manager.get_document_content(&uri) {
        Some(content) => compute_tokens(&content),
        None => {
            warn!("Document not found for semantic tokens: {}", uri);
            Vec::new()
        }
    };

    let result = SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data: tokens,
    });

    let response = Response {
        id: request.id,
        result: Some(serde_json::to_value(result)?),
        error: None,
    };
    connection.sender.send(Message::Response(response))?;
    Ok(())
}

/// Raw token before delta-encoding.
#[derive(Debug, Clone, Copy)]
struct RawToken {
    line: u32,
    start_char: u32,
    length: u32,
    token_type: u32,
}

/// Compute delta-encoded semantic tokens for the document.
pub fn compute_tokens(source: &str) -> Vec<lsp_types::SemanticToken> {
    let raw = scan(source);

    // Delta-encode per LSP spec: tokens are sorted by (line, start_char),
    // each emitted as (delta_line, delta_start, length, type, modifiers).
    let mut out = Vec::with_capacity(raw.len());
    let mut prev_line: u32 = 0;
    let mut prev_start: u32 = 0;
    for t in &raw {
        let delta_line = t.line - prev_line;
        let delta_start = if delta_line == 0 {
            t.start_char - prev_start
        } else {
            t.start_char
        };
        out.push(lsp_types::SemanticToken {
            delta_line,
            delta_start,
            length: t.length,
            token_type: t.token_type,
            token_modifiers_bitset: 0,
        });
        prev_line = t.line;
        prev_start = t.start_char;
    }
    out
}

fn scan(source: &str) -> Vec<RawToken> {
    let mut tokens = Vec::new();
    let mut line: u32 = 0;
    let mut col: u32 = 0;
    let mut chars = source.chars().peekable();
    let mut prev_was_func_kw = false; // 函数 NAME — mark NAME as FUNCTION

    while let Some(ch) = chars.next() {
        match ch {
            '\n' => {
                line += 1;
                col = 0;
            }
            ' ' | '\t' | '\r' => {
                col += ch.len_utf16() as u32;
            }
            '/' if chars.peek() == Some(&'/') => {
                let start = col;
                let mut len = 2u32;
                chars.next();
                while let Some(&c) = chars.peek() {
                    if c == '\n' {
                        break;
                    }
                    chars.next();
                    len += c.len_utf16() as u32;
                }
                tokens.push(RawToken {
                    line,
                    start_char: start,
                    length: len,
                    token_type: TOKEN_COMMENT,
                });
                col += len;
            }
            '"' => {
                let start = col;
                let mut len = 1u32;
                col += 1;
                let start_line = line;
                while let Some(c) = chars.next() {
                    if c == '\\' {
                        if let Some(next) = chars.next() {
                            len += 1 + next.len_utf16() as u32;
                            col += 1 + next.len_utf16() as u32;
                            if next == '\n' {
                                line += 1;
                                col = 0;
                            }
                            continue;
                        }
                    }
                    if c == '\n' {
                        line += 1;
                        col = 0;
                        len += 1;
                        continue;
                    }
                    len += c.len_utf16() as u32;
                    col += c.len_utf16() as u32;
                    if c == '"' {
                        break;
                    }
                }
                // String tokens spanning multiple lines: emit only on start_line
                // (delta-encoding makes multi-line spans impossible to express anyway).
                tokens.push(RawToken {
                    line: start_line,
                    start_char: start,
                    length: len.min(u32::MAX),
                    token_type: TOKEN_STRING,
                });
            }
            c if c.is_ascii_digit() => {
                let start = col;
                let mut len = c.len_utf16() as u32;
                while let Some(&n) = chars.peek() {
                    if n.is_ascii_digit() || n == '.' || n == '_' {
                        chars.next();
                        len += n.len_utf16() as u32;
                    } else {
                        break;
                    }
                }
                tokens.push(RawToken {
                    line,
                    start_char: start,
                    length: len,
                    token_type: TOKEN_NUMBER,
                });
                col += len;
            }
            c if is_ident_start(c) => {
                let start = col;
                let mut word = String::new();
                word.push(c);
                let mut len = c.len_utf16() as u32;
                while let Some(&n) = chars.peek() {
                    if is_ident_continue(n) {
                        word.push(n);
                        chars.next();
                        len += n.len_utf16() as u32;
                    } else {
                        break;
                    }
                }
                let token_type = if is_keyword(&word) {
                    if word == "函数" || word == "异步函数" {
                        prev_was_func_kw = true;
                    }
                    TOKEN_KEYWORD
                } else if is_builtin_type(&word) {
                    TOKEN_TYPE
                } else if prev_was_func_kw {
                    prev_was_func_kw = false;
                    TOKEN_FUNCTION
                } else {
                    TOKEN_VARIABLE
                };
                tokens.push(RawToken {
                    line,
                    start_char: start,
                    length: len,
                    token_type,
                });
                col += len;
            }
            _ => {
                col += ch.len_utf16() as u32;
            }
        }
    }

    tokens
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || is_cjk(c)
}

fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || is_cjk(c)
}

fn is_cjk(c: char) -> bool {
    let u = c as u32;
    (0x4E00..=0x9FFF).contains(&u)
}

fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "包" | "导入" | "公开" | "私有" | "作为"
        | "函数" | "异步函数" | "返回" | "等待"
        | "变量" | "常量"
        | "如果" | "否则" | "当" | "对于" | "循环" | "匹配"
        | "结构体" | "枚举" | "类型"
        | "真" | "假" | "空"
    )
}

fn is_builtin_type(word: &str) -> bool {
    matches!(
        word,
        "整数" | "长整数" | "短整数" | "字节" | "浮点数" | "布尔"
        | "字符" | "字符串" | "数组" | "字典" | "列表" | "集合"
        | "指针" | "引用" | "可变引用" | "未来"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token_kinds(source: &str) -> Vec<u32> {
        scan(source).iter().map(|t| t.token_type).collect()
    }

    #[test]
    fn classifies_keyword_type_string_number_comment() {
        let src = "// hi\n变量 x: 整数 = 42; 变量 s: 字符串 = \"hello\";";
        let kinds = token_kinds(src);
        assert!(kinds.contains(&TOKEN_COMMENT));
        assert!(kinds.contains(&TOKEN_KEYWORD));
        assert!(kinds.contains(&TOKEN_TYPE));
        assert!(kinds.contains(&TOKEN_STRING));
        assert!(kinds.contains(&TOKEN_NUMBER));
    }

    #[test]
    fn function_name_after_keyword_is_function_token() {
        let src = "函数 计算(x: 整数) : 整数 { 返回 x; }";
        let raw = scan(src);
        // 计算 should be classified FUNCTION (right after 函数)
        let 计算 = raw
            .iter()
            .find(|t| t.start_char == 3)
            .expect("token at col 3");
        assert_eq!(计算.token_type, TOKEN_FUNCTION);
    }

    #[test]
    fn delta_encoding_first_token_uses_absolute_position() {
        // 4 lines of leading whitespace/comments then a keyword.
        let src = "// l0\n\n// l2\n变量 x = 1;";
        let encoded = compute_tokens(src);
        assert!(!encoded.is_empty());
        // First token should be absolute on its line; check we don't emit garbage.
        assert!(encoded[0].length > 0);
    }

    #[test]
    fn cjk_uses_utf16_lengths() {
        // 变量 = 2 chars but each is 1 utf-16 code unit (BMP). length == 2.
        let raw = scan("变量");
        assert_eq!(raw.len(), 1);
        assert_eq!(raw[0].length, 2);
    }
}

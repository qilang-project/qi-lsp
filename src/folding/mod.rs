//! Folding range provider for Qi documents.
//!
//! Computes folding regions by scanning balanced braces ({}, 【】) and
//! contiguous comment / import blocks. Uses byte scanning rather than the
//! AST so it works even when the source has parse errors — folding should
//! degrade gracefully on edits-in-progress.

use anyhow::Result;
use log::{debug, warn};
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{FoldingRange, FoldingRangeKind, FoldingRangeParams};

use crate::document::DocumentManager;

/// Handle textDocument/foldingRange.
pub async fn handle_folding_range(
    connection: &Connection,
    request: Request,
    document_manager: &DocumentManager,
) -> Result<()> {
    debug!("Handling folding range request");

    let params: FoldingRangeParams = serde_json::from_value(request.params)?;
    let uri = params.text_document.uri.to_string();

    let ranges = match document_manager.get_document_content(&uri) {
        Some(content) => compute_folding_ranges(&content),
        None => {
            warn!("Document not found for folding: {}", uri);
            Vec::new()
        }
    };

    let response = Response {
        id: request.id,
        result: Some(serde_json::to_value(ranges)?),
        error: None,
    };
    connection.sender.send(Message::Response(response))?;
    Ok(())
}

/// Compute folding ranges by scanning the raw source.
///
/// Three kinds of regions:
/// - Brace pairs `{}` / `【】` spanning multiple lines.
/// - Two-or-more consecutive `//` line comments.
/// - Two-or-more consecutive `导入` lines (imports).
pub fn compute_folding_ranges(source: &str) -> Vec<FoldingRange> {
    let mut out = Vec::new();
    let lines: Vec<&str> = source.split('\n').collect();

    // Brace pairs: scan char-by-char respecting strings and comments.
    let mut stack: Vec<u32> = Vec::new();
    let mut line_idx: u32 = 0;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut in_string = false;
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\n' => {
                in_line_comment = false;
                line_idx += 1;
            }
            _ if in_line_comment => {}
            _ if in_block_comment => {
                if ch == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    in_block_comment = false;
                }
            }
            '"' => in_string = !in_string,
            _ if in_string => {
                if ch == '\\' {
                    chars.next();
                }
            }
            '/' => match chars.peek() {
                Some(&'/') => {
                    chars.next();
                    in_line_comment = true;
                }
                Some(&'*') => {
                    chars.next();
                    in_block_comment = true;
                }
                _ => {}
            },
            '{' | '【' => stack.push(line_idx),
            '}' | '】' => {
                if let Some(start) = stack.pop() {
                    if line_idx > start {
                        out.push(FoldingRange {
                            start_line: start,
                            start_character: None,
                            end_line: line_idx.saturating_sub(0),
                            end_character: None,
                            kind: Some(FoldingRangeKind::Region),
                            collapsed_text: None,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    // Contiguous `//` comment blocks.
    let mut block_start: Option<u32> = None;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let is_line_comment = trimmed.starts_with("//");
        match (is_line_comment, block_start) {
            (true, None) => block_start = Some(i as u32),
            (false, Some(start)) => {
                let end = i as u32 - 1;
                if end > start {
                    out.push(FoldingRange {
                        start_line: start,
                        start_character: None,
                        end_line: end,
                        end_character: None,
                        kind: Some(FoldingRangeKind::Comment),
                        collapsed_text: None,
                    });
                }
                block_start = None;
            }
            _ => {}
        }
    }
    if let Some(start) = block_start {
        let end = lines.len().saturating_sub(1) as u32;
        if end > start {
            out.push(FoldingRange {
                start_line: start,
                start_character: None,
                end_line: end,
                end_character: None,
                kind: Some(FoldingRangeKind::Comment),
                collapsed_text: None,
            });
        }
    }

    // Contiguous `导入` (import) blocks.
    let mut imp_start: Option<u32> = None;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let is_import = trimmed.starts_with("导入") || trimmed.starts_with("公开 导入");
        match (is_import, imp_start) {
            (true, None) => imp_start = Some(i as u32),
            (false, Some(start)) => {
                let end = i as u32 - 1;
                if end > start {
                    out.push(FoldingRange {
                        start_line: start,
                        start_character: None,
                        end_line: end,
                        end_character: None,
                        kind: Some(FoldingRangeKind::Imports),
                        collapsed_text: None,
                    });
                }
                imp_start = None;
            }
            _ => {}
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_brace_pair() {
        let src = "函数 f() {\n  返回 1;\n}\n";
        let r = compute_folding_ranges(src);
        assert!(r.iter().any(|f| f.start_line == 0 && f.end_line == 2));
    }

    #[test]
    fn folds_chinese_braces() {
        let src = "函数 f() 【\n  返回 1；\n】\n";
        let r = compute_folding_ranges(src);
        assert!(r.iter().any(|f| f.start_line == 0 && f.end_line == 2));
    }

    #[test]
    fn folds_consecutive_imports() {
        let src = "包 主程序;\n导入 a;\n导入 b;\n导入 c;\n函数 f() {}\n";
        let r = compute_folding_ranges(src);
        assert!(r.iter().any(|f| matches!(f.kind, Some(FoldingRangeKind::Imports))
            && f.start_line == 1
            && f.end_line == 3));
    }

    #[test]
    fn folds_consecutive_comments() {
        let src = "// header line 1\n// header line 2\n// header line 3\n函数 f() {}\n";
        let r = compute_folding_ranges(src);
        assert!(r.iter().any(|f| matches!(f.kind, Some(FoldingRangeKind::Comment))
            && f.start_line == 0
            && f.end_line == 2));
    }

    #[test]
    fn ignores_braces_in_strings_and_comments() {
        let src = "// not a fold: {\n变量 s: 字符串 = \"{ inside string }\";\n";
        let r = compute_folding_ranges(src);
        assert!(r.iter().all(|f| !matches!(f.kind, Some(FoldingRangeKind::Region))));
    }
}

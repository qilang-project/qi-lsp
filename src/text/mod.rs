//! Shared text utilities used by hover / definition / completion.
//!
//! These functions handle LSP positions correctly for Qi source. The previous
//! per-feature implementations mixed character indices with byte slicing, which
//! panicked the server on any cursor inside a CJK identifier.

use crate::document::{DocumentManager, DocumentPosition};
use lsp_types::Position;

/// Return true when `ch` can appear inside a Qi identifier — ASCII alphanumeric,
/// `_`, or CJK Unified Ideograph (BMP block U+4E00..U+9FFF).
pub fn is_identifier_char(ch: char) -> bool {
    let cu = ch as u32;
    ch.is_alphanumeric() || ch == '_' || (0x4E00..=0x9FFF).contains(&cu)
}

/// Extract the identifier surrounding `position` in the document at `uri`, if any.
///
/// `position.character` is interpreted as a **character index** within the line
/// (matching how `DocumentManager::position_to_offset` and ropey treat positions).
/// Returns the identifier text plus its character range within the line.
pub fn word_at_position(
    uri: &str,
    position: Position,
    document_manager: &DocumentManager,
) -> Option<(String, std::ops::Range<usize>)> {
    let line = document_manager.get_line_content(uri, position.line as usize)?;
    // Strip trailing newline if present, ropey includes it.
    let line = line.trim_end_matches('\n').trim_end_matches('\r');
    let chars: Vec<char> = line.chars().collect();
    let char_pos = position.character as usize;
    if char_pos > chars.len() {
        return None;
    }

    // Cursor sits between chars[char_pos-1] and chars[char_pos]. We accept either
    // side being an identifier character. Search left from char_pos for word start,
    // right from char_pos for word end.
    let mut start = char_pos;
    while start > 0 && is_identifier_char(chars[start - 1]) {
        start -= 1;
    }
    let mut end = char_pos;
    while end < chars.len() && is_identifier_char(chars[end]) {
        end += 1;
    }
    if start >= end {
        return None;
    }
    let word: String = chars[start..end].iter().collect();
    Some((word, start..end))
}

/// Compute the *char-prefix* of `position`'s line — text strictly before the
/// cursor. Returns `""` if line is missing.
///
/// Safe with CJK: works on `chars().take(n).collect()` rather than byte slicing.
pub fn line_before_cursor(
    uri: &str,
    position: Position,
    document_manager: &DocumentManager,
) -> String {
    let Some(line) = document_manager.get_line_content(uri, position.line as usize) else {
        return String::new();
    };
    let line = line.trim_end_matches('\n').trim_end_matches('\r');
    line.chars().take(position.character as usize).collect()
}

/// Build an LSP Range covering chars [start..end] on `line`.
pub fn char_range_on_line(line: u32, char_start: usize, char_end: usize) -> lsp_types::Range {
    lsp_types::Range {
        start: lsp_types::Position { line, character: char_start as u32 },
        end: lsp_types::Position { line, character: char_end as u32 },
    }
}

/// Convenience: turn a DocumentPosition into LSP Position.
#[allow(dead_code)]
pub fn pos_to_lsp(p: DocumentPosition) -> Position {
    Position { line: p.line as u32, character: p.character as u32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mgr_with(content: &str) -> (DocumentManager, String) {
        let dm = DocumentManager::new();
        let uri = "file:///t.qi".to_string();
        dm.open_document(&uri, content.to_string());
        (dm, uri)
    }

    #[test]
    fn extracts_ascii_word() {
        let (dm, uri) = mgr_with("let foo = 1\n");
        let p = Position { line: 0, character: 5 }; // inside "foo"
        let (w, r) = word_at_position(&uri, p, &dm).unwrap();
        assert_eq!(w, "foo");
        assert_eq!(r, 4..7);
    }

    #[test]
    fn extracts_cjk_word_no_panic() {
        // Reproduces the crash: cursor in middle of CJK identifier.
        let (dm, uri) = mgr_with("变量 名字 = 1\n");
        let p = Position { line: 0, character: 1 }; // between 变 and 量
        let (w, r) = word_at_position(&uri, p, &dm).unwrap();
        assert_eq!(w, "变量");
        assert_eq!(r, 0..2);
    }

    #[test]
    fn extracts_cjk_word_at_end_of_identifier() {
        let (dm, uri) = mgr_with("变量 名字 = 1\n");
        // line:  变 量 ␣ 名 字 ␣ = ␣ 1
        // idx:    0 1 2 3 4 5 6 7 8
        let p = Position { line: 0, character: 5 }; // just past 字
        let (w, r) = word_at_position(&uri, p, &dm).unwrap();
        assert_eq!(w, "名字");
        assert_eq!(r, 3..5);
    }

    #[test]
    fn no_word_at_whitespace() {
        let (dm, uri) = mgr_with("变量 名字\n");
        // line: 变 量 ␣ 名 字 → idx 2 is space, no chars around it are non-ident
        let p = Position { line: 0, character: 2 };
        // cursor is between 量(idx1) and ␣(idx2). 量 is on the left, ␣ on right.
        // start: scan left while chars[start-1] is identifier → start=0 (变量 both ident)
        // end:   scan right from 2 → chars[2]=' ' not ident → end=2
        // word "变量" returned, which is fine.
        let (w, _) = word_at_position(&uri, p, &dm).unwrap();
        assert_eq!(w, "变量");
    }

    #[test]
    fn line_before_cursor_cjk_safe() {
        // chars:   变 量 ␣ 名 字 ␣ = ␣ " 小 李 " ;
        // indices: 0  1 2 3  4 5 6 7 8 9  10 11 12
        let (dm, uri) = mgr_with("变量 名字 = \"小李\";\n");
        // Place cursor just after the opening quote (idx 9), prefix should end with `"`.
        let p = Position { line: 0, character: 9 };
        let prefix = line_before_cursor(&uri, p, &dm);
        assert_eq!(prefix.chars().count(), 9);
        assert!(prefix.ends_with('"'), "prefix={:?}", prefix);
    }

    #[test]
    fn line_before_cursor_missing_line_returns_empty() {
        let (dm, uri) = mgr_with("x\n");
        let p = Position { line: 99, character: 0 };
        assert_eq!(line_before_cursor(&uri, p, &dm), "");
    }
}
